CREATE TABLE IF NOT EXISTS user_phase_event (
    phase_event_id VARCHAR PRIMARY KEY,
    event_type VARCHAR NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    description VARCHAR,
    exclude_from_tdee BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (start_date <= end_date)
);

CREATE INDEX IF NOT EXISTS idx_user_phase_event_dates
    ON user_phase_event(start_date, end_date, phase_event_id);
