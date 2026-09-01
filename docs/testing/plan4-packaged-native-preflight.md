# Plan 4 packaged-native manager preflight and profile guard

Status for the fixed-source UI retest: **NOT RUN/PENDING**. This document is the
manager-only technical procedure. The prior manager-recorded P6 native run
against the pre-fix package is **FAIL** for stale shell state after Settings
mutations and for rejected Retry hiding the quality row; it must not be
rewritten as a pass. The fixed-source retest has not run and is not authorized
by this document alone.

That failed run nevertheless recorded successful bounded observations: fixture
import, Overview coverage `4/31`, stable `Attention` equal to 1 after repeated
Refresh, one visible Data quality failure row, normal application Quit/process
absence, retained synthetic aggregate checks, and exact six-root guarded
restoration. The remaining analytics pages and Phase events CRUD were not
completed. Fresh retest staging at
`/private/tmp/mfa-plan4-ui-retest.aWTdix` remains unused. Full details are in
`/private/tmp/mfa-plan4-native-approved.P6u51Q/manager-evidence.md`.

Automatic source/web verification is **PASS** across two ledgers: the original
`gates.tsv` records 21 rows through `workspace` with exit 0, and
`continuation-gates.tsv` records 12 rows from `rustfmt` through
`diff-unchanged` with exit 0. The original `disk-before-rustfmt` exit 70 remains
preserved as a separate interrupted attempt. The manager's HEAD and product/test
diff invariants passed. Separate app/DMG package-gate, DMG audit,
correction-round hygiene, and corrected native retest remain **PENDING**/
**NOT RUN**. Cleanup evidence records first only 19 validated rebuildable Mach-O
test executables and later only the isolated worktree's regenerable
`target/debug/build` cache; no sources, dependencies, profiles, backups,
release artifacts, or logs were removed.

The checked-in guard is `scripts/profile_guard.py`. It never launches the app,
never uses CuaDriver, and never recursively deletes a profile, test, backup,
master, holding, or recovery tree. The old external profile helper and runner
remain disabled and retained outside this repository for historical audit only;
they are not an implementation dependency and must not be invoked.

## Hard boundaries

1. Finish the package build and all artifact verification before any profile
   access. A package made before the Settings hydration or guard corrections is
   stale for native acceptance and must not be reported as evidence for this
   product.
2. Use the existing OS user and isolate only the six fixed roots with the
   helper. Do not make the profile appear isolated by overriding `HOME`,
   patching a manifest, patching a database, or copying profile contents into
   the test roots.
3. Native mode derives the real OS home with the account database and derives
   the six fixed MyFitAnalytics roots from the checked-in bundle identifier.
   It does not accept live-root, root-label, process-name, or recovery-path
   overrides. If the isolated execution context cannot provide a real OS home
   for the test user, stop and leave native acceptance **PENDING**.
4. The helper checks the exact `myfitanalytics` executable across the complete
   process listing and fails closed if process inspection fails or returns
   malformed data. A stale or guessed PID is not the absence proof.
5. Do not start the app until `capture` and `isolate` both print `PASS`. Do not
   run `restore` until the app has quit normally and the exact process check is
   clear.
6. Any guard failure, interruption, occupied destination, changed source,
   malformed manifest/journal, incomplete copy/move, or failed post-check is
   `BLOCKED`, never `PASS`. Retain the recovery scope for review; do not repair
   it by deletion.

## Fixed native roots

The native adapter uses the following canonical order and paths for the real OS
home. The bundle identifier is fixed to `com.simarglok.myfitanalytics` from
`src-tauri/tauri.conf.json`; the app display name is `MyFitAnalytics` and the
native executable name is `myfitanalytics`.

| Canonical label           | Fixed relative root                                                       |
| ------------------------- | ------------------------------------------------------------------------- |
| `application-support`     | `Library/Application Support/com.simarglok.myfitanalytics`                |
| `caches`                  | `Library/Caches/com.simarglok.myfitanalytics`                             |
| `saved-application-state` | `Library/Saved Application State/com.simarglok.myfitanalytics.savedState` |
| `httpstorages`            | `Library/HTTPStorages/com.simarglok.myfitanalytics`                       |
| `webkit`                  | `Library/WebKit/com.simarglok.myfitanalytics`                             |
| `preferences`             | `Library/Preferences/com.simarglok.myfitanalytics.plist`                  |

`preferences` is a regular file root, not a directory. A present directory or
regular-file root is supported; an absent root is represented explicitly. The
root basename is not part of a tree digest, so renaming a root without changing
its contents or metadata cannot change the digest.

## Recovery scope and native invocation

Create a new session-owned temporary recovery scope before the first helper
call. The scope must be outside the real home and must not contain a `Library`
path component. Create the marker with the exact content shown below. Do not
reuse an old scope or overwrite an existing recovery destination.

```bash
RECOVERY_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/mfa-plan4-guard.XXXXXX")"
printf '%s\n' 'MFA_PROFILE_GUARD_SYNTHETIC_V1' > "$RECOVERY_ROOT/.mfa-profile-guard-scope"
GUARD=(python3 scripts/profile_guard.py)
```

The following are the only operational native invocations. Run them from the
repository root and preserve their stdout, exit status, and the recovery scope
for the manager evidence. The helper creates `masters/`, `original-holding/`,
`variant-holding/`, `manifest.json`, and `journal.jsonl` below the recovery
scope; none may be moved into the live profile.

### 1. Capture a new baseline and independent masters

Run this only after the fresh packaged artifact and its checks are complete,
while the exact app process is absent:

```bash
"${GUARD[@]}" capture --native --recovery-root "$RECOVERY_ROOT"
```

`PASS` requires all six roots to be inspected, every present root to be copied
to an independent master, every copy to verify against its source, and every
source to remain unchanged across the copy. A failure leaves any partial
masters/journal for audit and cannot be retried by overwriting them; create a
new scope after the cause is understood.

### 2. Isolate originals before any app launch

Run only after capture passes:

```bash
"${GUARD[@]}" isolate --native --recovery-root "$RECOVERY_ROOT"
```

This phase verifies all masters again, verifies all still-live baseline sources
again, then moves each present original root into `original-holding`. It leaves
all six protected live roots absent. It does not copy the originals or masters
back into live. The app must therefore start against a fresh empty profile
surface; the retained original tree is not a test profile and must never be
used as one.

The journal records each original move durably before the phase can report
completion. If the phase is interrupted, rerun the same `isolate` command only
after resolving the reported blocker. A known journaled partial move may be
resumed; an occupied or ambiguous destination is a hard block. No cleanup or
rollback deletion is attempted.

### 3. Run the packaged app and visible UI procedure

Only after both phases pass, launch the current packaged artifact through the
normal manager-controlled installation/launch path. The helper does not launch
it. Perform the visible actions in
[plan4-packaged-native-manual.md](plan4-packaged-native-manual.md) in order.

The manual UI procedure must not be started if the package is stale, the exact
six roots were not isolated for this run, or the mandatory normal-install update
scenario is not prepared. Native acceptance remains
**NOT RUN** in each case.

### 4. Quit and retain post-run variants

After the app quits through its normal visible quit control, verify that the
exact app process is absent. Do not edit any root before the following helper
call:

```bash
"${GUARD[@]}" restore --native --recovery-root "$RECOVERY_ROOT"
```

Restore first verifies every independent master and every retained original
copy. It then takes two snapshots of every still-live post-run source and
requires them to be stable before the first live move. It moves present
post-run roots into the separate `variant-holding` tree, preserving those test
variants, and copy-restores masters into the live roots. It leaves roots that
were absent in the new baseline absent. It finishes only after:

- all master copies still match the manifest;
- all original-holding copies still match their journal records;
- all variant-holding copies still match their journal records;
- the live six-root state matches the new baseline exactly; and
- the durable journal contains `complete`.

An absent root is a first-class state. If the app creates a root that was absent
at capture, restore moves that new variant to `variant-holding` and leaves the
live root absent. If a copy/move fails partially, the journal records failure,
the result is not a pass, and retry is conservative: an ambiguous occupied
destination is rejected rather than overwritten or recursively cleaned.

## Manifest and hash contract

The manifest is JSON with exactly these top-level keys:

```json
{
  "format": "mfa-profile-guard-v1",
  "root_order": [
    "application-support",
    "caches",
    "saved-application-state",
    "httpstorages",
    "webkit",
    "preferences"
  ],
  "roots": [
    { "label": "…", "state": "…", "kind": "…", "digest": "…", "files": 0 }
  ]
}
```

The validator rejects unknown keys, duplicate/missing/reordered labels, invalid
states/kinds, invalid digest length/format, and inconsistent absent rows. A
present row has `state: "present"` and `kind: "directory"` or `"file"`; an
absent row has `state: "absent"`, `kind: "absent"`, zero files, and the fixed
versioned absent marker digest.

For a present root the SHA-256 input is versioned and canonical. It includes:

- the present-tree header;
- the root entry as relative name `.`;
- every child entry sorted by canonical relative POSIX name, including empty
  directories;
- each entry kind, relative name, permission mode, size, and `mtime_ns`;
- regular-file byte length and bytes.

The absolute path and root basename are excluded. ACLs, extended attributes
(xattrs), ownership (UID/GID), birth time, Finder/resource-fork metadata, flags,
and unrelated filesystem timestamps are not hashed. The contract therefore
does not claim full filesystem-metadata bit identity; it claims exact presence,
canonical names, supported object kinds, permission mode, size, `mtime_ns`, and
regular-file bytes. Symlinks, FIFOs, sockets, devices, and other unsupported
objects are rejected before they can be copied or moved.

## Journal and retry contract

The journal is append-only JSON Lines and each write is flushed and fsynced.
The expected phase events are:

1. `capture_started`, six `captured` records, `capture_complete`;
2. repeated `isolation_preflight`, six `original_moved` records,
   `isolation_complete`;
3. repeated `restore_preflight`, six `variant_moved` records, six `restored`
   records, `complete`.

`failure` and `interrupted` are durable non-success events. They never imply
completion. Unknown events, events after `complete`, incomplete canonical
label sets, malformed records, and phase mixing are rejected. A retry reads
the journal and verifies all known retained copies before resuming; it never
deletes, overwrites, or silently forgets a prior variant.

## Synthetic executable suite

The checked-in suite is independent of user paths and uses only temporary
fixtures. It exercises the same helper CLI and includes:

- root-basename rename independence;
- regular-file Preferences and empty directories;
- actual master-byte corruption and all-master verification before any move;
- partial isolation/variant move/copy failures and interruption/retry;
- source changes before isolation;
- absent roots and post-run-created variants;
- malformed manifests and durable journal failures;
- occupied destinations retained without overwrite;
- symlink and unsupported-object rejection;
- exact live-process rejection and malformed process-listing handling; and
- fake-home native path derivation without native filesystem access.

Run it from the repository root:

```bash
python3 scripts/test_profile_guard.py
```

The test-only `native-paths --synthetic-os-home …` diagnostic is gated by
`MFA_PROFILE_GUARD_TESTING=1` and only formats fixed paths; it is not native
capture/isolation/restore and must never be used as an acceptance result.

## Mandatory normal-install update coverage

The preflight/manual contract does **not** permit `N/A` for update coverage.
The following human-approved replacement criterion is the legitimate historical-
package scenario to prepare through normal packaging and installation, without
manifest or database patching:

1. Use the immutable prior-package command preparation that exercised the
   actual normal `PackageInstaller` and `CapabilityRegistry` paths with
   temporary module/config roots. It installed the historical packages without
   editing manifests or database records; this is command/package preparation,
   not native acceptance and not an invented application artifact.
2. Through the normal package-install/update commands, verify that the initial
   pending candidates are named `base` and `MyNetDiary`, then perform explicit
   updates for both modules and verify that both pending names clear. For the
   unselected `activity.days` capability, the expected command state before and
   after both explicit module updates is `MissingCapability`/`configure_source`,
   visible as `Required data is not available yet` with `Configure source`. The
   historical packages used by this scenario contain no incompatible contract.
   No automatic package update or provider selection is allowed. Retain the
   command evidence separately from the manual UI result.
3. In the current UI, exercise the visible pending-update surface and `Open
Settings`. Before either module update, verify the visible pending names
   `base` and `MyNetDiary` and the expected unavailable-data state. Press
   `Update` explicitly for MyNetDiary and then base; after both complete, verify
   that the pending notices have disappeared, the unavailable-data state remains
   unchanged, and no `Active provider` selection changed automatically.
4. In the `activity.days` row, explicitly press `Use` when that control is
   presented. Only this explicit selection may change the state to
   `WaitingForData` with `Import data` before a successful snapshot. Do not record
   this historical-package scenario as clearing an `IncompatibleContract`: the
   genuine incompatible-provider behavior remains a mandatory automatic-test
   boundary and must be checked separately.
5. If this normal command/package setup or the explicit UI `Use` step cannot be
   prepared, keep the update coverage item **PENDING**. Do not write `PASS` or
   `N/A`.

The historical recovery mismatch and historical native failure remain
historical `FAIL` evidence with unknown cause. This new guard must not rewrite
that baseline or claim that root renaming explains it. Successful web tests,
packaging, signing, or helper rehearsal do not imply native acceptance.

## Current correction evidence

The Settings regression was observed RED against the unchanged component and
then GREEN after the minimal typed-bootstrap hydration change:

```text
RED:   (cd web && pnpm exec vitest run src/lib/pages/SettingsPage.test.ts) -> exit 1; 3 failed, 5 passed
GREEN: (cd web && pnpm exec vitest run src/lib/pages/SettingsPage.test.ts) -> exit 0; 8 passed
```

The clarified fresh-empty guard contract was observed RED against the seeded
two-phase helper and then GREEN after the three-phase/native correction:

```text
RED:   python3 scripts/test_profile_guard.py -> exit 1; 7 assertion failures
GREEN: python3 scripts/test_profile_guard.py -> exit 0; 20 tests passed
```

These are repository/fixture results only. The manager-recorded P6 native run on
the pre-fix package is **FAIL** for stale shell state and rejected Retry hiding
the quality row; its normal Quit, storage, and six-root restoration observations
are separate successful observations. Native retest of the fixed product remains
**NOT RUN/PENDING**, and prior packaged artifacts are stale for that retest.
