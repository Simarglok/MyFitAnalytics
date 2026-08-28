# Plan 4 packaged-native manual acceptance

Status: approved acceptance procedure, **NOT RUN** for the current checkout.

This runbook is the current manual acceptance criterion for Plan 4. It is
separate from the automated Rust/web/package gates and from the future XCUITest
follow-up. A run is not passed until an operator completes every applicable
step, records the result, and attaches only safe synthetic screenshots.

## Scope and stop rules

- Test only Plan 4: local ingestion visibility, analytics/dashboard pages,
  source quality, settings, provider selection, phase events, packaging, and
  clean shutdown.
- Plan 5 desktop lifecycle, tray/background behavior, autostart, release
  publishing, notarization, and telemetry are out of scope.
- Do not implement or run XCUITest as part of this procedure.
- Do not install, invoke, configure, or require **CuaDriver**. Do not replace it
  with another third-party foreground-control daemon. The operator may use
  ordinary visible macOS interaction for this manual run only.
- Do not request or grant Accessibility, Screen Recording, Apple Events,
  Developer Tools, or any other macOS permission. If a permission prompt is
  shown, stop, record `BLOCKED: permission prompt`, quit safely if possible,
  restore the profile guard, and obtain explicit approval before changing
  permissions.
- Stop if any real export, private record, personal workspace, credential,
  secret, or private log is selected or observed. Do not copy it into the test
  root and do not attach it to the evidence.

## 1. Inputs and temporary roots

Use only these checked-in synthetic inputs:

| Purpose                                         | Repository fixture                                                     |
| ----------------------------------------------- | ---------------------------------------------------------------------- |
| MyNetDiary successful import                    | `modules/sources/mynetdiary/tests/fixtures/valid-full.xls`             |
| MyNetDiary deterministic failure/retry exercise | `modules/sources/mynetdiary/tests/fixtures/missing-required-sheet.xls` |
| Hevy measurements                               | `modules/sources/hevy/tests/fixtures/measurement_data.csv`             |
| Hevy workouts                                   | `modules/sources/hevy/tests/fixtures/workout_data.csv`                 |

The failure fixture is optional for the happy-path run, but it is required to
exercise bounded/current attention behavior. No other fixture, export, or
record may be used.

From the repository root, execute the setup below in the same shell that will
run the rest of this runbook. Do not paste later snippets into child shells.
The pre-provisioned guard is validated before the acceptance root is created,
and the single `EXIT` handler is installed immediately after the root and
guard variables are established, before any later command can fail:

```bash
set -euo pipefail
export REPO_ROOT="$PWD"
export TMP_BASE="${TMPDIR:-/tmp}"
export TMP_BASE="${TMP_BASE%/}"
: "${PROFILE_GUARD:?set PROFILE_GUARD to a pre-provisioned session-owned helper}"
if [ ! -x "$PROFILE_GUARD" ]; then
  printf '%s\n' 'BLOCKED: pre-provisioned PROFILE_GUARD is unavailable' >&2
  exit 1
fi

ACCEPTANCE_ROOT="$(mktemp -d "$TMP_BASE/mfa-plan4-manual.XXXXXX")"
export ACCEPTANCE_ROOT
export TEST_HOME="$ACCEPTANCE_ROOT/home"
export WORKSPACE="$ACCEPTANCE_ROOT/workspace"
export MYNETDIARY_INBOX="$WORKSPACE/inbox/mynetdiary"
export HEVY_INBOX="$WORKSPACE/inbox/hevy"
export PROFILE_GUARD_ROOT="$ACCEPTANCE_ROOT"
export PROFILE_GUARD_MANIFEST="$ACCEPTANCE_ROOT/profile-guard.tsv"
export PROFILE_GUARD_BACKUP_MANIFEST="$ACCEPTANCE_ROOT/profile-guard-before.tsv"

record_blocked() {
  local reason="$1"
  printf 'BLOCKED: %s\n' "$reason" >&2
  if [ -n "${ACCEPTANCE_ROOT:-}" ] && [ -d "${ACCEPTANCE_ROOT:-}" ]; then
    printf 'BLOCKED\t%s\n' "$reason" \
      >"$ACCEPTANCE_ROOT/cleanup-status.tsv" || true
  fi
}

validate_profile_guard_manifest() {
  local expected_footer="$1"
  python3 - "$PROFILE_GUARD_MANIFEST" "$expected_footer" <<'PY'
import re
import sys

manifest, expected_footer = sys.argv[1:]
labels = [
    "application-support",
    "caches",
    "webkit",
    "preferences",
    "saved-application-state",
    "http-storages",
]


def fail(reason):
    print(f"BLOCKED: profile-guard.tsv {reason}", file=sys.stderr)
    raise SystemExit(1)


try:
    with open(manifest, encoding="utf-8", newline="") as handle:
        lines = handle.read().splitlines()
except OSError:
    fail("cannot be read")

if len(lines) != 8:
    fail("must contain one header, six root rows, and one footer")
if lines[0] != "label\tstate\tdigest\tfiles\trestored":
    fail("header is not exact")

for index, label in enumerate(labels, start=1):
    fields = lines[index].split("\t")
    if len(fields) != 5:
        fail(f"row {index} must have five TSV fields")
    if fields[0] != label:
        fail(f"row {index} has an unexpected label")
    if fields[1] not in {"present", "absent"}:
        fail(f"row {index} has an invalid state")
    if re.fullmatch(r"[0-9a-f]{64}", fields[2]) is None:
        fail(f"row {index} has an invalid lowercase SHA-256 digest")
    if re.fullmatch(r"[0-9]+", fields[3]) is None:
        fail(f"row {index} has an invalid file count")
    if fields[4] != expected_footer:
        fail(f"row {index} has the wrong restored flag")

if lines[-1] != f"restored={expected_footer}":
    fail("footer is not exact")
PY
}

compare_profile_guard_manifests() {
  python3 - "$PROFILE_GUARD_BACKUP_MANIFEST" "$PROFILE_GUARD_MANIFEST" <<'PY'
import sys

before_path, after_path = sys.argv[1:]


def read_rows(path):
    with open(path, encoding="utf-8", newline="") as handle:
        return [line.split("\t") for line in handle.read().splitlines()]


try:
    before = read_rows(before_path)
    after = read_rows(after_path)
except OSError:
    print("BLOCKED: profile-guard backup comparison cannot be read", file=sys.stderr)
    raise SystemExit(1)

if len(before) != 8 or len(after) != 8:
    print("BLOCKED: profile-guard backup comparison has the wrong line count", file=sys.stderr)
    raise SystemExit(1)

for index in range(1, 7):
    if before[index][:4] != after[index][:4]:
        print(
            f"BLOCKED: profile-guard row {index} changed during restore",
            file=sys.stderr,
        )
        raise SystemExit(1)
PY
}

cleanup_acceptance() {
  local original_status="$?"
  local cleanup_status="$original_status"
  trap - EXIT

  finish_blocked() {
    local reason="$1"
    record_blocked "$reason"
    if [ "$cleanup_status" -eq 0 ]; then
      exit 1
    fi
    exit "$cleanup_status"
  }

  if [ -z "${ACCEPTANCE_ROOT:-}" ] || \
     [ -z "${PROFILE_GUARD_MANIFEST:-}" ]; then
    finish_blocked "acceptance root or profile-guard manifest is unset"
  fi
  if [ ! -f "$PROFILE_GUARD_MANIFEST" ]; then
    finish_blocked \
      "profile-guard backup manifest is missing; acceptance root retained"
  fi
  if ! "$PROFILE_GUARD" restore; then
    finish_blocked "profile-guard restore failed; acceptance root retained"
  fi
  if ! validate_profile_guard_manifest true; then
    finish_blocked \
      "profile-guard restore verification failed; acceptance root retained"
  fi
  if [ ! -f "$PROFILE_GUARD_BACKUP_MANIFEST" ]; then
    finish_blocked \
      "profile-guard backup record is missing; acceptance root retained"
  fi
  if ! compare_profile_guard_manifests; then
    finish_blocked \
      "profile-guard digest/state comparison failed; acceptance root retained"
  fi

  if ! test -n "$ACCEPTANCE_ROOT"; then
    finish_blocked "refusing to delete an empty acceptance root"
  fi
  case "$ACCEPTANCE_ROOT" in
    "$TMP_BASE"/mfa-plan4-manual.*) ;;
    *) finish_blocked "refusing to delete an unexpected acceptance root" ;;
  esac
  if ! rm -rf -- "$ACCEPTANCE_ROOT"; then
    finish_blocked "acceptance-root deletion failed; root retained"
  fi
  if [ -e "$ACCEPTANCE_ROOT" ]; then
    finish_blocked "acceptance root still exists after deletion"
  fi
  exit "$original_status"
}

trap cleanup_acceptance EXIT

mkdir -p "$TEST_HOME" "$MYNETDIARY_INBOX" "$HEVY_INBOX"
```

The application must be launched with `HOME="$TEST_HOME"` and the workspace
must be selected only from `$WORKSPACE`. The temporary root is disposable and
must not contain a copy of the user's normal workspace.

## 2. Preflight and profile guard

Before launch:

1. Confirm the checkout and artifact baseline:

   ```bash
   git status --short --untracked-files=all
   git rev-parse HEAD
   node scripts/fixtures/verify_fixture_privacy.mjs
   ```

   The status must be clean for the build being accepted. The fixture verifier
   must report this exact output:
   `verified 7 BIFF fixtures and 2 CSV fixtures; privacy scan passed`.

2. Before the first `PROFILE_GUARD` invocation, configure the pre-provisioned
   session-owned six-root hash-guard helper for `PROFILE_GUARD_ROOT` and
   `PROFILE_GUARD_MANIFEST`. The helper is not a repository dependency and must
   not be improvised during the run. It must protect these labels without
   printing their resolved absolute paths:
   `application-support`, `caches`, `webkit`, `preferences`,
   `saved-application-state`, and `http-storages`.

   The exact `$ACCEPTANCE_ROOT/profile-guard.tsv` contract is defined before
   the first invocation. It is UTF-8 TSV with no blank lines or extra fields:

   - line 1 is exactly
     `label\tstate\tdigest\tfiles\trestored`;
   - lines 2–7 are exactly one row for each label, in this order:
     `application-support`, `caches`, `webkit`, `preferences`,
     `saved-application-state`, `http-storages`;
   - every root row has five fields: the exact label; `state` equal to
     `present` or `absent`; `digest` equal to a lowercase 64-character
     SHA-256 digest of the deterministic root snapshot; `files` equal to a
     non-negative decimal file count; and `restored` equal to the current
     restore state;
   - immediately after a successful `backup`, every root row has
     `restored=false` and the final line is exactly `restored=false`;
   - only after a successful `restore` and post-restore verification may every
     root row change to `restored=true`; the final line must then be exactly
     `restored=true`;
   - `label`, `state`, `digest`, and `files` must remain byte-for-byte equal
     to the retained pre-restore manifest; only the per-row `restored` fields
     and final footer change from `false` to `true`;
   - the file has exactly eight lines total, with no paths, source bytes, or
     other fields. The setup block's
     `validate_profile_guard_manifest` function enforces this contract for
     both footer states.

   The guard backup is session-owned and ephemeral; it must verify the backup
   digest before the application starts. If the helper is absent, cannot be
   configured for this root and manifest, backup fails, the manifest violates
   this contract, or restoration cannot be verified, the single `EXIT` handler
   records `BLOCKED` and retains the acceptance root; do not improvise a
   replacement or change permissions.

   ```bash
   "$PROFILE_GUARD" backup
   validate_profile_guard_manifest false
   cp "$PROFILE_GUARD_MANIFEST" "$PROFILE_GUARD_BACKUP_MANIFEST"
   ```

3. Stage only the synthetic files:

   ```bash
   cp "$REPO_ROOT/modules/sources/mynetdiary/tests/fixtures/valid-full.xls" \
      "$MYNETDIARY_INBOX/valid-full.xls"
   cp "$REPO_ROOT/modules/sources/hevy/tests/fixtures/measurement_data.csv" \
      "$HEVY_INBOX/measurement_data.csv"
   cp "$REPO_ROOT/modules/sources/hevy/tests/fixtures/workout_data.csv" \
      "$HEVY_INBOX/workout_data.csv"
   ```

   Record fixture filenames and the acceptance-root label only. Do not record
   resolved home paths or file contents.

## 3. Fresh package and application verification

Run the package build before opening the app:

```bash
cd "$REPO_ROOT"
command -v duckdb
duckdb --version
bash scripts/build-module-packages.sh
python3 scripts/verify_module_packages.py
shasum -a 256 \
  dist/modules/mynetdiary.mfasource \
  dist/modules/hevy.mfasource \
  dist/modules/base.mfadashboard
```

The checked-in deterministic package hashes expected at this Plan 4 baseline
are:

| Package                | Expected SHA-256                                                   |
| ---------------------- | ------------------------------------------------------------------ |
| `mynetdiary.mfasource` | `79a8c96594a95e508fc5cae95057323528d3f180af9e8f3c25bf472b635fc56c` |
| `hevy.mfasource`       | `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379` |
| `base.mfadashboard`    | `13a11f972e93c8bfd51b6e371fb8cef62f45a0887bb4339b1f0b93badf89d901` |

The verifier must report exactly the three allowlisted production module
packages. Do not add a package to the bundle or edit a generated manifest.

Build and verify the fresh macOS artifacts:

```bash
pnpm --dir web build
cargo tauri build --bundles app
cargo tauri build --bundles dmg -vv

export APP="$REPO_ROOT/target/release/bundle/macos/MyFitAnalytics.app"
export BIN="$APP/Contents/MacOS/myfitanalytics"
export DMG="$REPO_ROOT/target/release/bundle/dmg/MyFitAnalytics_0.1.0_aarch64.dmg"
test -x "$BIN"
test -f "$DMG"
shasum -a 256 "$BIN" "$DMG"
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -d --entitlements :- "$APP"
hdiutil verify "$DMG"
```

Record the fresh binary and DMG hashes exactly as printed. The prior baseline
values are reference only: executable
`0b66ca53e055dc6101815e9b7516689c44f12a51fa7d492fa88735894812a611` and DMG
`45a859450ebcb890c22bd91ac681955ad7363ccdd1d637f1370d048b9a64fdc4`. A new
manual run must use its own fresh output and must not copy these values if the
artifacts differ.

## 4. Launch and initial state

Launch the exact packaged executable directly with the isolated home. Do not
use Finder automation or a controller daemon:

```bash
HOME="$TEST_HOME" "$BIN" >"$ACCEPTANCE_ROOT/app.stdout" 2>"$ACCEPTANCE_ROOT/app.stderr" &
export APP_PID=$!
```

Use ordinary visible macOS interaction to inspect the window. The first window
must be titled `MyFitAnalytics`, approximately 1200 x 800, and remain visible.
Only record whether the window is visible and whether a crash occurred. Never
attach raw stdout, stderr, WebKit logs, or paths; delete those files during
cleanup.

Expected initial state on a fresh profile:

- top-level navigation contains `Overview`, `Body`, `Nutrition`, `Activity`,
  `Strength`, `Sources & quality`, `Phase events`, and `Settings`;
- status is `Healthy` or `Not configured` until a workspace is selected;
- no raw JSON, `[[object Object]]`, source path, or private data is visible;
- Overview is not falsely `Ready` before data is imported.

## 5. Configure the synthetic workspace and Settings

Perform these UI steps exactly:

1. Open `Settings` from the top-level Analytics navigation.
2. Activate `Choose Workspace...` and select only the fresh `$WORKSPACE`.
3. For the MyNetDiary source, activate `Choose inbox` and select only
   `$MYNETDIARY_INBOX`.
4. For the Hevy source, activate `Choose inbox` and select only
   `$HEVY_INBOX`.
5. Confirm that Settings displays the workspace and both source inbox choices
   without exposing source bytes or unrelated profile locations.
6. Confirm that the installed bundled source modules and base dashboard are
   visible. Confirm the active provider control is explicit; do not infer a
   provider merely because a capability is present.
7. If an installed module shows `Update`, click only that module's explicit
   `Update` control and wait for the catalog to reload. Confirm that the module
   visibly shows the selected bundled version and its selected/active state;
   record the exact version and state text shown by the UI. Do not assert that
   the UI displays a SHA-256 hash: package SHA-256 verification is separate
   evidence in Section 3. Confirm that unrelated modules, disabled modules,
   custom packages, and provider selections are unchanged.
   Never perform a global or automatic update.
8. If no bundled module shows `Update` at this fresh artifact baseline, record
   `N/A — no update candidate advertised` and rely on the automated explicit
   update tests; do not claim that an update was exercised.
9. If the module catalog reports `Incompatible`, `Error`, or an unexpected
   selected version/state, stop the acceptance run and record the typed state.

## 6. Initial dashboard state and no-restart ingestion visibility

Before the first refresh, return to `Overview` and record the non-ready state:
`Waiting for data` or the exact typed non-ready state. Do not refresh the app or
restart it between configuration and ingestion.

1. Activate `Refresh data` once.
2. While the operation is active, observe `Refreshing data…` and, if visible,
   a positive active-job count.
3. Without restarting the app, wait for the data-change refresh to complete.
4. Expected result: the same process updates to `Healthy`, `0 active jobs`,
   `0 attention items`; the three synthetic inbox files are consumed and
   archived; the dashboard and navigation metadata update without a restart.
5. Expected initial observation is `2026-02-03` from the checked-in fixtures.
   The initial range must be the inclusive backend-owned 31-day range
   `2026-01-04` through `2026-02-03`, not a frontend-invented date.
6. Confirm that no source path, raw export row, SQL, credential, or raw JSON is
   rendered anywhere in the status or dashboard.

## 7. Refresh idempotence and actionable current attention

This section is performed while the app remains open. It uses only the checked-
in synthetic failure fixture.

1. Copy `modules/sources/mynetdiary/tests/fixtures/missing-required-sheet.xls`
   into `$MYNETDIARY_INBOX/missing-required-sheet.xls`.
2. Activate `Refresh data` and wait until active jobs return to zero.
3. Expected result: status is `Attention` (or the exact typed import-error
   state), the current attention count is one for this failed asset, and the
   displayed failure-code count is deterministic. The reason/identity must be
   privacy-safe and must not expose the absolute inbox/archive path or source
   bytes.
4. Activate `Refresh data` again without changing the failed synthetic file.
5. Expected result: the same current attention item remains one item; the
   count and failure-code count do not grow to two. This is the idempotence
   check. The app must not create duplicate current attention solely because
   Refresh was repeated.
6. Open `Sources & quality`. Confirm the failed asset has a visible `Retry
import` action and that retry returns a typed queued/result state rather than
   raw error text. If the fixture remains invalid, record the expected typed
   failure and do not claim recovery.
7. Confirm `Open settings` is offered only for the applicable module-update
   condition, not as a substitute for the failure reason. Do not claim success
   clearing for an artifact that cannot be repaired through the checked-in
   fixture flow; the successful-clear invariant is covered by the automated
   ingestion correction test.

## 8. Dashboard pages, gaps, provider, and phase events

From the same running app, visit every page in this order and record the title,
availability state, visible gap/readiness message, and absence of raw payloads:

| Page                | Expected checks                                                                                                                                       |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Overview`          | title, body-weight/nutrition/trend/quality blocks, coverage and freshness; insufficient coverage is explicit and never represented as invented zeroes |
| `Body`              | raw weights, daily median, trailing mean, trend, optional body-fat series, phase overlay, and status; nulls remain gaps                               |
| `Nutrition`         | calories, macros, trailing mean, TDEE, trend, quality, and missing/partial-day coverage                                                               |
| `Activity`          | steps, events, heart rate, water, trend, null-preserving gaps, and status                                                                             |
| `Strength`          | session windows, duration, working sets, e1RM, calendar, governed exercise keys, and status                                                           |
| `Sources & quality` | module states, provider choices, active snapshots, quality/retry surface, and status                                                                  |

For the imported fixture baseline, compare semantic values with
`web/e2e/fixtures/expected-dashboard.json`: body weights `81.4` and `81.1`
kg, trailing mean `81.25` kg, nutrition item count `2`, activity steps
`6400`, accepted event count `1`, and strength working sets `2`. Record only
semantic values and state; do not copy raw fixture rows into evidence.

Provider and phase-event steps:

1. In `Settings`, use the explicit provider control for `body.weight` and
   select `hevy` if the control offers that choice. Confirm the selected
   provider is rendered after the settings reload and that no unselected offer
   becomes active implicitly.
2. Open `Phase events`, create a synthetic event with type `cut`, dates
   `2026-01-15` through `2026-01-16`, description `synthetic phase`, and
   `Exclude from TDEE` checked. Save it.
3. Confirm the event appears in the list without a restart. Return to `Overview`
   or `Nutrition` and confirm the phase overlay and TDEE excluded-day state
   update after the fresh command query.
4. Exercise cancel on the delete confirmation if presented; confirm the event
   remains. Then explicitly confirm deletion and verify it disappears. Record
   only the event type/date/result, not database paths or private text.

## 9. Clean quit and read-only database checks

1. Quit the packaged app through its visible application quit control.
2. Wait for the exact packaged process to exit. Verify only the acceptance app
   process, not unrelated processes:

   ```bash
   pgrep -f "$BIN" || true
   ```

   Expected result is no matching packaged process. Do not kill an unrelated
   process; if the exact process remains, record `FAIL: process remains` and
   stop.

3. Locate the isolated application database under the test profile without
   printing the resolved path. Open it read-only with DuckDB:

   ```bash
   command -v duckdb
   duckdb --version
   export DB_PATH="$TEST_HOME/Library/Application Support/com.simarglok.myfitanalytics/myfitanalytics.duckdb"
   test -f "$DB_PATH"
   duckdb -readonly "$DB_PATH" -c "PRAGMA database_size;"
   duckdb -readonly "$DB_PATH" -c "
   SELECT table_name, row_count
   FROM (
     SELECT 'source_asset' table_name, COUNT(*) row_count FROM source_asset
     UNION ALL SELECT 'source_receipt', COUNT(*) FROM source_receipt
     UNION ALL SELECT 'ingestion_attempt', COUNT(*) FROM ingestion_attempt
     UNION ALL SELECT 'logical_snapshot', COUNT(*) FROM logical_snapshot
     UNION ALL SELECT 'active_snapshot', COUNT(*) FROM active_snapshot
     UNION ALL SELECT 'nutrition_item', COUNT(*) FROM nutrition_item
     UNION ALL SELECT 'body_measurement', COUNT(*) FROM body_measurement
     UNION ALL SELECT 'activity_day', COUNT(*) FROM activity_day
     UNION ALL SELECT 'heart_rate_observation', COUNT(*) FROM heart_rate_observation
     UNION ALL SELECT 'workout_session', COUNT(*) FROM workout_session
     UNION ALL SELECT 'exercise_set', COUNT(*) FROM exercise_set
     UNION ALL SELECT 'user_phase_event', COUNT(*) FROM user_phase_event
   ) ORDER BY table_name;"
   duckdb -readonly "$DB_PATH" -c "
   SELECT COUNT(*) AS broken_active_snapshots
   FROM active_snapshot a
   LEFT JOIN logical_snapshot l ON l.logical_snapshot_key = a.logical_snapshot_key
                                AND l.snapshot_id = a.snapshot_id
   WHERE l.snapshot_id IS NULL;"
   ```

   The final query must return `0`. After the happy-path import and one saved
   phase event, the expected aggregate counts are `3` each for
   `source_asset`, `source_receipt`, `ingestion_attempt`, `logical_snapshot`,
   and `active_snapshot`; `2` nutrition items; `2` body measurements; `1`
   activity day; `1` heart-rate observation; `1` workout session; `4` exercise
   sets; and `0` user phase events after the confirmed delete in Section 8.
   If the failure exercise creates an additional receipt/attempt, record the
   actual count and explain it; never delete rows to force the expected result.

   All database checks are read-only. Do not run `CHECKPOINT`, `DELETE`, `UPDATE`,
   `VACUUM`, or any recovery command against the acceptance database.

## 10. Cleanup and privacy verification

The `cleanup_acceptance` `EXIT` handler installed in Section 1 is the sole
cleanup path. It is installed before directory creation, profile backup, app
launch, or any other later command that runs with `set -e`; therefore both a
normal `exit 0` and an early failure enter the same handler. Do not add another
trap, invoke `restore` manually, clear the trap elsewhere, or run a second
`rm -rf` sequence.

On every exit, the handler performs this order:

1. retain the original exit status and disable only its own trap to prevent
   recursion;
2. require the acceptance root and profile-guard manifest to be set and
   require the manifest to exist; otherwise emit and record `BLOCKED` and
   retain the root;
3. invoke the guard's `restore`, then run
   `validate_profile_guard_manifest true`, which requires every root row and
   the exact `restored=true` footer; if either step fails, emit and record
   `BLOCKED` and retain the root;
4. run the existing non-empty acceptance-root check and the exact
   `$TMP_BASE/mfa-plan4-manual.*` prefix check;
5. only after the successful restore/verification and both path checks, delete
   the acceptance root and verify that it no longer exists; a deletion or
   absence-check failure emits and records `BLOCKED` and does not claim cleanup.

If the guard backup fails before producing a manifest, the handler does not
attempt an unverified restore; it records `BLOCKED` and retains the root. Any
other early failure with a manifest present still follows restore, manifest
verification, path checks, and conditional deletion in that order.

Before finalizing the record:

- verify the retained acceptance record contains the already-completed profile
  guard result: every root `restored=true` and exact before/after digest
  equality. The manifest itself is deleted with the temporary root.
- verify the exact packaged app process is absent;
- delete stdout/stderr, database copies, archive bytes, screenshots not being
  attached, and all temporary fixture copies;
- verify the normal workspace and profile were not selected or modified;
- scan the acceptance record and attachments for home paths, source paths,
  exports, health records, credentials, secrets, raw rows, and private logs;
- retain only command exit codes, deterministic hashes, counts, state labels,
  semantic aggregates, safe screenshots, and the pass/fail table below.

## 11. Pass/fail record

A reviewer must complete this record from the same run. `NOT RUN` is the only
valid value before a real manual execution.

| Check                                               | Result (`PASS`/`FAIL`/`N/A`/`BLOCKED`) | Evidence reference |
| --------------------------------------------------- | -------------------------------------- | ------------------ |
| Preflight and fixture privacy                       | `NOT RUN`                              |                    |
| Fresh module package hashes and allowlist           | `NOT RUN`                              |                    |
| Fresh app/DMG hashes, signing, and DMG verification | `NOT RUN`                              |                    |
| Profile backup and hash-identical restore           | `NOT RUN`                              |                    |
| Fresh visible packaged launch                       | `NOT RUN`                              |                    |
| Settings/workspace/inbox configuration              | `NOT RUN`                              |                    |
| No-restart ingestion visibility                     | `NOT RUN`                              |                    |
| Refresh idempotence/current attention               | `NOT RUN`                              |                    |
| Explicit bundled update behavior                    | `NOT RUN`                              |                    |
| All dashboard pages and semantic gaps               | `NOT RUN`                              |                    |
| Explicit provider selection                         | `NOT RUN`                              |                    |
| Phase event save/overlay/delete                     | `NOT RUN`                              |                    |
| Clean quit and no packaged process                  | `NOT RUN`                              |                    |
| Read-only DB integrity and aggregates               | `NOT RUN`                              |                    |
| Cleanup/privacy boundary                            | `NOT RUN`                              |                    |

Overall status for the current checkout: **NOT RUN**. Do not convert this to
`PASS` from the automated lower-level gates alone. A permission prompt,
non-synthetic input, profile-guard mismatch, raw/private data exposure, or
unexplained artifact/hash mismatch is a stop condition and must be recorded as
`BLOCKED` or `FAIL` with no claim of Plan 4 native acceptance.

After completing the pass/fail record, execute this as the final command in the
same shell so the sole handler performs the verified cleanup:

```bash
exit 0
```
