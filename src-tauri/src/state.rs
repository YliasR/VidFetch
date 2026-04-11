// Fields are consumed once Phase 5 (queue + pause/resume) lands.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::task::JoinHandle;

pub struct JobHandle {
    pub task: JoinHandle<()>,
    pub child_id: Option<u32>,
}

#[derive(Default)]
pub struct AppState {
    pub jobs: Mutex<HashMap<String, JobHandle>>,
}
