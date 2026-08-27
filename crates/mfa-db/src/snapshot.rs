use crate::command::{CreatePhaseEvent, DeletePhaseEvent, UpdatePhaseEvent};
use crate::error::DatabaseError;
use chrono::Utc;
use duckdb::{Connection, params};
use mfa_contracts::{LocalDate, PhaseEvent};

pub(crate) fn create_phase_event(
    connection: &Connection,
    command: CreatePhaseEvent,
) -> Result<PhaseEvent, DatabaseError> {
    validate(&command.phase_event)?;
    let phase_event = command.phase_event;
    connection
        .execute(
            "INSERT INTO user_phase_event(
                phase_event_id, event_type, start_date, end_date, description,
                exclude_from_tdee, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                phase_event.phase_event_id.to_string(),
                &phase_event.event_type,
                phase_event.start_date.as_naive(),
                phase_event.end_date.as_naive(),
                &phase_event.description,
                phase_event.exclude_from_tdee,
                Utc::now().naive_utc(),
                Utc::now().naive_utc(),
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;
    Ok(phase_event)
}

pub(crate) fn update_phase_event(
    connection: &Connection,
    command: UpdatePhaseEvent,
) -> Result<PhaseEvent, DatabaseError> {
    validate(&command.phase_event)?;
    let phase_event = command.phase_event;
    let updated = connection
        .execute(
            "UPDATE user_phase_event
             SET event_type = ?, start_date = ?, end_date = ?, description = ?,
                 exclude_from_tdee = ?, updated_at = ?
             WHERE phase_event_id = ?",
            params![
                &phase_event.event_type,
                phase_event.start_date.as_naive(),
                phase_event.end_date.as_naive(),
                &phase_event.description,
                phase_event.exclude_from_tdee,
                Utc::now().naive_utc(),
                phase_event.phase_event_id.to_string(),
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;
    if updated != 1 {
        return Err(DatabaseError::PhaseEventNotFound {
            phase_event_id: phase_event.phase_event_id,
        });
    }
    Ok(phase_event)
}

pub(crate) fn delete_phase_event(
    connection: &Connection,
    command: DeletePhaseEvent,
) -> Result<bool, DatabaseError> {
    let deleted = connection
        .execute(
            "DELETE FROM user_phase_event WHERE phase_event_id = ?",
            params![command.phase_event_id.to_string()],
        )
        .map_err(DatabaseError::from_duckdb)?;
    Ok(deleted == 1)
}

pub(crate) fn list_phase_events(connection: &Connection) -> Result<Vec<PhaseEvent>, DatabaseError> {
    let mut statement = connection
        .prepare(
            "SELECT phase_event_id, event_type, start_date, end_date, description,
                    exclude_from_tdee
             FROM user_phase_event
             ORDER BY start_date, end_date, phase_event_id",
        )
        .map_err(DatabaseError::from_duckdb)?;
    let rows = statement
        .query_map([], |row| {
            let phase_event_id: String = row.get(0)?;
            let start_date: chrono::NaiveDate = row.get(2)?;
            let end_date: chrono::NaiveDate = row.get(3)?;
            Ok((
                phase_event_id,
                row.get::<_, String>(1)?,
                start_date,
                end_date,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, bool>(5)?,
            ))
        })
        .map_err(DatabaseError::from_duckdb)?;
    rows.map(|row| {
        let (phase_event_id, event_type, start_date, end_date, description, exclude_from_tdee) =
            row.map_err(DatabaseError::from_duckdb)?;
        let phase_event_id = phase_event_id
            .parse()
            .map_err(|error| DatabaseError::Command {
                detail: format!("stored phase event identity is invalid: {error}"),
            })?;
        Ok(PhaseEvent {
            phase_event_id,
            event_type,
            start_date: LocalDate::from(start_date),
            end_date: LocalDate::from(end_date),
            description,
            exclude_from_tdee,
        })
    })
    .collect()
}

fn validate(phase_event: &PhaseEvent) -> Result<(), DatabaseError> {
    if phase_event.event_type.trim().is_empty() {
        return Err(DatabaseError::Validation {
            code: "invalid_phase_event",
            detail: "event type cannot be blank".to_owned(),
        });
    }
    if phase_event.start_date > phase_event.end_date {
        return Err(DatabaseError::Validation {
            code: "invalid_phase_event",
            detail: "phase event start date cannot be after end date".to_owned(),
        });
    }
    Ok(())
}
