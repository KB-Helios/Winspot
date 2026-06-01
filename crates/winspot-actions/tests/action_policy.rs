use winspot_actions::{ActionExecutor, ActionPolicy, clipboard_value};
use winspot_core::{ActionCapability, ActionKind, ActionRequested};

#[test]
fn clipboard_value_extracts_calculator_result() {
    assert_eq!(clipboard_value("2 + 2 = 4"), "4");
    assert_eq!(clipboard_value("Notepad"), "Notepad");
}

#[test]
fn executor_denies_clipboard_action_without_capability() {
    let executor = ActionExecutor::new(ActionPolicy::with_allowed([]));
    let completed = executor.execute(ActionRequested {
        action_id: "copy-1".to_string(),
        result_id: "calculator:2+2".to_string(),
        title: "2 + 2 = 4".to_string(),
        primary_action: ActionKind::Copy,
    });

    assert!(!completed.succeeded);
    assert!(completed.message.contains("ClipboardWrite"));
}

#[test]
fn open_action_refuses_disallowed_uri_scheme() {
    let executor = ActionExecutor::new(ActionPolicy::allow_all_local());
    let completed = executor.execute(ActionRequested {
        action_id: "open-1".to_string(),
        result_id: "evil:javascript:alert(1)".to_string(),
        title: "Definitely Safe".to_string(),
        primary_action: ActionKind::Open,
    });

    assert!(!completed.succeeded);
    assert!(completed.message.contains("refused to open"));
}

#[test]
fn open_action_refuses_nonexistent_path() {
    let executor = ActionExecutor::new(ActionPolicy::allow_all_local());
    let completed = executor.execute(ActionRequested {
        action_id: "open-2".to_string(),
        result_id: "file:C:\\winspot\\definitely\\missing.txt".to_string(),
        title: "Missing".to_string(),
        primary_action: ActionKind::Open,
    });

    assert!(!completed.succeeded);
    assert!(completed.message.contains("refused to open"));
}

#[test]
fn policy_allows_explicit_capabilities() {
    let policy = ActionPolicy::with_allowed([ActionCapability::ClipboardWrite]);

    assert!(
        policy
            .ensure_allowed(ActionCapability::ClipboardWrite)
            .is_ok()
    );
    assert!(
        policy
            .ensure_allowed(ActionCapability::ShellExecution)
            .is_err()
    );
}
