CREATE TABLE IF NOT EXISTS nutrition_item (
    nutrition_item_id VARCHAR PRIMARY KEY,
    snapshot_id VARCHAR NOT NULL,
    logical_snapshot_key VARCHAR NOT NULL,
    occurred_local_at TIMESTAMP,
    local_date DATE NOT NULL,
    meal VARCHAR NOT NULL,
    food_source_id VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    amount_raw VARCHAR NOT NULL,
    calories_kcal DOUBLE,
    protein_g DOUBLE,
    fat_g DOUBLE,
    carbs_g DOUBLE,
    fiber_g DOUBLE,
    sugars_g DOUBLE,
    sodium_mg DOUBLE,
    source_record_id VARCHAR NOT NULL,
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE TABLE IF NOT EXISTS body_measurement (
    body_measurement_id VARCHAR PRIMARY KEY,
    snapshot_id VARCHAR NOT NULL,
    logical_snapshot_key VARCHAR NOT NULL,
    local_date DATE NOT NULL,
    weight_kg DOUBLE NOT NULL CHECK (weight_kg > 0),
    body_fat_pct DOUBLE,
    source_record_id VARCHAR NOT NULL,
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE TABLE IF NOT EXISTS activity_event (
    activity_event_id VARCHAR PRIMARY KEY,
    snapshot_id VARCHAR NOT NULL,
    logical_snapshot_key VARCHAR NOT NULL,
    occurred_local_at TIMESTAMP NOT NULL,
    local_date DATE NOT NULL,
    activity_type VARCHAR NOT NULL,
    source_name VARCHAR NOT NULL,
    duration_seconds UINTEGER,
    distance_km DOUBLE,
    estimated_calories_kcal DOUBLE,
    origin_hint VARCHAR,
    quality_status VARCHAR NOT NULL CHECK (quality_status IN ('accepted', 'unknown_mapping', 'parse_warning')),
    source_record_id VARCHAR NOT NULL,
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE TABLE IF NOT EXISTS activity_day (
    activity_day_id VARCHAR PRIMARY KEY,
    snapshot_id VARCHAR NOT NULL,
    logical_snapshot_key VARCHAR NOT NULL,
    local_date DATE NOT NULL,
    steps UBIGINT,
    water_ml DOUBLE,
    heart_rate_observation_count UINTEGER NOT NULL,
    activity_duration_seconds UBIGINT NOT NULL,
    activity_distance_km DOUBLE NOT NULL,
    estimated_activity_calories_kcal DOUBLE NOT NULL,
    source_record_id VARCHAR NOT NULL,
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE TABLE IF NOT EXISTS heart_rate_observation (
    heart_rate_observation_id VARCHAR PRIMARY KEY,
    snapshot_id VARCHAR NOT NULL,
    logical_snapshot_key VARCHAR NOT NULL,
    observed_local_at TIMESTAMP NOT NULL,
    heart_rate_bpm DOUBLE NOT NULL CHECK (heart_rate_bpm > 0),
    source_record_id VARCHAR NOT NULL,
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE TABLE IF NOT EXISTS workout_session (
    workout_session_id VARCHAR PRIMARY KEY,
    snapshot_id VARCHAR NOT NULL,
    logical_snapshot_key VARCHAR NOT NULL,
    title VARCHAR NOT NULL,
    started_local_at TIMESTAMP NOT NULL,
    ended_local_at TIMESTAMP NOT NULL,
    duration_seconds UINTEGER,
    source_record_group_key VARCHAR NOT NULL,
    source_record_id VARCHAR NOT NULL,
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE TABLE IF NOT EXISTS exercise_set (
    exercise_set_id VARCHAR PRIMARY KEY,
    snapshot_id VARCHAR NOT NULL,
    logical_snapshot_key VARCHAR NOT NULL,
    workout_session_id VARCHAR NOT NULL,
    exercise_title_raw VARCHAR NOT NULL,
    exercise_key VARCHAR NOT NULL,
    exercise_block_ordinal UINTEGER NOT NULL,
    set_index UINTEGER NOT NULL,
    set_type VARCHAR NOT NULL,
    load_type VARCHAR NOT NULL,
    weight_kg DOUBLE,
    reps UINTEGER,
    duration_seconds UINTEGER,
    rpe DOUBLE,
    source_record_id VARCHAR NOT NULL,
    FOREIGN KEY (workout_session_id) REFERENCES workout_session(workout_session_id),
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE TABLE IF NOT EXISTS phase_event (
    phase_event_id VARCHAR PRIMARY KEY,
    snapshot_id VARCHAR NOT NULL,
    logical_snapshot_key VARCHAR NOT NULL,
    event_type VARCHAR NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    description VARCHAR,
    exclude_from_tdee BOOLEAN NOT NULL,
    source_record_id VARCHAR NOT NULL,
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE INDEX IF NOT EXISTS idx_nutrition_snapshot ON nutrition_item(logical_snapshot_key, snapshot_id);
CREATE INDEX IF NOT EXISTS idx_body_snapshot ON body_measurement(logical_snapshot_key, snapshot_id);
CREATE INDEX IF NOT EXISTS idx_activity_snapshot ON activity_event(logical_snapshot_key, snapshot_id);
CREATE INDEX IF NOT EXISTS idx_activity_day_snapshot ON activity_day(logical_snapshot_key, snapshot_id);
CREATE INDEX IF NOT EXISTS idx_heart_rate_snapshot ON heart_rate_observation(logical_snapshot_key, snapshot_id);
CREATE INDEX IF NOT EXISTS idx_workout_snapshot ON workout_session(logical_snapshot_key, snapshot_id);
CREATE INDEX IF NOT EXISTS idx_set_snapshot ON exercise_set(logical_snapshot_key, snapshot_id);
CREATE INDEX IF NOT EXISTS idx_phase_snapshot ON phase_event(logical_snapshot_key, snapshot_id);
