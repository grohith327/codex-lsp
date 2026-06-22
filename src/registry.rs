//! Command / custom-prompt / skill registry with simple TTL caching.
//!
//! Custom prompts and skills are loaded from disk with lean local structs (we
//! deliberately avoid depending on `codex-core`/`codex-protocol`). The loaders
//! mirror codex's behavior: prompts are `*.md` under `$CODEX_HOME/prompts`;
//! skills are `SKILL.md` files discovered recursively under known roots.

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

use crate::slash_command::PROMPTS_CMD_PREFIX;
use crate::slash_command::built_in_slash_commands;

const SKILL_SCAN_MAX_DEPTH: usize = 6;

#[derive(Debug, Clone)]
pub struct CustomPrompt {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Registry {
    pub prompts: Vec<CustomPrompt>,
    pub skills: Vec<Skill>,
}

impl Registry {
    /// Load prompts and skills given the workspace roots.
    pub fn load(workspace_roots: &[PathBuf]) -> Self {
        let mut prompts = Vec::new();
        if let Some(dir) = default_prompts_dir() {
            prompts = load_prompts(&dir);
        }

        let mut skill_roots: Vec<SkillRoot> = workspace_roots
            .iter()
            .cloned()
            .map(SkillRoot::plain)
            .collect();
        if let Some(home) = codex_home() {
            skill_roots.push(SkillRoot::plain(home.join("skills")));
            skill_roots.extend(configured_plugin_skill_roots(&home));
        }
        let skills = load_skills_from_roots(&skill_roots);

        Self { prompts, skills }
    }

    /// Prompt names (without the `prompts:` prefix), for slash completion.
    pub fn prompt_names(&self) -> Vec<String> {
        self.prompts.iter().map(|p| p.name.clone()).collect()
    }

    /// Whether `name` (the text after `/`) is an exact, known command. Used by
    /// diagnostics — fuzzy matching is only for keeping the popup open.
    pub fn is_known_command(&self, name: &str) -> bool {
        if built_in_slash_commands()
            .iter()
            .any(|(cmd, _)| *cmd == name)
        {
            return true;
        }
        let prefix = format!("{PROMPTS_CMD_PREFIX}:");
        if let Some(rest) = name.strip_prefix(&prefix) {
            return self.prompts.iter().any(|p| p.name == rest);
        }
        false
    }

    pub fn has_skill(&self, name: &str) -> bool {
        self.skills.iter().any(|s| s.name == name)
    }
}

pub fn codex_home() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("CODEX_HOME")
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    home_dir().map(|h| h.join(".codex"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

pub fn default_prompts_dir() -> Option<PathBuf> {
    codex_home().map(|h| h.join("prompts"))
}

/// Load `*.md` prompts from `dir`, sorted by name. Non-existent dir -> empty.
pub fn load_prompts(dir: &Path) -> Vec<CustomPrompt> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let description = std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| frontmatter_value(&c, "description"));
        out.push(CustomPrompt {
            name: name.to_string(),
            description,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Recursively discover `SKILL.md` files under `roots`, deduped by name.
pub fn load_skills(roots: &[PathBuf]) -> Vec<Skill> {
    let roots: Vec<SkillRoot> = roots.iter().cloned().map(SkillRoot::plain).collect();
    load_skills_from_roots(&roots)
}

fn load_skills_from_roots(roots: &[SkillRoot]) -> Vec<Skill> {
    let mut out: Vec<Skill> = Vec::new();
    for root in roots {
        discover_skills(&root.path, 0, root.plugin.as_ref(), &mut out);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);
    out
}

#[derive(Debug, Clone)]
struct SkillRoot {
    path: PathBuf,
    plugin: Option<PluginSkillMetadata>,
}

impl SkillRoot {
    fn plain(path: PathBuf) -> Self {
        Self { path, plugin: None }
    }
}

#[derive(Debug, Clone)]
struct PluginSkillMetadata {
    name: String,
    description: Option<String>,
    display_name: Option<String>,
    short_description: Option<String>,
}

fn discover_skills(
    dir: &Path,
    depth: usize,
    plugin: Option<&PluginSkillMetadata>,
    out: &mut Vec<Skill>,
) {
    if depth > SKILL_SCAN_MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            discover_skills(&path, depth + 1, plugin, out);
        } else if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                // Prefer an explicit frontmatter `name`; fall back to the
                // containing directory name.
                let name = frontmatter_value(&content, "name").or_else(|| {
                    path.parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .map(str::to_string)
                });
                if let Some(name) = name {
                    let mut description = frontmatter_value(&content, "description");
                    let mut display_name = None;
                    if let Some(plugin) = plugin
                        && plugin.name == name
                    {
                        display_name = plugin.display_name.clone();
                        description = description
                            .or_else(|| plugin.short_description.clone())
                            .or_else(|| plugin.description.clone());
                    }
                    out.push(Skill {
                        name,
                        description,
                        display_name,
                    });
                }
            }
        }
    }
}

fn configured_plugin_skill_roots(codex_home: &Path) -> Vec<SkillRoot> {
    let config_path = codex_home.join("config.toml");
    let Ok(config) = std::fs::read_to_string(config_path) else {
        return Vec::new();
    };
    let Ok(config) = config.parse::<toml::Value>() else {
        return Vec::new();
    };

    let marketplace_sources = marketplace_sources(&config);
    let mut roots = Vec::new();
    for plugin in enabled_plugins(&config) {
        let source = marketplace_sources.get(&plugin.marketplace);
        let Some(plugin_root) =
            resolve_plugin_root(codex_home, &plugin.name, &plugin.marketplace, source)
        else {
            continue;
        };
        if let Some(root) = plugin_skill_root(&plugin_root) {
            roots.push(root);
        }
    }

    roots.sort_by(|a, b| a.path.cmp(&b.path));
    roots.dedup_by(|a, b| a.path == b.path);
    roots
}

#[derive(Debug)]
struct EnabledPlugin {
    name: String,
    marketplace: String,
}

fn enabled_plugins(config: &toml::Value) -> Vec<EnabledPlugin> {
    let Some(plugins) = config.get("plugins").and_then(toml::Value::as_table) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for (key, value) in plugins {
        let enabled = value
            .as_table()
            .and_then(|table| table.get("enabled"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        if !enabled {
            continue;
        }
        let Some((name, marketplace)) = key.split_once('@') else {
            continue;
        };
        if name.is_empty() || marketplace.is_empty() {
            continue;
        }
        out.push(EnabledPlugin {
            name: name.to_string(),
            marketplace: marketplace.to_string(),
        });
    }
    out
}

fn marketplace_sources(config: &toml::Value) -> std::collections::HashMap<String, PathBuf> {
    let Some(marketplaces) = config.get("marketplaces").and_then(toml::Value::as_table) else {
        return std::collections::HashMap::new();
    };

    marketplaces
        .iter()
        .filter_map(|(name, value)| {
            let source = value
                .as_table()
                .and_then(|table| table.get("source"))
                .and_then(toml::Value::as_str)?;
            Some((name.clone(), PathBuf::from(source)))
        })
        .collect()
}

fn resolve_plugin_root(
    codex_home: &Path,
    plugin: &str,
    marketplace: &str,
    marketplace_source: Option<&PathBuf>,
) -> Option<PathBuf> {
    let cache_plugin_dir = codex_home
        .join("plugins")
        .join("cache")
        .join(marketplace)
        .join(plugin);
    if let Some(root) = latest_plugin_root(&cache_plugin_dir) {
        return Some(root);
    }

    if let Some(source) = marketplace_source {
        for candidate in [source.join("plugins").join(plugin), source.join(plugin)] {
            if candidate
                .join(".codex-plugin")
                .join("plugin.json")
                .is_file()
            {
                return Some(candidate);
            }
        }
    }

    None
}

fn latest_plugin_root(plugin_dir: &Path) -> Option<PathBuf> {
    if plugin_dir
        .join(".codex-plugin")
        .join("plugin.json")
        .is_file()
    {
        return Some(plugin_dir.to_path_buf());
    }

    let entries = std::fs::read_dir(plugin_dir).ok()?;
    let mut versions: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join(".codex-plugin").join("plugin.json").is_file())
        .collect();
    versions.sort_by(|a, b| {
        b.file_name()
            .and_then(|n| n.to_str())
            .cmp(&a.file_name().and_then(|n| n.to_str()))
    });
    versions.into_iter().next()
}

#[derive(Debug, Deserialize)]
struct PluginManifest {
    name: String,
    description: Option<String>,
    skills: Option<PathBuf>,
    interface: Option<PluginInterface>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginInterface {
    display_name: Option<String>,
    short_description: Option<String>,
}

fn plugin_skill_root(plugin_root: &Path) -> Option<SkillRoot> {
    let manifest_path = plugin_root.join(".codex-plugin").join("plugin.json");
    let manifest: PluginManifest =
        serde_json::from_str(&std::fs::read_to_string(manifest_path).ok()?).ok()?;

    let skills = manifest.skills.unwrap_or_else(|| PathBuf::from("skills"));
    let path = if skills.is_absolute() {
        skills
    } else {
        plugin_root.join(skills)
    };
    if !path.is_dir() {
        return None;
    }

    let interface = manifest.interface;
    Some(SkillRoot {
        path,
        plugin: Some(PluginSkillMetadata {
            name: manifest.name,
            description: manifest.description,
            display_name: interface
                .as_ref()
                .and_then(|interface| interface.display_name.clone()),
            short_description: interface.and_then(|interface| interface.short_description),
        }),
    })
}

/// Extract `key: value` from a leading `---` YAML-ish frontmatter block.
fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    let rest = content.strip_prefix("---")?;
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))?;
    let end = rest.find("\n---")?;
    let block = &rest[..end];
    for line in block.lines() {
        if let Some((k, v)) = line.split_once(':')
            && k.trim() == key
        {
            let v = v.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            if v.is_empty() {
                return None;
            }
            return Some(v.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn frontmatter_parsing() {
        let c = "---\nname: My Skill\ndescription: \"does things\"\n---\nbody";
        assert_eq!(frontmatter_value(c, "name").as_deref(), Some("My Skill"));
        assert_eq!(
            frontmatter_value(c, "description").as_deref(),
            Some("does things")
        );
        assert_eq!(frontmatter_value(c, "missing"), None);
        assert_eq!(frontmatter_value("no frontmatter", "name"), None);
    }

    #[test]
    fn loads_prompts_and_skills_from_disk() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path();

        let prompts_dir = root.join("prompts");
        fs::create_dir_all(&prompts_dir).unwrap();
        fs::write(
            prompts_dir.join("deploy.md"),
            "---\ndescription: ship it\n---\nrun the deploy",
        )
        .unwrap();
        fs::write(prompts_dir.join("notes.txt"), "ignored").unwrap();

        let prompts = load_prompts(&prompts_dir);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "deploy");
        assert_eq!(prompts[0].description.as_deref(), Some("ship it"));

        let skill_dir = root.join("skills").join("review");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: review\ndescription: review code\n---\n",
        )
        .unwrap();

        let skills = load_skills(&[root.join("skills")]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "review");
    }

    #[test]
    fn is_known_command_checks_builtins_and_prompts() {
        let reg = Registry {
            prompts: vec![CustomPrompt {
                name: "deploy".into(),
                description: None,
            }],
            skills: vec![],
        };
        assert!(reg.is_known_command("model"));
        assert!(reg.is_known_command("prompts:deploy"));
        assert!(!reg.is_known_command("prompts:nope"));
        assert!(!reg.is_known_command("definitelynotacommand"));
    }
}
