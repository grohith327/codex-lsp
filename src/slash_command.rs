//! Built-in slash command catalog.
//!
//! Ported from `codex-rs/tui/src/slash_command.rs`. Only `strum` is required.
//! The `available_during_task` accessor is preserved for fidelity but is not
//! used by the LSP.

use strum::IntoEnumIterator;
use strum_macros::AsRefStr;
use strum_macros::EnumIter;
use strum_macros::EnumString;
use strum_macros::IntoStaticStr;

/// Prefix used to namespace custom prompts as slash commands: `/prompts:<name>`.
pub const PROMPTS_CMD_PREFIX: &str = "prompts";

/// Commands that can be invoked by starting a message with a leading slash.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, EnumIter, AsRefStr, IntoStaticStr,
)]
#[strum(serialize_all = "kebab-case")]
pub enum SlashCommand {
    // DO NOT ALPHA-SORT! Enum order is presentation order in the popup, so
    // more frequently used commands should be listed first.
    Model,
    Approvals,
    #[strum(serialize = "setup-elevated-sandbox")]
    ElevateSandbox,
    Experimental,
    Skills,
    Review,
    New,
    Resume,
    Fork,
    Init,
    Compact,
    Diff,
    Mention,
    Status,
    Mcp,
    Logout,
    Quit,
    Exit,
    Feedback,
    Rollout,
    Ps,
    TestApproval,
}

impl SlashCommand {
    /// User-visible description shown in the popup.
    pub fn description(self) -> &'static str {
        match self {
            SlashCommand::Feedback => "send logs to maintainers",
            SlashCommand::New => "start a new chat during a conversation",
            SlashCommand::Init => "create an AGENTS.md file with instructions for Codex",
            SlashCommand::Compact => "summarize conversation to prevent hitting the context limit",
            SlashCommand::Review => "review my current changes and find issues",
            SlashCommand::Resume => "resume a saved chat",
            SlashCommand::Fork => "fork the current chat",
            SlashCommand::Quit | SlashCommand::Exit => "exit Codex",
            SlashCommand::Diff => "show git diff (including untracked files)",
            SlashCommand::Mention => "mention a file",
            SlashCommand::Skills => "use skills to improve how Codex performs specific tasks",
            SlashCommand::Status => "show current session configuration and token usage",
            SlashCommand::Ps => "list background terminals",
            SlashCommand::Model => "choose what model and reasoning effort to use",
            SlashCommand::Approvals => "choose what Codex can do without approval",
            SlashCommand::ElevateSandbox => "set up elevated agent sandbox",
            SlashCommand::Experimental => "toggle beta features",
            SlashCommand::Mcp => "list configured MCP tools",
            SlashCommand::Logout => "log out of Codex",
            SlashCommand::Rollout => "print the rollout file path",
            SlashCommand::TestApproval => "test approval request",
        }
    }

    /// Command string without the leading '/'.
    pub fn command(self) -> &'static str {
        self.into()
    }

    /// Whether this command can be run while a task is in progress.
    #[allow(dead_code)]
    pub fn available_during_task(self) -> bool {
        match self {
            SlashCommand::New
            | SlashCommand::Resume
            | SlashCommand::Fork
            | SlashCommand::Init
            | SlashCommand::Compact
            | SlashCommand::Model
            | SlashCommand::Approvals
            | SlashCommand::ElevateSandbox
            | SlashCommand::Experimental
            | SlashCommand::Review
            | SlashCommand::Logout => false,
            SlashCommand::Diff
            | SlashCommand::Mention
            | SlashCommand::Skills
            | SlashCommand::Status
            | SlashCommand::Ps
            | SlashCommand::Mcp
            | SlashCommand::Feedback
            | SlashCommand::Quit
            | SlashCommand::Exit => true,
            SlashCommand::Rollout => true,
            SlashCommand::TestApproval => true,
        }
    }

    fn is_visible(self) -> bool {
        match self {
            SlashCommand::Rollout | SlashCommand::TestApproval => cfg!(debug_assertions),
            _ => true,
        }
    }
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    SlashCommand::iter()
        .filter(|command| command.is_visible())
        .map(|c| (c.command(), c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_strings_are_kebab_case() {
        assert_eq!(SlashCommand::Model.command(), "model");
        assert_eq!(
            SlashCommand::ElevateSandbox.command(),
            "setup-elevated-sandbox"
        );
    }

    #[test]
    fn built_in_list_is_non_empty_and_ordered() {
        let cmds = built_in_slash_commands();
        assert_eq!(cmds.first().map(|(_, c)| *c), Some(SlashCommand::Model));
        assert!(cmds.iter().any(|(name, _)| *name == "init"));
    }
}
