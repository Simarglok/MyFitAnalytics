#!/usr/bin/env python3
"""Fail-closed synthetic/native profile-tree guard.

This helper never launches an application and never deletes a tree.  It is
intended to operate only on a session-owned temporary scope marked with
.mfa-profile-guard-scope.  The packaged-native preflight evidence record is
maintained in the owner's local Obsidian workspace; repository AGENTS.md
provides the vault location and read/update rules.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import pwd
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import Any, Iterable


FORMAT = "mfa-profile-guard-v1"
SCOPE_MARKER = ".mfa-profile-guard-scope"
SCOPE_MARKER_CONTENT = "MFA_PROFILE_GUARD_SYNTHETIC_V1\n"
ABSENT_DIGEST = hashlib.sha256(b"MFA_PROFILE_GUARD_V1\nABSENT\n").hexdigest()
ROOT_LABELS = (
    "application-support",
    "caches",
    "saved-application-state",
    "httpstorages",
    "webkit",
    "preferences",
)
DIGEST_LENGTH = 64
INTERRUPTED_EXIT = 75
NATIVE_BUNDLE_ID = "com.simarglok.myfitanalytics"
NATIVE_PROCESS_NAME = "myfitanalytics"


class GuardError(Exception):
    """A public, path-safe guard failure."""


class GuardInterrupted(GuardError):
    """A synthetic interruption after a durable journal event."""


@dataclass(frozen=True)
class Snapshot:
    state: str
    kind: str
    digest: str
    files: int

    def as_manifest_row(self, label: str) -> dict[str, object]:
        return {
            "label": label,
            "state": self.state,
            "kind": self.kind,
            "digest": self.digest,
            "files": self.files,
        }


@dataclass(frozen=True)
class RootSpec:
    label: str
    path: Path


@dataclass(frozen=True)
class GuardConfig:
    scope_root: Path
    live_root: Path
    master_root: Path
    original_holding_root: Path
    variant_holding_root: Path
    manifest: Path
    journal: Path
    roots: tuple[RootSpec, ...]
    process_pid: int | None
    process_name: str


@dataclass(frozen=True)
class JournalSummary:
    captured: dict[str, Snapshot]
    original_moved: dict[str, Snapshot]
    isolation_complete: bool
    variant_moved: dict[str, Snapshot]
    restored: frozenset[str]
    complete: bool


def _lexists(path: Path) -> bool:
    return os.path.lexists(path)


def _is_relative(path: Path, parent: Path, *, allow_equal: bool = False) -> bool:
    try:
        relative = path.relative_to(parent)
    except ValueError:
        return False
    return allow_equal or relative != Path(".")


def _safe_path(path_value: str, *, label: str) -> Path:
    path = Path(path_value)
    if not path.is_absolute() or ".." in path.parts:
        raise GuardError(f"{label} must be an absolute normalized path")
    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        try:
            mode = current.lstat().st_mode
        except FileNotFoundError:
            break
        except OSError:
            raise GuardError(f"{label} could not be inspected") from None
        if stat.S_ISLNK(mode):
            if current in {Path("/var"), Path("/tmp")}:
                continue
            raise GuardError(f"{label} contains a symlink")
    return path


def _resolved(path: Path) -> Path:
    try:
        return path.resolve(strict=False)
    except OSError:
        raise GuardError("guard path could not be resolved") from None


def _validate_scope(scope_root: Path) -> Path:
    if not _lexists(scope_root):
        raise GuardError("scope root is missing")
    if not scope_root.is_dir():
        raise GuardError("scope root is not a directory")
    scope = _resolved(scope_root)
    if scope == Path(scope.anchor):
        raise GuardError("scope root is unsafe")
    if "Library" in scope.parts:
        raise GuardError("scope root may not contain a Library path component")
    home = _resolved(Path.home())
    if _is_relative(scope, home, allow_equal=True):
        raise GuardError("scope root may not be inside the user home")
    allowed_temporary_roots = {
        _resolved(Path(tempfile.gettempdir())),
        _resolved(Path("/tmp")),
        _resolved(Path("/private/tmp")),
    }
    if not any(
        _is_relative(scope, temporary_root)
        for temporary_root in allowed_temporary_roots
    ):
        raise GuardError("scope root must be inside a temporary directory")
    marker = scope / SCOPE_MARKER
    if not _lexists(marker) or marker.is_symlink() or not marker.is_file():
        raise GuardError("scope marker is missing")
    try:
        marker_content = marker.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        raise GuardError("scope marker is unreadable") from None
    if marker_content != SCOPE_MARKER_CONTENT:
        raise GuardError("scope marker is invalid")
    return scope


def _parse_roots(raw_roots: Iterable[str]) -> tuple[tuple[str, Path], ...]:
    parsed: dict[str, Path] = {}
    for raw_root in raw_roots:
        label, separator, raw_path = raw_root.partition("=")
        if not separator or label not in ROOT_LABELS or not raw_path:
            raise GuardError("root mapping is malformed")
        if label in parsed:
            raise GuardError("root mappings contain a duplicate label")
        parsed[label] = _safe_path(raw_path, label=f"{label} root")
    if tuple(parsed) != ROOT_LABELS:
        raise GuardError("root mappings must contain the six canonical labels in order")
    return tuple((label, parsed[label]) for label in ROOT_LABELS)


def _validated_config(
    *,
    scope: Path,
    live_root: Path,
    master_root: Path,
    original_holding_root: Path,
    variant_holding_root: Path,
    manifest: Path,
    journal: Path,
    raw_roots: Iterable[str],
    process_pid: int | None,
    process_name: str,
    native: bool,
) -> GuardConfig:
    parsed_roots = _parse_roots(raw_roots)
    resolved_live = _resolved(live_root)
    resolved_master = _resolved(master_root)
    resolved_original_holding = _resolved(original_holding_root)
    resolved_variant_holding = _resolved(variant_holding_root)
    resolved_manifest = _resolved(manifest)
    resolved_journal = _resolved(journal)
    named_paths = (
        ("live root", resolved_live),
        ("master root", resolved_master),
        ("original holding root", resolved_original_holding),
        ("variant holding root", resolved_variant_holding),
    )
    recovery_paths = named_paths[1:]
    for label, path in recovery_paths:
        if not _is_relative(path, scope):
            raise GuardError(f"{label} must be inside the recovery scope")
    if not native and not _is_relative(resolved_live, scope):
        raise GuardError("live root must be inside the synthetic scope")
    for left_index, (left_label, left_path) in enumerate(named_paths):
        for right_label, right_path in named_paths[left_index + 1 :]:
            if _is_relative(left_path, right_path, allow_equal=True) or _is_relative(
                right_path, left_path, allow_equal=True
            ):
                raise GuardError(f"{left_label} and {right_label} may not overlap")
    if resolved_manifest == resolved_journal:
        raise GuardError("manifest and journal must be different files")
    for file_path in (resolved_manifest, resolved_journal):
        if not _is_relative(file_path, scope):
            raise GuardError("manifest and journal must be inside the recovery scope")
        if any(
            _is_relative(file_path, container, allow_equal=True)
            for _, container in named_paths
        ):
            raise GuardError("manifest and journal must be outside guarded trees")
        if not file_path.parent.is_dir():
            raise GuardError("manifest and journal parents must exist")
    if not _lexists(live_root) or not live_root.is_dir():
        raise GuardError("live root is missing or not a directory")

    roots: list[RootSpec] = []
    for label, root_path in parsed_roots:
        resolved_root = _resolved(root_path)
        if not _is_relative(resolved_root, resolved_live):
            raise GuardError("each canonical root must be below the live root")
        if not native and not _is_relative(resolved_root, scope):
            raise GuardError("canonical root must be inside the synthetic scope")
        roots.append(RootSpec(label, resolved_root))

    if process_pid is not None and process_pid <= 0:
        raise GuardError("process pid is malformed")
    if not process_name:
        raise GuardError("an exact process check is required")
    normalized_process_name = Path(process_name).name
    if not normalized_process_name:
        raise GuardError("process name is malformed")
    return GuardConfig(
        scope,
        resolved_live,
        resolved_master,
        resolved_original_holding,
        resolved_variant_holding,
        resolved_manifest,
        resolved_journal,
        tuple(roots),
        process_pid,
        normalized_process_name,
    )


def _real_os_home() -> Path:
    try:
        home_value = pwd.getpwuid(os.getuid()).pw_dir
    except (KeyError, OSError):
        raise GuardError("the real OS home could not be resolved") from None
    return _safe_path(home_value, label="real OS home")


def _build_native_config(args: argparse.Namespace) -> GuardConfig:
    if not args.recovery_root:
        raise GuardError("native mode requires a recovery root")
    if any(
        value is not None
        for value in (
            args.scope_root,
            args.live_root,
            args.master_root,
            args.original_holding_root,
            args.variant_holding_root,
            args.manifest,
            args.journal,
            args.process_pid,
            args.process_name,
            args.root,
        )
    ):
        raise GuardError("native mode does not accept synthetic path overrides")
    recovery_root = _safe_path(args.recovery_root, label="native recovery root")
    scope = _validate_scope(recovery_root)
    os_home = _real_os_home()
    roots = native_paths(os_home)
    raw_roots = [f"{label}={roots[label]}" for label in ROOT_LABELS]
    live_root = os_home / "Library"
    return _validated_config(
        scope=scope,
        live_root=live_root,
        master_root=scope / "masters",
        original_holding_root=scope / "original-holding",
        variant_holding_root=scope / "variant-holding",
        manifest=scope / "manifest.json",
        journal=scope / "journal.jsonl",
        raw_roots=raw_roots,
        process_pid=None,
        process_name=NATIVE_PROCESS_NAME,
        native=True,
    )


def _build_config(args: argparse.Namespace) -> GuardConfig:
    if args.native:
        return _build_native_config(args)
    if args.recovery_root is not None:
        raise GuardError("recovery root is available only in native mode")
    required_values = (
        args.scope_root,
        args.live_root,
        args.master_root,
        args.original_holding_root,
        args.variant_holding_root,
        args.manifest,
        args.journal,
        args.process_name,
        args.root,
    )
    if any(value is None for value in required_values):
        raise GuardError("synthetic mode requires all explicit path arguments")
    scope_root = _safe_path(args.scope_root, label="scope root")
    scope = _validate_scope(scope_root)
    return _validated_config(
        scope=scope,
        live_root=_safe_path(args.live_root, label="live root"),
        master_root=_safe_path(args.master_root, label="master root"),
        original_holding_root=_safe_path(
            args.original_holding_root, label="original holding root"
        ),
        variant_holding_root=_safe_path(
            args.variant_holding_root, label="variant holding root"
        ),
        manifest=_safe_path(args.manifest, label="manifest"),
        journal=_safe_path(args.journal, label="journal"),
        raw_roots=args.root,
        process_pid=args.process_pid,
        process_name=args.process_name,
        native=False,
    )


def native_paths(os_home: Path) -> dict[str, Path]:
    """Return the fixed MyFitAnalytics macOS roots for an injected OS home."""
    if not os_home.is_absolute() or ".." in os_home.parts:
        raise GuardError("native OS home is malformed")
    library = os_home / "Library"
    return {
        "application-support": library
        / "Application Support"
        / NATIVE_BUNDLE_ID,
        "caches": library / "Caches" / NATIVE_BUNDLE_ID,
        "saved-application-state": library
        / "Saved Application State"
        / f"{NATIVE_BUNDLE_ID}.savedState",
        "httpstorages": library / "HTTPStorages" / NATIVE_BUNDLE_ID,
        "webkit": library / "WebKit" / NATIVE_BUNDLE_ID,
        "preferences": library / "Preferences" / f"{NATIVE_BUNDLE_ID}.plist",
    }


def process_listing_has_exact(output: str, expected: str) -> bool:
    if not output.strip() or not expected:
        raise GuardError("process inspection returned no usable data")
    for line in output.splitlines():
        fields = line.strip().split(None, 1)
        if len(fields) != 2 or not fields[0].isdigit():
            raise GuardError("process inspection returned malformed data")
        if Path(fields[1]).name == expected:
            return True
    return False


def _assert_process_absent(config: GuardConfig) -> None:
    try:
        result = subprocess.run(
            ["ps", "-axo", "pid=,comm="],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        raise GuardError("exact process absence could not be verified") from None
    if result.returncode != 0 or not result.stdout.strip():
        raise GuardError("exact process absence could not be verified")
    if process_listing_has_exact(result.stdout, config.process_name):
        raise GuardError("the exact application process is still running")


def _metadata_record(kind: str, relative: str, mode: int, size: int, mtime_ns: int) -> bytes:
    metadata = json.dumps(
        {
            "kind": kind,
            "mode": mode,
            "mtime_ns": mtime_ns,
            "path": relative,
            "size": size,
        },
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return len(metadata).to_bytes(8, "big") + metadata


def _mtime_ns(value: os.stat_result) -> int:
    return int(getattr(value, "st_mtime_ns", round(value.st_mtime * 1_000_000_000)))


def _snapshot_tree(path: Path) -> Snapshot:
    if not _lexists(path):
        return Snapshot("absent", "absent", ABSENT_DIGEST, 0)
    try:
        root_stat = path.lstat()
    except OSError:
        raise GuardError("guarded root could not be inspected") from None
    if stat.S_ISLNK(root_stat.st_mode):
        raise GuardError("symlinks are not supported")
    if stat.S_ISREG(root_stat.st_mode):
        entries: list[tuple[str, str, Path, os.stat_result]] = [
            ("file", ".", path, root_stat)
        ]
        file_count = 1
    elif stat.S_ISDIR(root_stat.st_mode):
        entries = [("dir", ".", path, root_stat)]
        file_count = 0

        def collect(directory: Path, prefix: str) -> None:
            nonlocal file_count
            try:
                children = sorted(os.scandir(directory), key=lambda entry: entry.name)
            except OSError:
                raise GuardError("guarded tree could not be enumerated") from None
            for child in children:
                relative = f"{prefix}/{child.name}" if prefix != "." else child.name
                child_path = Path(child.path)
                try:
                    child_stat = child.stat(follow_symlinks=False)
                except OSError:
                    raise GuardError("guarded tree entry could not be inspected") from None
                if stat.S_ISLNK(child_stat.st_mode):
                    raise GuardError("symlinks are not supported")
                if stat.S_ISDIR(child_stat.st_mode):
                    entries.append(("dir", relative, child_path, child_stat))
                    collect(child_path, relative)
                elif stat.S_ISREG(child_stat.st_mode):
                    entries.append(("file", relative, child_path, child_stat))
                    file_count += 1
                else:
                    raise GuardError("unsupported filesystem objects are not allowed")

        collect(path, ".")
    else:
        raise GuardError("unsupported guarded root object")

    entries.sort(key=lambda item: item[1])
    digest = hashlib.sha256(b"MFA_PROFILE_GUARD_TREE_V1\nPRESENT\n")
    for kind, relative, entry_path, entry_stat in entries:
        digest.update(
            _metadata_record(
                kind,
                relative,
                stat.S_IMODE(entry_stat.st_mode),
                entry_stat.st_size if kind == "file" else 0,
                _mtime_ns(entry_stat),
            )
        )
        if kind == "file":
            try:
                before_read = entry_path.stat()
                data = entry_path.read_bytes()
                after_read = entry_path.stat()
            except OSError:
                raise GuardError("guarded file could not be read") from None
            if (
                before_read.st_size != after_read.st_size
                or _mtime_ns(before_read) != _mtime_ns(after_read)
                or stat.S_IMODE(before_read.st_mode)
                != stat.S_IMODE(after_read.st_mode)
            ):
                raise GuardError("guarded file changed during hashing")
            digest.update(len(data).to_bytes(8, "big"))
            digest.update(data)
    kind = "directory" if stat.S_ISDIR(root_stat.st_mode) else "file"
    return Snapshot("present", kind, digest.hexdigest(), file_count)


def _fsync_file(path: Path) -> None:
    try:
        with path.open("rb") as handle:
            os.fsync(handle.fileno())
    except OSError:
        raise GuardError("durability check failed") from None


def _fsync_directory(path: Path) -> None:
    try:
        descriptor = os.open(path, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    except OSError:
        raise GuardError("durability check failed") from None


def _copy_tree(source: Path, destination: Path) -> None:
    if _lexists(destination):
        raise GuardError("copy destination is occupied")
    try:
        source_stat = source.lstat()
        if stat.S_ISREG(source_stat.st_mode):
            shutil.copy2(source, destination, follow_symlinks=False)
            _fsync_file(destination)
            _fsync_directory(destination.parent)
            return
        if not stat.S_ISDIR(source_stat.st_mode):
            raise GuardError("unsupported guarded root object")
        destination.mkdir(mode=stat.S_IMODE(source_stat.st_mode))
        children = sorted(os.scandir(source), key=lambda entry: entry.name)
        for child in children:
            child_path = Path(child.path)
            target_path = destination / child.name
            child_stat = child.stat(follow_symlinks=False)
            if stat.S_ISLNK(child_stat.st_mode):
                raise GuardError("symlinks are not supported")
            if stat.S_ISDIR(child_stat.st_mode) or stat.S_ISREG(child_stat.st_mode):
                _copy_tree(child_path, target_path)
            else:
                raise GuardError("unsupported filesystem objects are not allowed")
        shutil.copystat(source, destination, follow_symlinks=False)
        _fsync_directory(destination)
        _fsync_directory(destination.parent)
    except GuardError:
        raise
    except OSError:
        raise GuardError("copy operation failed") from None


def _append_event(journal: Path, event: dict[str, object], *, create: bool = False) -> None:
    try:
        if create:
            handle = journal.open("x", encoding="utf-8")
        else:
            handle = journal.open("a", encoding="utf-8")
        with handle:
            handle.write(json.dumps(event, sort_keys=True) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
    except OSError:
        raise GuardError("journal durability failed") from None


def _load_json(path: Path, *, kind: str) -> object:
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, UnicodeError, json.JSONDecodeError):
        raise GuardError(f"{kind} is malformed") from None


def _require_keys(value: object, keys: set[str], *, kind: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != keys:
        raise GuardError(f"{kind} is malformed")
    return value


def _validate_digest(value: object, *, absent: bool = False) -> str:
    if not isinstance(value, str) or len(value) != DIGEST_LENGTH:
        raise GuardError("manifest digest is malformed")
    try:
        int(value, 16)
    except ValueError:
        raise GuardError("manifest digest is malformed") from None
    if absent and value != ABSENT_DIGEST:
        raise GuardError("manifest absent digest is malformed")
    return value


def _snapshot_from_row(value: object, *, label: str) -> Snapshot:
    if isinstance(value, dict) and "event" in value:
        event_row = _require_keys(
            value,
            {"event", "label", "state", "kind", "digest", "files"},
            kind="journal snapshot",
        )
        value = {
            key: event_row[key]
            for key in ("label", "state", "kind", "digest", "files")
        }
    row = _require_keys(
        value,
        {"label", "state", "kind", "digest", "files"},
        kind="manifest row",
    )
    if row["label"] != label or row["state"] not in {"absent", "present"}:
        raise GuardError("manifest row is malformed")
    state = row["state"]
    kind = row["kind"]
    if not isinstance(state, str) or not isinstance(kind, str):
        raise GuardError("manifest row is malformed")
    if state == "absent":
        if kind != "absent":
            raise GuardError("manifest absent kind is malformed")
    elif kind not in {"directory", "file"}:
        raise GuardError("manifest present kind is malformed")
    files = row["files"]
    if type(files) is not int or files < 0:
        raise GuardError("manifest file count is malformed")
    digest = _validate_digest(row["digest"], absent=state == "absent")
    if state == "absent" and files != 0:
        raise GuardError("manifest absent file count is malformed")
    return Snapshot(state, kind, digest, files)


def _load_manifest(config: GuardConfig) -> dict[str, Snapshot]:
    if not _lexists(config.manifest) or config.manifest.is_symlink():
        raise GuardError("manifest is missing")
    document = _require_keys(
        _load_json(config.manifest, kind="manifest"),
        {"format", "root_order", "roots"},
        kind="manifest",
    )
    if document["format"] != FORMAT or document["root_order"] != list(ROOT_LABELS):
        raise GuardError("manifest header is malformed")
    rows = document["roots"]
    if not isinstance(rows, list) or len(rows) != len(ROOT_LABELS):
        raise GuardError("manifest root count is malformed")
    snapshots: dict[str, Snapshot] = {}
    for label, row in zip(ROOT_LABELS, rows):
        if label in snapshots:
            raise GuardError("manifest contains a duplicate root")
        snapshots[label] = _snapshot_from_row(row, label=label)
    return snapshots


def _load_journal(config: GuardConfig) -> JournalSummary:
    if not _lexists(config.journal) or config.journal.is_symlink():
        raise GuardError("journal is missing")
    try:
        lines = config.journal.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError):
        raise GuardError("journal is malformed") from None
    if not lines:
        raise GuardError("journal is malformed")
    events: list[dict[str, object]] = []
    for line in lines:
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            raise GuardError("journal is malformed") from None
        if not isinstance(value, dict):
            raise GuardError("journal is malformed")
        events.append(value)

    first = _require_keys(events[0], {"event", "format"}, kind="journal")
    if first["event"] != "capture_started" or first["format"] != FORMAT:
        raise GuardError("journal header is malformed")
    captured: dict[str, Snapshot] = {}
    original_moved: dict[str, Snapshot] = {}
    variant_moved: dict[str, Snapshot] = {}
    restored: set[str] = set()
    isolation_complete = False
    complete = False
    phase = "capture"
    for event in events[1:]:
        event_name = event.get("event")
        if complete:
            raise GuardError("journal has events after completion")
        if event_name in {"failure", "interrupted"}:
            row = _require_keys(event, {"event", "reason"}, kind="journal")
            if not isinstance(row["reason"], str) or not row["reason"]:
                raise GuardError("journal failure event is malformed")
            continue

        if phase == "capture":
            if event_name == "captured":
                row = _require_keys(
                    event,
                    {"event", "label", "state", "kind", "digest", "files"},
                    kind="journal",
                )
                label = row["label"]
                if label not in ROOT_LABELS or label in captured:
                    raise GuardError("journal captured event is malformed")
                captured[label] = _snapshot_from_row(row, label=label)
            elif event_name == "capture_complete":
                _require_keys(event, {"event", "format"}, kind="journal")
                if event["format"] != FORMAT or tuple(captured) != ROOT_LABELS:
                    raise GuardError("journal capture completion is malformed")
                phase = "isolation"
            else:
                raise GuardError("journal capture phase is malformed")
            continue

        if phase == "isolation":
            if event_name == "isolation_preflight":
                _require_keys(event, {"event", "format"}, kind="journal")
                if event["format"] != FORMAT:
                    raise GuardError("journal isolation preflight is malformed")
            elif event_name == "original_moved":
                row = _require_keys(
                    event,
                    {"event", "label", "state", "kind", "digest", "files"},
                    kind="journal",
                )
                label = row["label"]
                if label not in ROOT_LABELS or label in original_moved:
                    raise GuardError("journal original move is malformed")
                original_moved[label] = _snapshot_from_row(row, label=label)
            elif event_name == "isolation_complete":
                _require_keys(event, {"event", "format"}, kind="journal")
                if (
                    event["format"] != FORMAT
                    or tuple(original_moved) != ROOT_LABELS
                ):
                    raise GuardError("journal isolation completion is malformed")
                isolation_complete = True
                phase = "restore"
            else:
                raise GuardError("journal isolation phase is malformed")
            continue

        if phase == "restore":
            if event_name == "restore_preflight":
                _require_keys(event, {"event", "format"}, kind="journal")
                if event["format"] != FORMAT:
                    raise GuardError("journal restore preflight is malformed")
            elif event_name == "variant_moved":
                row = _require_keys(
                    event,
                    {"event", "label", "state", "kind", "digest", "files"},
                    kind="journal",
                )
                label = row["label"]
                if label not in ROOT_LABELS or label in variant_moved:
                    raise GuardError("journal variant move is malformed")
                variant_moved[label] = _snapshot_from_row(row, label=label)
            elif event_name == "restored":
                row = _require_keys(event, {"event", "label"}, kind="journal")
                label = row["label"]
                if label not in ROOT_LABELS or label not in variant_moved or label in restored:
                    raise GuardError("journal restored event is malformed")
                restored.add(label)
            elif event_name == "complete":
                _require_keys(event, {"event", "format"}, kind="journal")
                if (
                    event["format"] != FORMAT
                    or tuple(variant_moved) != ROOT_LABELS
                    or restored != set(ROOT_LABELS)
                ):
                    raise GuardError("journal completion is malformed")
                complete = True
            else:
                raise GuardError("journal restore phase is malformed")
            continue

        raise GuardError("journal phase is malformed")

    if phase == "capture" or tuple(captured) != ROOT_LABELS:
        raise GuardError("journal capture record is incomplete")
    return JournalSummary(
        captured,
        original_moved,
        isolation_complete,
        variant_moved,
        frozenset(restored),
        complete,
    )


def _write_manifest(config: GuardConfig, snapshots: dict[str, Snapshot]) -> None:
    document = {
        "format": FORMAT,
        "root_order": list(ROOT_LABELS),
        "roots": [snapshots[label].as_manifest_row(label) for label in ROOT_LABELS],
    }
    try:
        with config.manifest.open("x", encoding="utf-8") as handle:
            json.dump(document, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
    except OSError:
        raise GuardError("manifest durability failed") from None
    _fsync_directory(config.manifest.parent)


def _ensure_new_capture_destinations(config: GuardConfig) -> None:
    if any(
        _lexists(path)
        for path in (
            config.master_root,
            config.original_holding_root,
            config.variant_holding_root,
            config.manifest,
            config.journal,
        )
    ):
        raise GuardError("capture destinations must be new and unoccupied")
    try:
        config.master_root.mkdir()
        config.original_holding_root.mkdir()
        config.variant_holding_root.mkdir()
    except OSError:
        raise GuardError("capture destinations could not be created") from None
    _fsync_directory(config.scope_root)


def _master_path(config: GuardConfig, label: str) -> Path:
    return config.master_root / label


def _original_holding_path(config: GuardConfig, label: str) -> Path:
    return config.original_holding_root / label


def _variant_holding_path(config: GuardConfig, label: str) -> Path:
    return config.variant_holding_root / label


def _verify_master_copies(
    config: GuardConfig, manifest: dict[str, Snapshot]
) -> None:
    if not _lexists(config.master_root) or not config.master_root.is_dir():
        raise GuardError("master root is missing")
    try:
        actual_names = {child.name for child in config.master_root.iterdir()}
    except OSError:
        raise GuardError("master root could not be enumerated") from None
    expected_names = {
        label for label in ROOT_LABELS if manifest[label].state == "present"
    }
    if actual_names != expected_names:
        raise GuardError("master root layout is malformed")
    for label in ROOT_LABELS:
        expected = manifest[label]
        actual = _snapshot_tree(_master_path(config, label))
        if actual != expected:
            raise GuardError("master copy does not match the manifest")


def _verify_retained_copies(
    holding_root: Path, moved: dict[str, Snapshot]
) -> None:
    if not _lexists(holding_root) or not holding_root.is_dir():
        raise GuardError("retained holding root is missing")
    try:
        actual_names = {child.name for child in holding_root.iterdir()}
    except OSError:
        raise GuardError("retained holding root could not be enumerated") from None
    expected_names = {
        label for label, snapshot in moved.items() if snapshot.state == "present"
    }
    if actual_names != expected_names:
        raise GuardError("retained holding layout is malformed")
    for label, source_snapshot in moved.items():
        retained_snapshot = _snapshot_tree(holding_root / label)
        if retained_snapshot != source_snapshot:
            raise GuardError("retained copy does not match its journal")


def _capture(config: GuardConfig) -> None:
    _assert_process_absent(config)
    _ensure_new_capture_destinations(config)
    _append_event(
        config.journal,
        {"event": "capture_started", "format": FORMAT},
        create=True,
    )
    snapshots: dict[str, Snapshot] = {}
    try:
        for root in config.roots:
            before = _snapshot_tree(root.path)
            destination = _master_path(config, root.label)
            if before.state == "present":
                _copy_tree(root.path, destination)
                copied = _snapshot_tree(destination)
                if copied != before:
                    raise GuardError("master copy verification failed")
            elif _lexists(destination):
                raise GuardError("absent root has an unexpected master copy")
            after = _snapshot_tree(root.path)
            if after != before:
                raise GuardError("live source changed during baseline capture")
            snapshots[root.label] = before
            _append_event(
                config.journal,
                {"event": "captured", **before.as_manifest_row(root.label)},
            )
        _write_manifest(config, snapshots)
        _append_event(
            config.journal,
            {"event": "capture_complete", "format": FORMAT},
        )
    except GuardError:
        raise
    print("PASS: new baseline and independent master copies verified")


def _validate_manifest_journal(
    manifest: dict[str, Snapshot], summary: JournalSummary
) -> None:
    for label in ROOT_LABELS:
        if summary.captured[label] != manifest[label]:
            raise GuardError("manifest and capture journal disagree")


def _fault_controls_enabled(args: argparse.Namespace, names: tuple[str, ...]) -> None:
    if any(getattr(args, name, None) is not None for name in names):
        if os.environ.get("MFA_PROFILE_GUARD_TESTING") != "1":
            raise GuardError("synthetic fault controls are disabled")


def _isolate(config: GuardConfig, args: argparse.Namespace) -> None:
    _assert_process_absent(config)
    manifest = _load_manifest(config)
    summary = _load_journal(config)
    _validate_manifest_journal(manifest, summary)
    _fault_controls_enabled(
        args,
        ("synthetic_fail_after_isolate", "synthetic_interrupt_after_isolate"),
    )
    if summary.isolation_complete:
        _verify_master_copies(config, manifest)
        _verify_retained_copies(config.original_holding_root, summary.original_moved)
        for root in config.roots:
            if _snapshot_tree(root.path).state != "absent":
                raise GuardError("completed isolation has a live protected root")
        print("PASS: original roots retained and live roots isolated empty")
        return

    _verify_master_copies(config, manifest)
    _verify_retained_copies(config.original_holding_root, summary.original_moved)
    for root in config.roots:
        if root.label in summary.original_moved:
            if _snapshot_tree(root.path).state != "absent":
                raise GuardError("isolated root has an unexpected live destination")
            continue
        if _lexists(_original_holding_path(config, root.label)):
            raise GuardError("original holding destination is occupied")

    live_before: dict[str, Snapshot] = {}
    for root in config.roots:
        if root.label in summary.original_moved:
            continue
        before = _snapshot_tree(root.path)
        after = _snapshot_tree(root.path)
        if before != manifest[root.label] or after != before:
            raise GuardError("live source changed before isolation")
        live_before[root.label] = after

    _append_event(
        config.journal,
        {"event": "isolation_preflight", "format": FORMAT},
    )
    original_moved = dict(summary.original_moved)
    move_progress = 0
    for root in config.roots:
        if root.label in original_moved:
            continue
        current = _snapshot_tree(root.path)
        if current != live_before[root.label]:
            raise GuardError("live source changed before its isolation move")
        destination = _original_holding_path(config, root.label)
        if current.state == "present":
            try:
                os.rename(root.path, destination)
                _fsync_directory(root.path.parent)
                _fsync_directory(destination.parent)
            except OSError:
                raise GuardError("moving the original source failed") from None
        original_moved[root.label] = current
        _append_event(
            config.journal,
            {"event": "original_moved", **current.as_manifest_row(root.label)},
        )
        move_progress += 1
        if args.synthetic_interrupt_after_isolate == move_progress:
            _append_event(
                config.journal,
                {"event": "interrupted", "reason": "synthetic isolation interruption"},
            )
            raise GuardInterrupted("synthetic isolation interruption recorded")
        if args.synthetic_fail_after_isolate == move_progress:
            raise GuardError("synthetic isolation failure recorded")

    _verify_master_copies(config, manifest)
    _verify_retained_copies(config.original_holding_root, original_moved)
    for root in config.roots:
        if _snapshot_tree(root.path).state != "absent":
            raise GuardError("isolation did not leave a fresh empty root")
    _append_event(
        config.journal,
        {"event": "isolation_complete", "format": FORMAT},
    )
    print("PASS: original roots retained and live roots isolated empty")


def _preflight_restore(
    config: GuardConfig,
    manifest: dict[str, Snapshot],
    summary: JournalSummary,
) -> dict[str, Snapshot]:
    if not summary.isolation_complete or tuple(summary.original_moved) != ROOT_LABELS:
        raise GuardError("restore requires completed pre-launch isolation")
    _verify_master_copies(config, manifest)
    _verify_retained_copies(config.original_holding_root, summary.original_moved)
    _verify_retained_copies(config.variant_holding_root, summary.variant_moved)
    for root in config.roots:
        if root.label in summary.variant_moved:
            if root.label not in summary.restored and _snapshot_tree(root.path).state != "absent":
                raise GuardError("variant-moved root has an unexpected live destination")
            continue
        if _lexists(_variant_holding_path(config, root.label)):
            raise GuardError("variant holding destination is occupied")

    live_before = {
        root.label: _snapshot_tree(root.path)
        for root in config.roots
        if root.label not in summary.variant_moved
    }
    live_after = {
        root.label: _snapshot_tree(root.path)
        for root in config.roots
        if root.label not in summary.variant_moved
    }
    if live_before != live_after:
        raise GuardError("live sources changed during restore preflight")
    return live_after


def _root_by_label(config: GuardConfig, label: str) -> RootSpec:
    for root in config.roots:
        if root.label == label:
            return root
    raise GuardError("unknown canonical root")


def _restore(
    config: GuardConfig,
    args: argparse.Namespace,
) -> None:
    _assert_process_absent(config)
    manifest = _load_manifest(config)
    summary = _load_journal(config)
    _validate_manifest_journal(manifest, summary)
    if not summary.isolation_complete:
        raise GuardError("restore requires completed pre-launch isolation")
    if summary.complete:
        _verify_master_copies(config, manifest)
        _verify_retained_copies(config.original_holding_root, summary.original_moved)
        _verify_retained_copies(config.variant_holding_root, summary.variant_moved)
        for root in config.roots:
            if _snapshot_tree(root.path) != manifest[root.label]:
                raise GuardError("completed restore does not match the manifest")
        print("PASS: retained restore and six-root verification already complete")
        return

    _fault_controls_enabled(
        args,
        (
            "synthetic_fail_after_variant_move",
            "synthetic_interrupt_after_variant_move",
            "synthetic_fail_after_copy",
        ),
    )
    live_snapshots = _preflight_restore(config, manifest, summary)
    _append_event(
        config.journal,
        {"event": "restore_preflight", "format": FORMAT},
    )
    variant_moved = dict(summary.variant_moved)
    restored = set(summary.restored)
    move_progress = 0
    for root in config.roots:
        if root.label in variant_moved:
            continue
        current = _snapshot_tree(root.path)
        if current != live_snapshots[root.label]:
            raise GuardError("live source changed before its variant move")
        destination = _variant_holding_path(config, root.label)
        if current.state == "present":
            try:
                os.rename(root.path, destination)
                _fsync_directory(root.path.parent)
                _fsync_directory(destination.parent)
            except OSError:
                raise GuardError("moving the test variant failed") from None
        variant_moved[root.label] = current
        _append_event(
            config.journal,
            {"event": "variant_moved", **current.as_manifest_row(root.label)},
        )
        move_progress += 1
        if args.synthetic_interrupt_after_variant_move == move_progress:
            _append_event(
                config.journal,
                {"event": "interrupted", "reason": "synthetic restore interruption"},
            )
            raise GuardInterrupted("synthetic restore interruption recorded")
        if args.synthetic_fail_after_variant_move == move_progress:
            raise GuardError("synthetic variant move failure recorded")

    _verify_master_copies(config, manifest)
    _verify_retained_copies(config.original_holding_root, summary.original_moved)
    _verify_retained_copies(config.variant_holding_root, variant_moved)
    copy_progress = 0
    for root in config.roots:
        if root.label in restored:
            continue
        live_path = root.path
        if manifest[root.label].state == "present":
            if _lexists(live_path):
                raise GuardError("live restore destination is occupied")
            _copy_tree(_master_path(config, root.label), live_path)
            if _snapshot_tree(live_path) != manifest[root.label]:
                raise GuardError("copied restore does not match the manifest")
        elif _lexists(live_path):
            raise GuardError("absent root unexpectedly exists after variant move")
        _append_event(config.journal, {"event": "restored", "label": root.label})
        restored.add(root.label)
        copy_progress += 1
        if args.synthetic_fail_after_copy == copy_progress:
            raise GuardError("synthetic copy failure recorded")

    _verify_master_copies(config, manifest)
    _verify_retained_copies(config.original_holding_root, summary.original_moved)
    _verify_retained_copies(config.variant_holding_root, variant_moved)
    for root in config.roots:
        if _snapshot_tree(root.path) != manifest[root.label]:
            raise GuardError("final six-root verification failed")
    _append_event(config.journal, {"event": "complete", "format": FORMAT})
    print("PASS: retained originals, retained variants, and exact six-root restore complete")


def _record_failure(config: GuardConfig | None, reason: str) -> None:
    if config is None or not _lexists(config.journal) or config.journal.is_symlink():
        return
    try:
        summary = _load_journal(config)
        if summary.complete:
            return
    except GuardError:
        return
    try:
        _append_event(config.journal, {"event": "failure", "reason": reason})
    except GuardError:
        return


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Fail-closed six-root guard; never launches or deletes."
    )
    subparsers = parser.add_subparsers(dest="action", required=True)
    for action in ("capture", "isolate", "restore"):
        subparser = subparsers.add_parser(action)
        subparser.add_argument("--native", action="store_true")
        subparser.add_argument("--recovery-root")
        subparser.add_argument("--scope-root")
        subparser.add_argument("--live-root")
        subparser.add_argument("--master-root")
        subparser.add_argument("--original-holding-root")
        subparser.add_argument("--variant-holding-root")
        subparser.add_argument("--manifest")
        subparser.add_argument("--journal")
        subparser.add_argument("--process-pid", type=int)
        subparser.add_argument("--process-name")
        subparser.add_argument("--root", action="append")
        if action == "isolate":
            subparser.add_argument(
                "--synthetic-fail-after-isolate", type=_positive_int
            )
            subparser.add_argument(
                "--synthetic-interrupt-after-isolate", type=_positive_int
            )
        if action == "restore":
            subparser.add_argument(
                "--synthetic-fail-after-variant-move", type=_positive_int
            )
            subparser.add_argument(
                "--synthetic-interrupt-after-variant-move", type=_positive_int
            )
            subparser.add_argument(
                "--synthetic-fail-after-copy", type=_positive_int
            )
    native_paths_parser = subparsers.add_parser("native-paths")
    native_paths_parser.add_argument("--synthetic-os-home", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if args.action == "native-paths":
        if os.environ.get("MFA_PROFILE_GUARD_TESTING") != "1":
            print("BLOCKED: synthetic native path inspection is disabled")
            return 1
        try:
            fake_home = Path(args.synthetic_os_home)
            if not fake_home.is_absolute() or ".." in fake_home.parts:
                raise GuardError("synthetic OS home is malformed")
            print(
                json.dumps(
                    {
                        "bundle_id": NATIVE_BUNDLE_ID,
                        "roots": {
                            label: str(native_paths(fake_home)[label])
                            for label in ROOT_LABELS
                        },
                    },
                    sort_keys=True,
                )
            )
            return 0
        except GuardError as error:
            print(f"BLOCKED: {error}")
            return 1
    config: GuardConfig | None = None
    try:
        config = _build_config(args)
        if args.action == "capture":
            _capture(config)
        elif args.action == "isolate":
            _isolate(config, args)
        else:
            _restore(config, args)
        return 0
    except GuardInterrupted as error:
        print(f"BLOCKED: {error}", file=sys.stderr)
        return INTERRUPTED_EXIT
    except GuardError as error:
        _record_failure(config, str(error))
        print(f"BLOCKED: {error}", file=sys.stderr)
        return 1
    except (OSError, ValueError, TypeError):
        _record_failure(config, "guard failed closed")
        print("BLOCKED: guard failed closed", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
