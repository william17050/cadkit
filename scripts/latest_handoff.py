#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path


PATTERN = re.compile(r"^(CK-\d{4})_H(\d{3})\.md$")


def resolve_latest(base_dir: Path, work_item: str) -> Path:
    target_dir = base_dir / "qa" / "handoffs"
    if not target_dir.is_dir():
        raise FileNotFoundError(f"handoff directory not found: {target_dir}")

    best_version = None
    best_path = None
    for path in target_dir.iterdir():
        match = PATTERN.match(path.name)
        if not match:
            continue
        item_id, version_text = match.groups()
        if item_id != work_item:
            continue
        version = int(version_text)
        if best_version is None or version > best_version:
            best_version = version
            best_path = path

    if best_path is None:
        raise FileNotFoundError(f"no handoff found for work item {work_item}")

    return best_path.resolve()


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: latest_handoff.py CK-XXXX", file=sys.stderr)
        return 2

    work_item = sys.argv[1].strip()
    if not re.fullmatch(r"CK-\d{4}", work_item):
        print("error: work item must match CK-XXXX", file=sys.stderr)
        return 2

    repo_root = Path(__file__).resolve().parent.parent
    try:
        print(resolve_latest(repo_root, work_item))
    except FileNotFoundError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
