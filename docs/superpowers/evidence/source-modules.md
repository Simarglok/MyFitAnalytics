# Source Module Verification Evidence

## Fixture and package verification

- Only checked-in synthetic fixtures were used. No real exports were used.
- Fixture verification passed: 7 BIFF fixtures and 2 CSV fixtures; privacy scan passed.
- Deterministic package verification passed:
  - MyNetDiary SHA-256: `3871f17503080c308111741c0c2202ade91ef8e9e2584cc3cf2a00e7387fefa6`
  - Hevy SHA-256: `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379`

## Automated verification

The following checks passed:

- `cargo test -p mfa-source-contract-tests`
- `cargo test -p mfa-source-mynetdiary`
- `cargo test -p mfa-source-hevy`
- `source_modules_gate`: 4/4 passed
- `provider_selection`: 1/1 passed
- `module_lifecycle_commands`: 9/9 passed
- `cargo test --workspace -- --test-threads=1`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- SettingsPage tests
- Full web tests: 10/10 passed
- `pnpm --dir web check`
- `pnpm --dir web build`

## Terra High review disposition

Terra High final review disposition: `CHANGES REQUIRED` after three correction rounds.

The remaining code blocker is that `package.rs` commits uninstall state before filesystem cleanup and suppresses remove/read/sync failures, so deletion can report success while staged bytes remain or are partially deleted; uninstall is not fully atomic/restorable.

## macOS packaging and launch verification

- `cargo tauri build` failed during DMG bundling at `bundle_dmg.sh`, including an outside-sandbox retry.
- `cargo tauri build --bundles app` passed and produced `target/release/bundle/macos/MyFitAnalytics.app`.
- Packaged executable SHA-256: `b87da66560b6bdfe9867988d7fff1caae9d28aabe68c103276401afd91dd3cd4`.
- Packaged module hashes match the deterministic hashes above:
  - MyNetDiary: `3871f17503080c308111741c0c2202ade91ef8e9e2584cc3cf2a00e7387fefa6`
  - Hevy: `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379`
- `codesign --verify --deep --strict` failed with: `code has no resources but signature indicates they must be present`. The app has no sealed resources or `_CodeSignature`.
- Native launch with an isolated profile stayed running without an immediate crash, but the bundled catalog was empty.
- Packaged modules are under `Contents/Resources/_up_/dist/modules`, while runtime lookup expects `resource_dir/modules`.
- Native install/disable/re-enable/uninstall production lifecycle could not be completed.
- UI automation was unavailable because macOS Accessibility permission was not granted.

## Remaining Draft PR work

- Make uninstall deletion atomic and restorable.
- Align Tauri resource packaging and runtime lookup.
- Fix strict app signing and DMG packaging.
- Rerun the full native install/disable/re-enable/uninstall lifecycle and obtain an independent Terra review.

No merge, tag, release, push, or pull request was performed.
