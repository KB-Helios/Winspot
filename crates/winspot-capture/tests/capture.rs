use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use winspot_capture::{
    CaptureBackend, CaptureCommand, CaptureMode, CaptureService, CaptureTarget, MonitorSelection,
    WINDOWS_CAPTURE_PLUGIN_ID, WINDOWS_CAPTURE_RESULT_PREFIX, WindowsCaptureProvider,
    WindowsCaptureSettings, artifact_path_with_timestamp, default_output_directory_from,
    load_settings_from_path, parse_capture_query, parse_capture_result_id,
};
use winspot_core::{ActionKind, SearchResultKind};
use winspot_search::providers::DynamicSearchProvider;

#[test]
fn parse_capture_queries_recognizes_screenshots_and_recordings() {
    let settings = WindowsCaptureSettings::default();

    assert_eq!(
        parse_capture_query("screenshot", &settings),
        Some(CaptureCommand::screenshot(CaptureTarget::Monitor(
            MonitorSelection::Primary
        )))
    );
    assert_eq!(
        parse_capture_query("capture screen", &settings),
        Some(CaptureCommand::screenshot(CaptureTarget::Monitor(
            MonitorSelection::Primary
        )))
    );
    assert_eq!(
        parse_capture_query("screenshot monitor 2", &settings),
        Some(CaptureCommand::screenshot(CaptureTarget::Monitor(
            MonitorSelection::Index(2)
        )))
    );
    assert_eq!(
        parse_capture_query("screenshot window", &settings),
        Some(CaptureCommand::screenshot(CaptureTarget::ForegroundWindow))
    );
    assert_eq!(
        parse_capture_query("record screen", &settings),
        Some(CaptureCommand::record(
            CaptureTarget::Monitor(MonitorSelection::Primary),
            8
        ))
    );
    assert_eq!(
        parse_capture_query("record screen 10s", &settings),
        Some(CaptureCommand::record(
            CaptureTarget::Monitor(MonitorSelection::Primary),
            10
        ))
    );
    assert_eq!(
        parse_capture_query("record monitor 2 20s", &settings),
        Some(CaptureCommand::record(
            CaptureTarget::Monitor(MonitorSelection::Index(2)),
            20
        ))
    );
    assert_eq!(
        parse_capture_query("record window 10s", &settings),
        Some(CaptureCommand::record(CaptureTarget::ForegroundWindow, 10))
    );
    assert_eq!(parse_capture_query("screen saver", &settings), None);
}

#[test]
fn parse_capture_queries_clamps_recording_duration() {
    let settings = WindowsCaptureSettings {
        default_record_seconds: 3,
        max_record_seconds: 12,
        ..WindowsCaptureSettings::default()
    }
    .normalized();

    assert_eq!(
        parse_capture_query("record screen 0s", &settings),
        Some(CaptureCommand::record(
            CaptureTarget::Monitor(MonitorSelection::Primary),
            1
        ))
    );
    assert_eq!(
        parse_capture_query("record screen 99s", &settings),
        Some(CaptureCommand::record(
            CaptureTarget::Monitor(MonitorSelection::Primary),
            12
        ))
    );
    assert_eq!(
        parse_capture_query("record window", &settings),
        Some(CaptureCommand::record(CaptureTarget::ForegroundWindow, 3))
    );
}

#[test]
fn result_ids_round_trip_and_reject_malformed_payloads() {
    let settings = WindowsCaptureSettings::default();
    let commands = [
        CaptureCommand::screenshot(CaptureTarget::Monitor(MonitorSelection::Primary)),
        CaptureCommand::screenshot(CaptureTarget::Monitor(MonitorSelection::Index(2))),
        CaptureCommand::screenshot(CaptureTarget::ForegroundWindow),
        CaptureCommand::record(CaptureTarget::Monitor(MonitorSelection::Primary), 8),
        CaptureCommand::record(CaptureTarget::ForegroundWindow, 10),
    ];

    for command in commands {
        let id = command.result_id();
        assert!(id.starts_with(WINDOWS_CAPTURE_RESULT_PREFIX));
        assert_eq!(
            parse_capture_result_id(&id, &settings).expect("parse result id"),
            command
        );
    }

    assert!(parse_capture_result_id("plugin:windows-capture", &settings).is_err());
    assert!(
        parse_capture_result_id("plugin:windows-capture:screenshot:monitor:0", &settings).is_err()
    );
    assert!(
        parse_capture_result_id("plugin:windows-capture:record:screen:primary:8", &settings)
            .is_err()
    );
    assert!(
        parse_capture_result_id(
            "plugin:windows-capture:record:monitor:primary:999",
            &settings
        )
        .is_err()
    );
    assert!(
        parse_capture_result_id("plugin:fastflowlm:record:monitor:primary:8", &settings).is_err()
    );
}

#[test]
fn settings_normalize_output_directory_and_recording_caps() {
    let settings = WindowsCaptureSettings {
        output_directory: "  C:\\Captures  ".to_string(),
        default_record_seconds: 0,
        max_record_seconds: 0,
        pre_capture_delay_ms: 10_000,
        ..WindowsCaptureSettings::default()
    }
    .normalized();

    assert_eq!(settings.output_directory, "C:\\Captures");
    assert_eq!(settings.default_record_seconds, 8);
    assert_eq!(settings.max_record_seconds, 60);
    assert_eq!(settings.pre_capture_delay_ms, 5_000);

    let settings = WindowsCaptureSettings {
        default_record_seconds: 120,
        max_record_seconds: 30,
        ..WindowsCaptureSettings::default()
    }
    .normalized();
    assert_eq!(settings.default_record_seconds, 30);
}

#[test]
fn load_settings_reads_capture_object_from_launcher_settings_json() {
    let path = std::env::temp_dir().join(format!(
        "winspot-capture-settings-{}-{}.json",
        std::process::id(),
        1
    ));
    fs::write(
        &path,
        r#"{
            "capture": {
                "enabled": false,
                "outputDirectory": " C:\\Captures ",
                "defaultRecordSeconds": 4,
                "maxRecordSeconds": 12,
                "includeCursor": false,
                "preCaptureDelayMs": 500
            }
        }"#,
    )
    .expect("write settings");

    let settings = load_settings_from_path(&path).expect("load capture settings");

    assert!(!settings.enabled);
    assert_eq!(settings.output_directory, r"C:\Captures");
    assert_eq!(settings.default_record_seconds, 4);
    assert_eq!(settings.max_record_seconds, 12);
    assert!(!settings.include_cursor);
    assert_eq!(settings.pre_capture_delay_ms, 500);

    fs::remove_file(path).expect("cleanup settings");
}

#[test]
fn output_paths_follow_portable_and_installed_layouts() {
    let portable_dir = Path::new(r"C:\WinspotPortable");
    assert_eq!(
        default_output_directory_from(portable_dir, true, Some(r"C:\Ignored")),
        Some(PathBuf::from(r"C:\WinspotPortable\data\captures"))
    );
    assert_eq!(
        default_output_directory_from(portable_dir, false, Some(r"C:\Users\Kevin\AppData\Local")),
        Some(PathBuf::from(
            r"C:\Users\Kevin\AppData\Local\Winspot\captures"
        ))
    );
    assert_eq!(
        default_output_directory_from(portable_dir, false, None),
        None
    );

    assert_eq!(
        artifact_path_with_timestamp(Path::new(r"C:\Captures"), CaptureMode::Screenshot, 42),
        PathBuf::from(r"C:\Captures\winspot-screenshot-42.png")
    );
    assert_eq!(
        artifact_path_with_timestamp(
            Path::new(r"C:\Captures"),
            CaptureMode::Record { seconds: 8 },
            43
        ),
        PathBuf::from(r"C:\Captures\winspot-recording-43.mp4")
    );
}

#[test]
fn provider_emits_lightweight_plugin_results_for_capture_queries() {
    let provider = WindowsCaptureProvider::new(WindowsCaptureSettings::default());
    let results = provider.search("record monitor 2 20s");

    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result.id, "plugin:windows-capture:record:monitor:2:20");
    assert_eq!(result.title, "Record monitor 2 for 20s");
    assert_eq!(result.kind, SearchResultKind::Plugin);
    assert_eq!(result.source.as_deref(), Some(WINDOWS_CAPTURE_PLUGIN_ID));
    assert_eq!(result.primary_action, ActionKind::PluginCommand);
    assert!(result.actions.iter().any(|action| {
        action.id == "run-plugin"
            && action.label == "Record"
            && action.kind == ActionKind::PluginCommand
    }));

    assert!(provider.search("record collection").is_empty());
}

#[test]
fn service_uses_backend_and_reports_output_path() {
    let backend = Arc::new(RecordingBackend::default());
    let service = CaptureService::with_backend(
        WindowsCaptureSettings {
            output_directory: r"C:\Captures".to_string(),
            ..WindowsCaptureSettings::default()
        },
        backend.clone(),
    );

    let outcome = service
        .execute_result_id("plugin:windows-capture:screenshot:window:foreground")
        .expect("capture succeeds");

    assert!(outcome.output_path.contains("winspot-screenshot-"));
    assert!(outcome.output_path.ends_with(".png"));
    assert_eq!(
        backend.calls.lock().expect("calls").as_slice(),
        &[CaptureCommand::screenshot(CaptureTarget::ForegroundWindow)]
    );
}

#[test]
fn service_refuses_disabled_capture_settings() {
    let service = CaptureService::with_backend(
        WindowsCaptureSettings {
            enabled: false,
            ..WindowsCaptureSettings::default()
        },
        Arc::new(RecordingBackend::default()),
    );

    let error = service
        .execute_result_id("plugin:windows-capture:screenshot:monitor:primary")
        .expect_err("disabled capture should fail");

    assert!(error.to_string().contains("disabled"));
}

#[derive(Default)]
struct RecordingBackend {
    calls: Mutex<Vec<CaptureCommand>>,
}

impl CaptureBackend for RecordingBackend {
    fn capture(
        &self,
        command: &CaptureCommand,
        _settings: &WindowsCaptureSettings,
        _output_path: &Path,
    ) -> anyhow::Result<()> {
        self.calls.lock().expect("calls").push(command.clone());
        Ok(())
    }
}
