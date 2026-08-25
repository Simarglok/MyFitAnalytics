CREATE TABLE IF NOT EXISTS logical_snapshot (
    snapshot_id VARCHAR PRIMARY KEY,
    logical_snapshot_key VARCHAR NOT NULL,
    attempt_id VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL,
    status VARCHAR NOT NULL CHECK (status IN ('committed', 'superseded')),
    UNIQUE (logical_snapshot_key, snapshot_id)
);

CREATE TABLE IF NOT EXISTS active_snapshot (
    logical_snapshot_key VARCHAR PRIMARY KEY,
    snapshot_id VARCHAR NOT NULL,
    attempt_id VARCHAR NOT NULL,
    committed_at TIMESTAMP NOT NULL,
    changed_capabilities JSON NOT NULL,
    record_count UBIGINT NOT NULL,
    FOREIGN KEY (snapshot_id) REFERENCES logical_snapshot(snapshot_id),
    FOREIGN KEY (attempt_id) REFERENCES ingestion_attempt(attempt_id)
);

CREATE INDEX IF NOT EXISTS idx_logical_snapshot_key ON logical_snapshot(logical_snapshot_key);

CREATE OR REPLACE VIEW active_nutrition_items AS
SELECT
    n.nutrition_item_id,
    n.snapshot_id,
    n.logical_snapshot_key,
    n.occurred_local_at,
    n.local_date,
    n.meal,
    n.food_source_id,
    n.name,
    n.amount_raw,
    n.calories_kcal,
    n.protein_g,
    n.fat_g,
    n.carbs_g,
    n.fiber_g,
    n.sugars_g,
    n.sodium_mg,
    n.source_record_id
FROM nutrition_item AS n
JOIN active_snapshot AS a
  ON a.logical_snapshot_key = n.logical_snapshot_key
 AND a.snapshot_id = n.snapshot_id;

CREATE OR REPLACE VIEW active_body_measurements AS
SELECT
    b.body_measurement_id,
    b.snapshot_id,
    b.logical_snapshot_key,
    b.local_date,
    b.weight_kg,
    b.body_fat_pct,
    b.source_record_id
FROM body_measurement AS b
JOIN active_snapshot AS a
  ON a.logical_snapshot_key = b.logical_snapshot_key
 AND a.snapshot_id = b.snapshot_id;

CREATE OR REPLACE VIEW active_activity_events AS
SELECT
    e.activity_event_id,
    e.snapshot_id,
    e.logical_snapshot_key,
    e.occurred_local_at,
    e.local_date,
    e.activity_type,
    e.source_name,
    e.duration_seconds,
    e.distance_km,
    e.estimated_calories_kcal,
    e.origin_hint,
    e.quality_status,
    e.source_record_id
FROM activity_event AS e
JOIN active_snapshot AS a
  ON a.logical_snapshot_key = e.logical_snapshot_key
 AND a.snapshot_id = e.snapshot_id;

CREATE OR REPLACE VIEW active_activity_days AS
SELECT
    d.activity_day_id,
    d.snapshot_id,
    d.logical_snapshot_key,
    d.local_date,
    d.steps,
    d.water_ml,
    d.heart_rate_observation_count,
    d.activity_duration_seconds,
    d.activity_distance_km,
    d.estimated_activity_calories_kcal,
    d.source_record_id
FROM activity_day AS d
JOIN active_snapshot AS a
  ON a.logical_snapshot_key = d.logical_snapshot_key
 AND a.snapshot_id = d.snapshot_id;

CREATE OR REPLACE VIEW active_heart_rate_observations AS
SELECT
    h.heart_rate_observation_id,
    h.snapshot_id,
    h.logical_snapshot_key,
    h.observed_local_at,
    h.heart_rate_bpm,
    h.source_record_id
FROM heart_rate_observation AS h
JOIN active_snapshot AS a
  ON a.logical_snapshot_key = h.logical_snapshot_key
 AND a.snapshot_id = h.snapshot_id;

CREATE OR REPLACE VIEW active_workout_sessions AS
SELECT
    w.workout_session_id,
    w.snapshot_id,
    w.logical_snapshot_key,
    w.title,
    w.started_local_at,
    w.ended_local_at,
    w.duration_seconds,
    w.source_record_group_key,
    w.source_record_id
FROM workout_session AS w
JOIN active_snapshot AS a
  ON a.logical_snapshot_key = w.logical_snapshot_key
 AND a.snapshot_id = w.snapshot_id;

CREATE OR REPLACE VIEW active_exercise_sets AS
SELECT
    e.exercise_set_id,
    e.snapshot_id,
    e.logical_snapshot_key,
    e.workout_session_id,
    e.exercise_title_raw,
    e.exercise_key,
    e.exercise_block_ordinal,
    e.set_index,
    e.set_type,
    e.load_type,
    e.weight_kg,
    e.reps,
    e.duration_seconds,
    e.rpe,
    e.source_record_id
FROM exercise_set AS e
JOIN active_snapshot AS a
  ON a.logical_snapshot_key = e.logical_snapshot_key
 AND a.snapshot_id = e.snapshot_id;

CREATE OR REPLACE VIEW active_phase_events AS
SELECT
    p.phase_event_id,
    p.snapshot_id,
    p.logical_snapshot_key,
    p.event_type,
    p.start_date,
    p.end_date,
    p.description,
    p.exclude_from_tdee,
    p.source_record_id
FROM phase_event AS p
JOIN active_snapshot AS a
  ON a.logical_snapshot_key = p.logical_snapshot_key
 AND a.snapshot_id = p.snapshot_id;
