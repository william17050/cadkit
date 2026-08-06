#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import platform
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


def run_capture(args: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=str(cwd),
        text=True,
        capture_output=True,
        check=False,
    )


def git_output(repo_root: Path, *args: str) -> str:
    proc = run_capture(["git", *args], repo_root)
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or f"git {' '.join(args)} failed")
    return proc.stdout.strip()


def expected_executable_path(repo_root: Path) -> Path:
    name = "cadkit.exe" if os.name == "nt" else "cadkit"
    return repo_root / "target" / "debug" / name


def prepare_metadata(repo_root: Path, work_item: str, report_version: str) -> dict:
    return {
        "work_item": work_item,
        "report_version": report_version,
        "utc_timestamp": datetime.now(timezone.utc).isoformat(),
        "git_commit_hash": git_output(repo_root, "rev-parse", "HEAD"),
        "worktree_dirty": bool(git_output(repo_root, "status", "--short")),
        "operating_system": platform.platform(),
        "build_command": "cargo build --workspace",
        "build_result": "pending",
        "executable_path": str(expected_executable_path(repo_root)),
        "executable_exists": False,
        "executable_size_bytes": None,
        "build_duration_seconds": None,
    }


def write_metadata(repo_root: Path, work_item: str, report_version: str, metadata: dict) -> Path:
    out_dir = repo_root / "qa" / "evidence" / work_item / report_version
    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / "build_metadata.json"
    out_path.write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return out_path


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: prepare_qa_build.py CK-XXXX RNNN", file=sys.stderr)
        return 2

    work_item = sys.argv[1].strip()
    report_version = sys.argv[2].strip()
    if not (len(work_item) == 7 and work_item.startswith("CK-") and work_item[3:].isdigit()):
        print("error: work item must match CK-XXXX", file=sys.stderr)
        return 2
    if not (len(report_version) == 4 and report_version.startswith("R") and report_version[1:].isdigit()):
        print("error: report version must match RNNN", file=sys.stderr)
        return 2

    repo_root = Path(__file__).resolve().parent.parent
    metadata = prepare_metadata(repo_root, work_item, report_version)
    build_start = time.perf_counter()
    proc = subprocess.run(
        ["cargo", "build", "--workspace"],
        cwd=str(repo_root),
        text=True,
        capture_output=True,
        check=False,
    )
    duration = round(time.perf_counter() - build_start, 3)

    exe_path = expected_executable_path(repo_root)
    metadata["build_duration_seconds"] = duration
    metadata["build_result"] = "success" if proc.returncode == 0 else "failure"
    metadata["executable_exists"] = exe_path.exists()
    if exe_path.exists():
        metadata["executable_size_bytes"] = exe_path.stat().st_size

    metadata_path = write_metadata(repo_root, work_item, report_version, metadata)

    print(f"work_item: {work_item}")
    print(f"report_version: {report_version}")
    print(f"git_commit_hash: {metadata['git_commit_hash']}")
    print(f"worktree_dirty: {metadata['worktree_dirty']}")
    print(f"build_command: {metadata['build_command']}")
    print(f"build_result: {metadata['build_result']}")
    print(f"build_duration_seconds: {metadata['build_duration_seconds']}")
    print(f"executable_path: {metadata['executable_path']}")
    print(f"executable_exists: {metadata['executable_exists']}")
    print(f"metadata_path: {metadata_path}")

    if proc.returncode != 0:
        if proc.stdout:
            print(proc.stdout, file=sys.stderr, end="" if proc.stdout.endswith("\n") else "\n")
        if proc.stderr:
            print(proc.stderr, file=sys.stderr, end="" if proc.stderr.endswith("\n") else "\n")
        return proc.returncode

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
