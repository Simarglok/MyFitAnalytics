use mfa_contracts::{LocalDate, PhaseEvent};
use mfa_db::{
    CreatePhaseEvent, DatabaseService, DeletePhaseEvent, ListPhaseEvents, UpdatePhaseEvent,
};
use std::str::FromStr;
use tempfile::TempDir;
use uuid::Uuid;

fn date(value: &str) -> LocalDate {
    LocalDate::from_str(value).unwrap()
}

fn event(id: u128, start: &str, end: &str, exclude_from_tdee: bool) -> PhaseEvent {
    PhaseEvent {
        phase_event_id: Uuid::from_u128(id),
        event_type: format!("phase-{id}"),
        start_date: date(start),
        end_date: date(end),
        description: Some(format!("description-{id}")),
        exclude_from_tdee,
    }
}

#[tokio::test]
async fn user_phase_events_round_trip_through_create_update_delete_and_restart() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("phase-events.duckdb");
    let service = DatabaseService::start(&path, 8).await.unwrap();
    let first = event(1, "2026-01-10", "2026-01-12", true);
    let overlapping = event(2, "2026-01-12", "2026-01-15", true);
    let overlay = event(3, "2026-01-11", "2026-01-14", false);

    for phase_event in [first.clone(), overlapping.clone(), overlay.clone()] {
        let created = service
            .execute(CreatePhaseEvent {
                phase_event: phase_event.clone(),
            })
            .await
            .unwrap();
        assert_eq!(created, phase_event);
    }

    let mut updated = first.clone();
    updated.end_date = date("2026-01-13");
    updated.description = Some("updated description".to_owned());
    assert_eq!(
        service
            .execute(UpdatePhaseEvent {
                phase_event: updated.clone(),
            })
            .await
            .unwrap(),
        updated
    );
    assert!(
        service
            .execute(DeletePhaseEvent {
                phase_event_id: overlay.phase_event_id,
            })
            .await
            .unwrap()
    );
    assert!(
        !service
            .execute(DeletePhaseEvent {
                phase_event_id: Uuid::from_u128(99),
            })
            .await
            .unwrap()
    );

    assert_eq!(
        service.execute(ListPhaseEvents).await.unwrap(),
        vec![updated.clone(), overlapping.clone()]
    );
    let mut invalid = overlapping.clone();
    invalid.start_date = invalid.end_date;
    invalid.end_date = date("2026-01-01");
    let error = service
        .execute(CreatePhaseEvent {
            phase_event: invalid,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_phase_event");
    service.shutdown().await.unwrap();

    let restarted = DatabaseService::start(&path, 8).await.unwrap();
    assert_eq!(
        restarted.execute(ListPhaseEvents).await.unwrap(),
        vec![updated, overlapping]
    );
    restarted.shutdown().await.unwrap();
}
