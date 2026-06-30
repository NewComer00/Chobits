#!/usr/bin/env python3
"""
list_vts_hotkeys.py — List VTS hotkeys from live-ascii (or VTube Studio).

Connects to the VTS WebSocket API, authenticates, and prints hotkeys for the
current model. Chobits auto-detects these at startup — this tool is for inspection.

Usage:
    python tool/list_vts_hotkeys.py [--url ws://127.0.0.1:8001]

Requires: pip install websockets
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
import uuid

try:
    import websockets
except ImportError:
    print("Missing dependency: pip install websockets", file=sys.stderr)
    sys.exit(1)

PLUGIN_NAME = "Chobits hotkey lister"
DEVELOPER = "Chobits"


def request(message_type: str, data: dict | None = None) -> dict:
    return {
        "apiName": "VTubeStudioPublicAPI",
        "apiVersion": "1.0",
        "requestID": str(uuid.uuid4()),
        "messageType": message_type,
        "data": data or {},
    }


async def recv_response(ws, expected_type: str) -> dict:
    while True:
        raw = await ws.recv()
        msg = json.loads(raw)
        if msg.get("messageType") == expected_type:
            return msg.get("data", {})
        if msg.get("messageType") == "APIError":
            raise RuntimeError(msg.get("data", msg))


async def main(url: str) -> int:
    async with websockets.connect(url) as ws:
        await ws.send(json.dumps(request("APIStateRequest")))
        await recv_response(ws, "APIStateResponse")

        await ws.send(
            json.dumps(
                request(
                    "AuthenticationTokenRequest",
                    {
                        "pluginName": PLUGIN_NAME,
                        "pluginDeveloper": DEVELOPER,
                        "pluginIcon": "",
                    },
                )
            )
        )
        token_data = await recv_response(ws, "AuthenticationTokenResponse")
        token = token_data["authenticationToken"]

        await ws.send(
            json.dumps(
                request(
                    "AuthenticationRequest",
                    {
                        "pluginName": PLUGIN_NAME,
                        "pluginDeveloper": DEVELOPER,
                        "authenticationToken": token,
                    },
                )
            )
        )
        await recv_response(ws, "AuthenticationResponse")

        await ws.send(json.dumps(request("HotkeysInCurrentModelRequest")))
        data = await recv_response(ws, "HotkeysInCurrentModelResponse")

        model = data.get("modelName", "(unknown)")
        print(f"Model: {model}\n")
        print(f"{'hotkey_id':<40}  name")
        print("-" * 72)
        for hk in data.get("availableHotkeys", []):
            hid = hk.get("hotkeyID", "")
            name = hk.get("name", "")
            print(f"{hid:<40}  {name}")

    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="List VTS hotkeys from live-ascii")
    parser.add_argument(
        "--url",
        default="ws://127.0.0.1:8001",
        help="VTS WebSocket URL (default: ws://127.0.0.1:8001)",
    )
    args = parser.parse_args()
    try:
        raise SystemExit(asyncio.run(main(args.url)))
    except Exception as exc:
        print(f"Error: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
