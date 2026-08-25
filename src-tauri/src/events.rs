use mfa_ingestion::{CoreEvent, IngestionCoordinator};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataChangedEvent {
    pub capabilities: Vec<String>,
    pub dashboards: Vec<String>,
}

pub fn spawn_event_forwarder(app: &AppHandle, coordinator: IngestionCoordinator) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut events = coordinator.subscribe();
        while let Ok(event) = events.recv().await {
            if let CoreEvent::DataChanged {
                capabilities,
                dashboards,
            } = event
            {
                let event = DataChangedEvent {
                    capabilities: capabilities
                        .into_iter()
                        .map(|capability| capability.to_string())
                        .collect(),
                    dashboards: dashboards
                        .into_iter()
                        .map(|dashboard| dashboard.to_string())
                        .collect(),
                };
                let _ = app.emit("data-changed", event);
            }
        }
    });
}
