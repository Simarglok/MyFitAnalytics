#!/usr/bin/env python3
"""Synthetic executable tests for scripts/profile_guard.py."""

from __future__ import annotations

import json
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from typing import cast
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "profile_guard.py"
SCOPE_MARKER = ".mfa-profile-guard-scope"
LABELS = (
    "application-support",
    "caches",
    "saved-application-state",
    "httpstorages",
    "webkit",
    "preferences",
)


class Fixture:
    def __init__(self, scope: Path, root_names: dict[str, str] | None = None) -> None:
        self.scope = scope
        self.scope.mkdir(parents=True)
        (scope / SCOPE_MARKER).write_text(
            "MFA_PROFILE_GUARD_SYNTHETIC_V1\n", encoding="utf-8"
        )
        self.live_root = scope / "live"
        self.live_root.mkdir()
        self.master_root = scope / "masters"
        self.original_holding_root = scope / "original-holding"
        self.variant_holding_root = scope / "variant-holding"
        self.manifest = scope / "manifest.json"
        self.journal = scope / "journal.jsonl"
        root_names = root_names or {}
        self.roots: dict[str, Path] = {}
        for label in LABELS:
            root = self.live_root / root_names.get(label, label)
            if label == "preferences":
                root.write_text(
                    f"stable synthetic preferences: {label}\n", encoding="utf-8"
                )
            else:
                root.mkdir()
                (root / "payload.txt").write_text(
                    f"stable synthetic payload: {label}\n", encoding="utf-8"
                )
                (root / "empty-directory").mkdir()
                nested = root / "nested"
                nested.mkdir()
                (nested / "nested.txt").write_text(
                    f"nested synthetic payload: {label}\n", encoding="utf-8"
                )
            self.roots[label] = root

    def command(
        self,
        action: str,
        *extra: str,
        process_pid: int | None = None,
        process_name: str = "synthetic-app",
        scope_root: Path | None = None,
    ) -> subprocess.CompletedProcess[str]:
        if process_pid is None:
            process = subprocess.Popen([sys.executable, "-c", "pass"])
            process.wait()
            process_pid = process.pid
        command = [
            sys.executable,
            str(SCRIPT),
            action,
            "--scope-root",
            str(scope_root or self.scope),
            "--live-root",
            str(self.live_root),
            "--master-root",
            str(self.master_root),
            "--original-holding-root",
            str(self.original_holding_root),
            "--variant-holding-root",
            str(self.variant_holding_root),
            "--manifest",
            str(self.manifest),
            "--journal",
            str(self.journal),
            "--process-pid",
            str(process_pid),
            "--process-name",
            process_name,
        ]
        for label in LABELS:
            command.extend(["--root", f"{label}={self.roots[label]}"])
        command.extend(extra)
        environment = os.environ.copy()
        environment["MFA_PROFILE_GUARD_TESTING"] = "1"
        return subprocess.run(
            command,
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )

    def capture(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return self.command("capture", *extra)

    def isolate(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return self.command("isolate", *extra)

    def restore(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return self.command("restore", *extra)

    def create_variant(
        self, label: str, contents: str = "synthetic app variant\n"
    ) -> Path:
        root = self.roots[label]
        if os.path.lexists(root):
            if root.is_dir() and not root.is_symlink():
                shutil.rmtree(root)
            else:
                root.unlink()
        if label == "preferences":
            root.write_text(contents, encoding="utf-8")
        else:
            root.mkdir()
            (root / "payload.txt").write_text(contents, encoding="utf-8")
        return root


def tree_contents(path: Path) -> dict[str, bytes | None] | None:
    if not os.path.lexists(path):
        return None
    if path.is_symlink():
        return {"<symlink>": None}
    if path.is_file():
        return {"": path.read_bytes()}
    contents: dict[str, bytes | None] = {"": None}
    for current, directories, files in os.walk(path, followlinks=False):
        current_path = Path(current)
        for directory in sorted(directories):
            relative = (current_path / directory).relative_to(path).as_posix()
            contents[relative] = None
        for file_name in sorted(files):
            file_path = current_path / file_name
            relative = file_path.relative_to(path).as_posix()
            contents[relative] = file_path.read_bytes()
    return contents


def manifest_rows(fixture: Fixture) -> dict[str, dict[str, object]]:
    document = json.loads(fixture.manifest.read_text(encoding="utf-8"))
    return {row["label"]: row for row in document["roots"]}


def normalize_fixture_metadata(fixture: Fixture) -> None:
    timestamp = 1_700_000_000
    for root in fixture.roots.values():
        paths = [root]
        if root.is_dir():
            for current, directories, files in os.walk(root):
                current_path = Path(current)
                paths.extend(current_path / name for name in directories)
                paths.extend(current_path / name for name in files)
        for path in sorted(paths, key=lambda item: len(item.parts), reverse=True):
            os.utime(path, (timestamp, timestamp), follow_symlinks=False)


class ProfileGuardSyntheticTests(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary_directory = tempfile.TemporaryDirectory(
            prefix="mfa-profile-guard-test-"
        )
        self.fixture = Fixture(Path(self._temporary_directory.name) / "fixture")

    def tearDown(self) -> None:
        self._temporary_directory.cleanup()

    def assert_success(self, result: subprocess.CompletedProcess[str]) -> None:
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def assert_blocked(self, result: subprocess.CompletedProcess[str]) -> None:
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("BLOCKED", result.stdout + result.stderr)

    def test_root_rename_does_not_change_canonical_digest(self) -> None:
        first = Fixture(Path(self._temporary_directory.name) / "first")
        renamed = Fixture(
            Path(self._temporary_directory.name) / "renamed",
            root_names={
                "application-support": "different-root-name",
                "preferences": "different-preferences-name.plist",
            },
        )
        normalize_fixture_metadata(first)
        normalize_fixture_metadata(renamed)
        self.assert_success(first.capture())
        self.assert_success(renamed.capture())
        first_rows = manifest_rows(first)
        renamed_rows = manifest_rows(renamed)
        for label in LABELS:
            self.assertEqual(first_rows[label]["digest"], renamed_rows[label]["digest"])

    def test_capture_and_isolate_leave_fresh_empty_live_roots(self) -> None:
        self.assert_success(self.fixture.capture())
        original_application_support = tree_contents(
            self.fixture.roots["application-support"]
        )
        original_preferences = tree_contents(self.fixture.roots["preferences"])

        self.assert_success(self.fixture.isolate())

        self.assertEqual(
            original_application_support,
            tree_contents(self.fixture.original_holding_root / "application-support"),
        )
        self.assertEqual(
            original_preferences,
            tree_contents(self.fixture.original_holding_root / "preferences"),
        )
        for root in self.fixture.roots.values():
            self.assertFalse(os.path.lexists(root))
        self.assertFalse(
            os.path.lexists(self.fixture.variant_holding_root / "application-support")
        )
        self.assertEqual("file", manifest_rows(self.fixture)["preferences"]["kind"])

    def test_source_change_before_isolation_is_blocked_without_moving_any_root(self) -> None:
        self.assert_success(self.fixture.capture())
        changed = self.fixture.roots["application-support"] / "payload.txt"
        changed.write_bytes(b"changed before synthetic isolation\n")

        result = self.fixture.isolate()

        self.assert_blocked(result)
        self.assertEqual([], list(self.fixture.original_holding_root.iterdir()))
        self.assertFalse(
            os.path.lexists(self.fixture.original_holding_root / "application-support")
        )
        self.assertEqual(b"changed before synthetic isolation\n", changed.read_bytes())

    def test_actual_master_byte_corruption_is_rejected_before_variant_move(self) -> None:
        self.assert_success(self.fixture.capture())
        self.assert_success(self.fixture.isolate())
        master_file = self.fixture.master_root / "application-support" / "payload.txt"
        master_file.write_bytes(b"corrupted synthetic master bytes\n")

        result = self.fixture.restore()

        self.assert_blocked(result)
        self.assertFalse(os.path.lexists(self.fixture.roots["application-support"]))
        self.assertFalse(
            os.path.lexists(self.fixture.variant_holding_root / "application-support")
        )
        self.assertTrue(
            (self.fixture.original_holding_root / "application-support").exists()
        )

    def test_all_masters_are_verified_before_any_variant_move(self) -> None:
        self.assert_success(self.fixture.capture())
        self.assert_success(self.fixture.isolate())
        second_master = self.fixture.master_root / "caches" / "nested" / "nested.txt"
        second_master.write_bytes(b"corrupted second master\n")

        result = self.fixture.restore()

        self.assert_blocked(result)
        self.assertEqual([], list(self.fixture.variant_holding_root.iterdir()))
        self.assertFalse(os.path.lexists(self.fixture.roots["application-support"]))

    def test_partial_isolation_failure_is_not_pass_and_retry_is_conservative(self) -> None:
        self.assert_success(self.fixture.capture())

        failed = self.fixture.isolate("--synthetic-fail-after-isolate", "1")

        self.assert_blocked(failed)
        journal = self.fixture.journal.read_text()
        self.assertIn('"event": "failure"', journal)
        self.assertNotIn('"event": "isolation_complete"', journal)
        self.assertTrue(
            (self.fixture.original_holding_root / "application-support").exists()
        )
        self.assertFalse(os.path.lexists(self.fixture.roots["application-support"]))
        self.assertTrue((self.fixture.master_root / "application-support").exists())

        self.assert_success(self.fixture.isolate())
        self.assertIn('"event": "isolation_complete"', self.fixture.journal.read_text())
        for root in self.fixture.roots.values():
            self.assertFalse(os.path.lexists(root))

    def test_interruption_during_isolation_can_be_retried_without_cleanup(self) -> None:
        self.assert_success(self.fixture.capture())

        interrupted = self.fixture.isolate(
            "--synthetic-interrupt-after-isolate", "1"
        )

        self.assertNotEqual(
            interrupted.returncode, 0, interrupted.stdout + interrupted.stderr
        )
        self.assertIn('"event": "interrupted"', self.fixture.journal.read_text())
        self.assert_success(self.fixture.isolate())
        self.assertIn('"event": "isolation_complete"', self.fixture.journal.read_text())

    def test_partial_variant_move_failure_is_not_pass_and_retry_retains_variant(self) -> None:
        self.assert_success(self.fixture.capture())
        self.assert_success(self.fixture.isolate())
        self.fixture.create_variant("application-support")
        variant_before_move = tree_contents(self.fixture.roots["application-support"])

        failed = self.fixture.restore("--synthetic-fail-after-variant-move", "1")

        self.assert_blocked(failed)
        self.assertEqual(
            variant_before_move,
            tree_contents(self.fixture.variant_holding_root / "application-support"),
        )
        self.assertFalse(os.path.lexists(self.fixture.roots["application-support"]))
        self.assertTrue(
            (self.fixture.original_holding_root / "application-support").exists()
        )
        self.assertNotIn('"event": "complete"', self.fixture.journal.read_text())

        self.assert_success(self.fixture.restore())
        self.assertEqual(
            tree_contents(self.fixture.master_root / "application-support"),
            tree_contents(self.fixture.roots["application-support"]),
        )
        self.assertEqual(
            variant_before_move,
            tree_contents(self.fixture.variant_holding_root / "application-support"),
        )

    def test_interruption_during_variant_move_can_be_retried(self) -> None:
        self.assert_success(self.fixture.capture())
        self.assert_success(self.fixture.isolate())
        self.fixture.create_variant("application-support")

        interrupted = self.fixture.restore(
            "--synthetic-interrupt-after-variant-move", "1"
        )

        self.assertNotEqual(
            interrupted.returncode, 0, interrupted.stdout + interrupted.stderr
        )
        self.assertIn('"event": "interrupted"', self.fixture.journal.read_text())
        self.assert_success(self.fixture.restore())
        self.assertIn('"event": "complete"', self.fixture.journal.read_text())

    def test_partial_copy_failure_cannot_pass_and_retry_preserves_variants(self) -> None:
        self.assert_success(self.fixture.capture())
        self.assert_success(self.fixture.isolate())
        self.fixture.create_variant(
            "application-support", "synthetic copy-phase variant\n"
        )
        variant_before_restore = tree_contents(self.fixture.roots["application-support"])

        failed = self.fixture.restore("--synthetic-fail-after-copy", "1")

        self.assert_blocked(failed)
        journal = self.fixture.journal.read_text()
        self.assertIn('"event": "failure"', journal)
        self.assertNotIn('"event": "complete"', journal)
        self.assertEqual(
            variant_before_restore,
            tree_contents(self.fixture.variant_holding_root / "application-support"),
        )

        self.assert_success(self.fixture.restore())
        self.assertTrue((self.fixture.master_root / "application-support").exists())
        self.assertTrue(
            (self.fixture.original_holding_root / "application-support").exists()
        )
        self.assertTrue(
            (self.fixture.variant_holding_root / "application-support").exists()
        )

    def test_absent_baseline_root_stays_absent_and_new_variant_is_retained(self) -> None:
        shutil.rmtree(self.fixture.roots["caches"])
        self.assert_success(self.fixture.capture())
        self.assert_success(self.fixture.isolate())
        self.fixture.create_variant("caches", "new synthetic variant\n")

        self.assert_success(self.fixture.restore())

        self.assertFalse(os.path.lexists(self.fixture.roots["caches"]))
        self.assertTrue((self.fixture.variant_holding_root / "caches").exists())
        self.assertFalse((self.fixture.original_holding_root / "caches").exists())
        self.assertEqual("absent", manifest_rows(self.fixture)["caches"]["state"])

    def test_malformed_manifest_is_rejected_without_mutation(self) -> None:
        self.assert_success(self.fixture.capture())
        self.assert_success(self.fixture.isolate())
        self.fixture.manifest.write_text("{malformed\n", encoding="utf-8")

        result = self.fixture.restore()

        self.assert_blocked(result)
        self.assertFalse(os.path.lexists(self.fixture.roots["application-support"]))
        self.assertFalse(
            os.path.lexists(self.fixture.variant_holding_root / "application-support")
        )

    def test_occupied_variant_destination_is_rejected_and_retained(self) -> None:
        self.assert_success(self.fixture.capture())
        self.assert_success(self.fixture.isolate())
        occupied = self.fixture.variant_holding_root / "application-support"
        occupied.mkdir()
        keep = occupied / "must-remain.txt"
        keep.write_text("occupied synthetic destination\n", encoding="utf-8")

        result = self.fixture.restore()

        self.assert_blocked(result)
        self.assertEqual(
            "occupied synthetic destination\n", keep.read_text(encoding="utf-8")
        )
        self.assertFalse(os.path.lexists(self.fixture.roots["application-support"]))
        self.assertTrue(
            (self.fixture.original_holding_root / "application-support").exists()
        )

    def test_symlink_root_is_rejected(self) -> None:
        target = self.fixture.scope / "symlink-target"
        target.mkdir()
        shutil.rmtree(self.fixture.roots["caches"])
        self.fixture.roots["caches"].symlink_to(target, target_is_directory=True)

        result = self.fixture.capture()

        self.assert_blocked(result)
        self.assertFalse(self.fixture.manifest.exists())

    def test_unsupported_fifo_is_rejected(self) -> None:
        os.mkfifo(self.fixture.roots["webkit"] / "unsupported-fifo")

        result = self.fixture.capture()

        self.assert_blocked(result)
        self.assertFalse(self.fixture.manifest.exists())

    def test_exact_live_process_is_rejected(self) -> None:
        process_name = subprocess.run(
            ["ps", "-p", str(os.getpid()), "-o", "comm="],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        self.assertTrue(process_name)

        result = self.fixture.command(
            "capture",
            process_pid=os.getpid(),
            process_name=Path(process_name).name,
        )

        self.assert_blocked(result)
        self.assertFalse(self.fixture.manifest.exists())

    def test_library_named_scope_is_rejected_without_profile_access(self) -> None:
        unsafe = Fixture(Path(self._temporary_directory.name) / "Library" / "synthetic")

        result = unsafe.capture()

        self.assert_blocked(result)
        self.assertFalse(unsafe.manifest.exists())

    def test_process_listing_validator_is_exact_and_fail_closed(self) -> None:
        import profile_guard

        validator = profile_guard.process_listing_has_exact
        self.assertEqual(profile_guard.NATIVE_PROCESS_NAME, "myfitanalytics")
        self.assertTrue(
            validator("123 /synthetic/MyFitAnalytics.app/Contents/MacOS/myfitanalytics\n", "myfitanalytics")
        )
        self.assertTrue(validator("123 myfitanalytics\n", "myfitanalytics"))
        self.assertFalse(validator("123 OtherApp\n", "myfitanalytics"))
        with self.assertRaises(profile_guard.GuardError):
            validator("not-a-process-row\n", "myfitanalytics")
        with self.assertRaises(profile_guard.GuardError):
            validator("", "myfitanalytics")
        process_config = cast(
            profile_guard.GuardConfig, argparse.Namespace(process_name="myfitanalytics")
        )
        for process_result in (
            subprocess.CompletedProcess(["ps"], 1, stdout="", stderr="failed"),
            subprocess.CompletedProcess(["ps"], 0, stdout="", stderr=""),
        ):
            with patch.object(
                profile_guard.subprocess, "run", return_value=process_result
            ):
                with self.assertRaises(profile_guard.GuardError):
                    profile_guard._assert_process_absent(process_config)

    def test_native_paths_use_injected_home_and_fixed_bundle_id(self) -> None:
        fake_home = Path(self._temporary_directory.name) / "fake-home"
        fake_home.mkdir()
        environment = os.environ.copy()
        environment["MFA_PROFILE_GUARD_TESTING"] = "1"
        result = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "native-paths",
                "--synthetic-os-home",
                str(fake_home),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        document = json.loads(result.stdout)
        self.assertEqual("com.simarglok.myfitanalytics", document["bundle_id"])
        expected = {
            "application-support": fake_home
            / "Library/Application Support/com.simarglok.myfitanalytics",
            "caches": fake_home / "Library/Caches/com.simarglok.myfitanalytics",
            "saved-application-state": fake_home
            / "Library/Saved Application State/com.simarglok.myfitanalytics.savedState",
            "httpstorages": fake_home
            / "Library/HTTPStorages/com.simarglok.myfitanalytics",
            "webkit": fake_home / "Library/WebKit/com.simarglok.myfitanalytics",
            "preferences": fake_home
            / "Library/Preferences/com.simarglok.myfitanalytics.plist",
        }
        self.assertEqual(
            {label: str(path) for label, path in expected.items()}, document["roots"]
        )

    def test_native_builder_and_three_phases_use_injected_context(self) -> None:
        import profile_guard

        fake_home = Path(self._temporary_directory.name) / "native-home"
        library = fake_home / "Library"
        library.mkdir(parents=True)
        fixed_roots = profile_guard.native_paths(fake_home)
        for label, path in fixed_roots.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            if label == "preferences":
                path.write_text("synthetic preferences\n", encoding="utf-8")
            else:
                path.mkdir()
                (path / "payload.txt").write_text(label, encoding="utf-8")
                (path / "empty").mkdir()

        recovery_root = Path(self._temporary_directory.name) / "native-recovery"
        recovery_root.mkdir()
        (recovery_root / SCOPE_MARKER).write_text(
            "MFA_PROFILE_GUARD_SYNTHETIC_V1\n", encoding="utf-8"
        )
        arguments = argparse.Namespace(
            native=True,
            recovery_root=str(recovery_root),
            scope_root=None,
            live_root=None,
            master_root=None,
            original_holding_root=None,
            variant_holding_root=None,
            manifest=None,
            journal=None,
            process_pid=None,
            process_name=None,
            root=None,
        )
        isolate_arguments = argparse.Namespace(
            synthetic_fail_after_isolate=None,
            synthetic_interrupt_after_isolate=None,
        )
        restore_arguments = argparse.Namespace(
            synthetic_fail_after_variant_move=None,
            synthetic_interrupt_after_variant_move=None,
            synthetic_fail_after_copy=None,
        )
        process_result = subprocess.CompletedProcess(
            ["ps", "-axo", "pid=,comm="],
            0,
            stdout="123 unrelated-process\n",
            stderr="",
        )

        with (
            patch.object(profile_guard, "_real_os_home", return_value=fake_home),
            patch.object(profile_guard.subprocess, "run", return_value=process_result),
        ):
            config = profile_guard._build_native_config(arguments)
            profile_guard._capture(config)
            expected_parents = {root.path.parent for root in config.roots}
            with patch.object(profile_guard, "_fsync_directory") as fsync_directory:
                profile_guard._isolate(config, isolate_arguments)
            actual_parents = {
                call.args[0] for call in fsync_directory.call_args_list
            }
            self.assertTrue(expected_parents <= actual_parents)
            self.assertTrue(
                all(not os.path.lexists(path) for path in fixed_roots.values())
            )
            variant = fixed_roots["application-support"]
            variant.mkdir(parents=True)
            (variant / "post-run.txt").write_text("variant", encoding="utf-8")
            with patch.object(profile_guard, "_fsync_directory") as fsync_directory:
                profile_guard._restore(config, restore_arguments)
            actual_parents = {
                call.args[0] for call in fsync_directory.call_args_list
            }
            self.assertTrue(expected_parents <= actual_parents)

        self.assertTrue((config.master_root / "preferences").is_file())
        self.assertTrue(
            (config.variant_holding_root / "application-support" / "post-run.txt").is_file()
        )


if __name__ == "__main__":
    unittest.main()
