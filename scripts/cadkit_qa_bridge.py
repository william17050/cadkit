#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path


def next_command_path(commands_dir: Path, action: str) -> Path:
    commands_dir.mkdir(parents=True, exist_ok=True)
    stamp = time.time_ns()
    return commands_dir / f"{stamp}_{os.getpid()}_{action}.json"


def enqueue(root: Path, payload: dict, wait_seconds: float) -> int:
    commands_dir = root / "commands"
    results_dir = root / "results"
    command_path = next_command_path(commands_dir, payload["action"])
    result_path = results_dir / f"{command_path.stem}.result.json"
    command_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    deadline = time.time() + wait_seconds
    while time.time() < deadline:
        if result_path.exists():
            sys.stdout.write(result_path.read_text(encoding="utf-8"))
            return 0
        time.sleep(0.1)

    print(
        f"error: timed out waiting for result {result_path}",
        file=sys.stderr,
    )
    return 1


def print_state(root: Path) -> int:
    state_path = root / "state.json"
    if not state_path.exists():
        print(f"error: state file not found: {state_path}", file=sys.stderr)
        return 1
    sys.stdout.write(state_path.read_text(encoding="utf-8"))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Send commands to the CadKit in-app QA automation bridge."
    )
    parser.add_argument(
        "--wait-seconds",
        type=float,
        default=10.0,
        help="seconds to wait for the app to write a result file",
    )

    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    status = subparsers.add_parser("status", help="print the current bridge state.json")
    status.add_argument("root")

    send = subparsers.add_parser("send", help="send a raw JSON command payload")
    send.add_argument("root")
    send.add_argument("payload_json")

    run_command = subparsers.add_parser("run-command", help="execute a CadKit command alias")
    run_command.add_argument("root")
    run_command.add_argument("text")

    ortho = subparsers.add_parser("ortho", help="set or toggle Ortho mode")
    ortho.add_argument("root")
    ortho.add_argument(
        "mode",
        nargs="?",
        choices=["on", "off", "toggle"],
        default="toggle",
    )

    hover_world_snapped = subparsers.add_parser(
        "hover-world-snapped",
        help="resolve a snapped hover near a world-space location",
    )
    hover_world_snapped.add_argument("root")
    hover_world_snapped.add_argument("x", type=float)
    hover_world_snapped.add_argument("y", type=float)

    click_world_snapped = subparsers.add_parser(
        "click-world-snapped",
        help="deliver a snapped viewport click near a world-space location",
    )
    click_world_snapped.add_argument("root")
    click_world_snapped.add_argument("x", type=float)
    click_world_snapped.add_argument("y", type=float)
    click_world_snapped.add_argument("--additive", action="store_true")

    click_world = subparsers.add_parser("click-world", help="deliver a viewport click in world coordinates")
    click_world.add_argument("root")
    click_world.add_argument("x", type=float)
    click_world.add_argument("y", type=float)
    click_world.add_argument("--additive", action="store_true")

    drag_select = subparsers.add_parser("drag-select-world", help="apply window/crossing selection in world coordinates")
    drag_select.add_argument("root")
    drag_select.add_argument("x0", type=float)
    drag_select.add_argument("y0", type=float)
    drag_select.add_argument("x1", type=float)
    drag_select.add_argument("y1", type=float)
    drag_select.add_argument("--shift", action="store_true")

    for name in ["escape", "delete", "undo", "redo", "quit"]:
        sub = subparsers.add_parser(name, help=f"send {name} to the automation bridge")
        sub.add_argument("root")

    zoom = subparsers.add_parser("zoom", help="apply viewport zoom delta")
    zoom.add_argument("root")
    zoom.add_argument("delta", type=float)

    pan = subparsers.add_parser("pan-screen", help="apply viewport pan using screen-pixel deltas")
    pan.add_argument("root")
    pan.add_argument("dx", type=float)
    pan.add_argument("dy", type=float)

    recovery = subparsers.add_parser("recovery", help="answer the startup recovery prompt")
    recovery.add_argument("root")
    recovery.add_argument("decision", choices=["restore", "discard", "later"])

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    if args.subcommand == "status":
        return print_state(Path(args.root))

    root = Path(args.root)
    if args.subcommand == "send":
        try:
            payload = json.loads(args.payload_json)
        except json.JSONDecodeError as exc:
            print(f"error: invalid JSON payload: {exc}", file=sys.stderr)
            return 2
        if not isinstance(payload, dict) or "action" not in payload:
            print("error: payload must be a JSON object with an action field", file=sys.stderr)
            return 2
        return enqueue(root, payload, args.wait_seconds)

    payload: dict
    match args.subcommand:
        case "run-command":
            payload = {"action": "run_command", "text": args.text}
        case "ortho":
            if args.mode == "toggle":
                payload = {"action": "ortho"}
            else:
                payload = {"action": "ortho", "enabled": args.mode == "on"}
        case "hover-world-snapped":
            payload = {"action": "hover_world_snapped", "x": args.x, "y": args.y}
        case "click-world-snapped":
            payload = {
                "action": "click_world_snapped",
                "x": args.x,
                "y": args.y,
                "additive": args.additive,
            }
        case "click-world":
            payload = {
                "action": "click_world",
                "x": args.x,
                "y": args.y,
                "additive": args.additive,
            }
        case "drag-select-world":
            payload = {
                "action": "drag_select_world",
                "x0": args.x0,
                "y0": args.y0,
                "x1": args.x1,
                "y1": args.y1,
                "shift": args.shift,
            }
        case "escape":
            payload = {"action": "escape"}
        case "delete":
            payload = {"action": "delete"}
        case "undo":
            payload = {"action": "undo"}
        case "redo":
            payload = {"action": "redo"}
        case "quit":
            payload = {"action": "quit"}
        case "zoom":
            payload = {"action": "zoom", "delta": args.delta}
        case "pan-screen":
            payload = {"action": "pan_screen", "dx": args.dx, "dy": args.dy}
        case "recovery":
            payload = {"action": "recovery", "decision": args.decision}
        case _:
            parser.error(f"unsupported subcommand: {args.subcommand}")
            return 2

    return enqueue(root, payload, args.wait_seconds)


if __name__ == "__main__":
    raise SystemExit(main())
