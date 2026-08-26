use mfa_ingestion::{CoreEvent, IngestionCoordinator};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataChangedEvent {
    pub capabilities: Vec<String>,
    pub dashboards: Vec<String>,
}

pub trait DataChangedSink: Send + Sync {
    fn emit(&self, event: DataChangedEvent);
}

impl<F> DataChangedSink for F
where
    F: Fn(DataChangedEvent) + Send + Sync,
{
    fn emit(&self, event: DataChangedEvent) {
        self(event);
    }
}

struct TauriDataChangedSink {
    app: AppHandle,
}

impl DataChangedSink for TauriDataChangedSink {
    fn emit(&self, event: DataChangedEvent) {
        let _ = self.app.emit("data-changed", event);
    }
}

pub fn tauri_event_sink(app: &AppHandle) -> Arc<dyn DataChangedSink> {
    Arc::new(TauriDataChangedSink { app: app.clone() })
}

pub fn spawn_event_forwarders(
    sink: Arc<dyn DataChangedSink>,
    coordinators: Vec<IngestionCoordinator>,
) {
    for coordinator in coordinators {
        let sink = Arc::clone(&sink);
        tauri::async_runtime::spawn(async move {
            let mut events = coordinator.subscribe();
            while let Ok(event) = events.recv().await {
                if let CoreEvent::DataChanged {
                    capabilities,
                    dashboards,
                } = event
                {
                    sink.emit(DataChangedEvent {
                        capabilities: capabilities
                            .into_iter()
                            .map(|capability| capability.to_string())
                            .collect(),
                        dashboards: dashboards
                            .into_iter()
                            .map(|dashboard| dashboard.to_string())
                            .collect(),
                    });
                }
            }
        });
    }
}
