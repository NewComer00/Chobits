#!/usr/bin/env python3
"""
openseeface_play_packet.py — Playback .osf.bin files over UDP

Reads a flat binary .osf.bin file (sequence of 1785-byte OpenSeeFace frames)
and sends them over UDP to 127.0.0.1:11573 at 25fps.

Useful for testing OSF expression playback without the full chobits stack.

Usage:
    python openseeface_play_packet.py <input.osf.bin> [--loop]
"""

import socket
import sys
import time
import signal

FRAME_LEN = 1785
FRAME_INTERVAL = 0.04  # 25 fps
UDP_IP = "127.0.0.1"
UDP_PORT = 11573

running = True

def signal_handler(sig, frame):
    global running
    print("\n[play] Stopping...")
    running = False

signal.signal(signal.SIGINT, signal_handler)

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <input.osf.bin> [--loop]")
        sys.exit(1)

    input_path = sys.argv[1]
    loop = "--loop" in sys.argv

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

    with open(input_path, "rb") as f:
        data = f.read()

    frames = [data[i : i + FRAME_LEN] for i in range(0, len(data), FRAME_LEN)]
    # Filter out partial frames at end
    frames = [f for f in frames if len(f) == FRAME_LEN]

    if not frames:
        print(f"[play] No valid frames found in {input_path}")
        sys.exit(1)

    print(f"[play] Loaded {len(frames)} frames from {input_path}")
    print(f"[play] Sending to {UDP_IP}:{UDP_PORT} @ 25fps")
    print(f"[play] Press Ctrl+C to stop")

    while running:
        for i, frame in enumerate(frames):
            if not running:
                break
            sock.sendto(frame, (UDP_IP, UDP_PORT))
            print(f"\r[play] Frame {i + 1}/{len(frames)}", end="", flush=True)
            time.sleep(FRAME_INTERVAL)

        if not loop:
            break
        print(f"\n[play] Looping...")

    sock.close()
    print(f"\n[play] Done.")

if __name__ == "__main__":
    main()
