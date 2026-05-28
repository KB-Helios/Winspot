use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

pub type UsageSnapshot = HashMap<String, UsageSignal>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEvent {
    pub result_id: String,
    pub timestamp_unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UsageSignal {
    pub launch_count: u32,
    pub last_used_unix_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct UsageStore {
    path: PathBuf,
}

impl UsageStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn record(&self, event: UsageEvent) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, &event)?;
        file.write_all(b"\n")?;
        file.flush()
    }

    pub fn load_snapshot(&self) -> std::io::Result<UsageSnapshot> {
        let Ok(file) = fs::File::open(&self.path) else {
            return Ok(UsageSnapshot::default());
        };

        let mut snapshot = UsageSnapshot::default();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let Ok(event) = serde_json::from_str::<UsageEvent>(&line) else {
                continue;
            };

            let signal = snapshot.entry(event.result_id).or_default();
            signal.launch_count = signal.launch_count.saturating_add(1);
            signal.last_used_unix_seconds = signal
                .last_used_unix_seconds
                .max(event.timestamp_unix_seconds);
        }

        Ok(snapshot)
    }
}
