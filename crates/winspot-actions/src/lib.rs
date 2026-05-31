use std::{collections::HashSet, process::Command};

use winspot_core::{ActionCapability, ActionCompleted, ActionKind, ActionRequested};

#[derive(Debug, Clone, Default)]
pub struct ActionPolicy {
    allowed: HashSet<ActionCapability>,
}

impl ActionPolicy {
    pub fn allow_all_local() -> Self {
        Self {
            allowed: [
                ActionCapability::ClipboardWrite,
                ActionCapability::FilesystemRead,
                ActionCapability::ProcessExecution,
                ActionCapability::ProcessInspection,
                ActionCapability::ShellExecution,
                ActionCapability::PluginExecution,
            ]
            .into_iter()
            .collect(),
        }
    }

    pub fn with_allowed(allowed: impl IntoIterator<Item = ActionCapability>) -> Self {
        Self {
            allowed: allowed.into_iter().collect(),
        }
    }

    pub fn ensure_allowed(&self, capability: ActionCapability) -> anyhow::Result<()> {
        if self.allowed.contains(&capability) {
            Ok(())
        } else {
            anyhow::bail!("Capability {:?} is not allowed", capability)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionExecutor {
    policy: ActionPolicy,
}

impl ActionExecutor {
    pub fn new(policy: ActionPolicy) -> Self {
        Self { policy }
    }

    pub fn execute(&self, action: ActionRequested) -> ActionCompleted {
        match self.execute_inner(&action) {
            Ok(message) => ActionCompleted {
                action_id: action.action_id,
                succeeded: true,
                message,
            },
            Err(error) => ActionCompleted {
                action_id: action.action_id,
                succeeded: false,
                message: error.to_string(),
            },
        }
    }

    fn execute_inner(&self, action: &ActionRequested) -> anyhow::Result<String> {
        match action.primary_action {
            ActionKind::Copy | ActionKind::CopyPath => {
                self.policy
                    .ensure_allowed(ActionCapability::ClipboardWrite)?;
                copy_to_clipboard(clipboard_value(&action.title))?;
                Ok(format!("Copied {}", clipboard_value(&action.title)))
            }
            ActionKind::RunCommand => {
                self.policy
                    .ensure_allowed(ActionCapability::ProcessExecution)?;
                run_command_action(action)
            }
            ActionKind::Open | ActionKind::OpenContainingFolder => {
                self.policy
                    .ensure_allowed(ActionCapability::ShellExecution)?;
                open_action(action)
            }
            ActionKind::KillProcess => {
                self.policy
                    .ensure_allowed(ActionCapability::ProcessExecution)?;
                Ok(format!("Process action queued for {}", action.title))
            }
            ActionKind::PluginCommand => {
                self.policy
                    .ensure_allowed(ActionCapability::PluginExecution)?;
                Ok(format!("Ran plugin action {}", action.title))
            }
        }
    }
}

pub fn clipboard_value(title: &str) -> &str {
    match title.rsplit_once(" = ") {
        Some((_, value)) => value.trim(),
        None => title.trim(),
    }
}

fn run_command_action(action: &ActionRequested) -> anyhow::Result<String> {
    let command = match action.result_id.as_str() {
        "command:calculator" => "calc.exe",
        "command:terminal" => "wt.exe",
        other => anyhow::bail!("Unsupported command action {other}"),
    };

    Command::new(command)
        .spawn()
        .map(|_| format!("Launched {}", action.title))
        .map_err(|error| anyhow::anyhow!("run {}: {error}", action.title))
}

fn open_action(action: &ActionRequested) -> anyhow::Result<String> {
    let Some((_, target)) = action.result_id.split_once(':') else {
        anyhow::bail!("Action target is missing");
    };

    Command::new("explorer")
        .arg(target)
        .spawn()
        .map(|_| format!("Opened {}", action.title))
        .map_err(|error| anyhow::anyhow!("open {}: {error}", action.title))
}

#[cfg(windows)]
fn copy_to_clipboard(value: &str) -> anyhow::Result<()> {
    clipboard_win::set_clipboard_string(value)
        .map_err(|error| anyhow::anyhow!("copy {value} to clipboard: {error}"))
}

#[cfg(not(windows))]
fn copy_to_clipboard(_value: &str) -> anyhow::Result<()> {
    Ok(())
}
