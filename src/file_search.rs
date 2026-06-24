//! Warm `fff-search` adapter for `@` file completions.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use fff_search::FFFMode;
use fff_search::FilePicker;
use fff_search::FilePickerOptions;
use fff_search::FuzzySearchOptions;
use fff_search::MixedItemRef;
use fff_search::MixedSearchConfig;
use fff_search::PaginationArgs;
use fff_search::QueryParser;
use fff_search::SharedFilePicker;
use fff_search::SharedFrecency;
use fff_search::SharedQueryTracker;

const INITIAL_SCAN_WAIT: Duration = Duration::from_secs(2);
const SEARCH_THREADS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSearchKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSearchMatch {
    pub path: PathBuf,
    pub kind: FileSearchKind,
    pub score: i32,
}

#[derive(Default)]
pub struct FffFileSearch {
    roots: DashMap<PathBuf, Arc<SearchRoot>>,
}

struct SearchRoot {
    picker: SharedFilePicker,
    query_tracker: SharedQueryTracker,
}

impl FffFileSearch {
    pub async fn search(&self, root: &Path, query: &str, limit: usize) -> Vec<FileSearchMatch> {
        if query.trim().is_empty() || limit == 0 {
            return Vec::new();
        }

        let Ok(root) = std::fs::canonicalize(root) else {
            return Vec::new();
        };
        let Some(state) = self.root_state(root) else {
            return Vec::new();
        };

        let query = query.to_string();
        tokio::task::spawn_blocking(move || search_blocking(state, &query, limit))
            .await
            .unwrap_or_default()
    }

    pub fn cached_root_count(&self) -> usize {
        self.roots.len()
    }

    fn root_state(&self, root: PathBuf) -> Option<Arc<SearchRoot>> {
        if let Some(existing) = self.roots.get(&root) {
            return Some(existing.clone());
        }

        let created = Arc::new(SearchRoot::new(&root)?);
        match self.roots.entry(root) {
            dashmap::mapref::entry::Entry::Occupied(entry) => Some(entry.get().clone()),
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(created.clone());
                Some(created)
            }
        }
    }
}

impl SearchRoot {
    fn new(root: &Path) -> Option<Self> {
        let base_path = root.to_str()?.to_string();
        let picker = SharedFilePicker::default();
        let frecency = SharedFrecency::noop();
        let query_tracker = SharedQueryTracker::noop();

        FilePicker::new_with_shared_state(
            picker.clone(),
            frecency,
            FilePickerOptions {
                base_path,
                enable_mmap_cache: false,
                enable_content_indexing: false,
                mode: FFFMode::Ai,
                watch: true,
                follow_symlinks: true,
                ..Default::default()
            },
        )
        .ok()?;

        Some(Self {
            picker,
            query_tracker,
        })
    }
}

fn search_blocking(state: Arc<SearchRoot>, query: &str, limit: usize) -> Vec<FileSearchMatch> {
    state.picker.wait_for_scan(INITIAL_SCAN_WAIT);

    let Ok(picker_guard) = state.picker.read() else {
        return Vec::new();
    };
    let Some(picker) = picker_guard.as_ref() else {
        return Vec::new();
    };
    let Ok(query_guard) = state.query_tracker.read() else {
        return Vec::new();
    };

    let parser = QueryParser::new(MixedSearchConfig);
    let parsed = parser.parse(query);
    let results = picker.fuzzy_search_mixed(
        &parsed,
        query_guard.as_ref(),
        FuzzySearchOptions {
            max_threads: SEARCH_THREADS,
            current_file: None,
            project_path: Some(picker.base_path()),
            combo_boost_score_multiplier: 0,
            min_combo_count: 0,
            pagination: PaginationArgs { offset: 0, limit },
        },
    );

    let mut matches: Vec<_> = results
        .items
        .into_iter()
        .zip(results.scores)
        .filter_map(|(item, score)| match item {
            MixedItemRef::File(file) => Some(FileSearchMatch {
                path: PathBuf::from(file.relative_path(picker)),
                kind: FileSearchKind::File,
                score: score.total,
            }),
            MixedItemRef::Dir(dir) => {
                let path = normalize_dir_path(dir.relative_path(picker));
                (!path.as_os_str().is_empty()).then_some(FileSearchMatch {
                    path,
                    kind: FileSearchKind::Directory,
                    score: score.total,
                })
            }
        })
        .collect();

    matches.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    matches
}

fn normalize_dir_path(path: String) -> PathBuf {
    PathBuf::from(path.trim_end_matches(['/', '\\']))
}
