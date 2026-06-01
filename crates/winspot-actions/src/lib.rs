use std::{collections::HashSet, ffi::OsString, process::Command};

use winspot_core::{
    ActionCapability, ActionCompleted, ActionKind, ActionRequested, OpenTarget,
    classify_open_target,
};

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
                kill_process_action(action)
            }
            ActionKind::PluginCommand => {
                self.policy
                    .ensure_allowed(ActionCapability::PluginExecution)?;
                plugin_command_action(action)
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

fn kill_process_action(action: &ActionRequested) -> anyhow::Result<String> {
    // Process results carry their PID in the id ("process:<pid>:<image>"); parse
    // it rather than trusting the title, and report the real `taskkill` outcome
    // instead of pretending the kill always succeeds.
    let pid = action
        .result_id
        .strip_prefix("process:")
        .and_then(|rest| rest.split(':').next())
        .and_then(|pid| pid.trim().parse::<u32>().ok())
        .ok_or_else(|| anyhow::anyhow!("refused to kill {}: missing process id", action.title))?;

    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .map_err(|error| anyhow::anyhow!("kill {}: {error}", action.title))?;

    if output.status.success() {
        return Ok(format!("Terminated {} (PID {pid})", action.title));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = if stderr.trim().is_empty() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        stderr
    };
    anyhow::bail!(
        "kill {} (PID {pid}) failed: {}",
        action.title,
        detail.trim()
    )
}

fn plugin_command_action(action: &ActionRequested) -> anyhow::Result<String> {
    // Plugin results carry their id as "plugin:<id>". Only built-ins that map to
    // a launchable command are executable; the rest are search-only providers,
    // so refuse them explicitly instead of reporting a fake success.
    let plugin_id = action
        .result_id
        .strip_prefix("plugin:")
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow::anyhow!("refused to run {}: malformed plugin id", action.title))?;

    let command = match plugin_id {
        "calculator" => "calc.exe",
        "terminal" => "wt.exe",
        other => anyhow::bail!("plugin '{other}' has no executable command"),
    };

    Command::new(command)
        .spawn()
        .map(|_| format!("Launched {}", action.title))
        .map_err(|error| anyhow::anyhow!("run plugin {}: {error}", action.title))
}

fn open_action(action: &ActionRequested) -> anyhow::Result<String> {
    // Validate the client-supplied target before handing it to the shell so a
    // malicious IPC message cannot drive `explorer` into a disallowed protocol
    // handler or a malformed/injected argument.
    let target = classify_open_target(&action.result_id)
        .map_err(|error| anyhow::anyhow!("refused to open {}: {error}", action.title))?;

    let arg: OsString = match target {
        OpenTarget::Path(path) => path.into_os_string(),
        OpenTarget::Uri(uri) => OsString::from(uri),
    };

    Command::new("explorer")
        .arg(arg)
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
