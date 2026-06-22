#!/usr/bin/env python3
"""
openseeface_record_packet.py — Capture raw OpenSeeFace UDP frames to .osf.bin

Listens on UDP :11573 for OSF tracking data frames and writes them
verbatim to an .osf.bin file. Press Ctrl+C to stop recording.

Usage:
    python openseeface_record_packet.py <output.osf.bin>

The output file is a flat binary sequence of 1785-byte frames
suitable for playback by chobits-osf.
"""

import socket
import sys
import signal

FRAME_LEN = 1785
UDP_IP = "127.0.0.1"
UDP_PORT = 11573

running = True

def signal_handler(sig, frame):
    global running
    print("\n[record] Stopping...")
    running = False

signal.signal(signal.SIGINT, signal_handler)

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <output.osf.bin>")
        sys.exit(1)

    output_path = sys.argv[1]

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((UDP_IP, UDP_PORT))
    sock.settimeout(0.1)

    print(f"[record] Listening on {UDP_IP}:{UDP_PORT}")
    print(f"[record] Writing frames to {output_path}")
    print(f"[record] Press Ctrl+C to stop")

    frame_count = 0
    with open(output_path, "wb") as f:
        while running:
            try:
                data, addr = sock.recvfrom(FRAME_LEN + 1024)
                if len(data) >= FRAME_LEN:
                    # Write exactly FRAME_LEN bytes per frame
                    f.write(data[:FRAME_LEN])
                    frame_count += 1
                    print(f"\r[record] Frames: {frame_count}", end="", flush=True)
            except socket.timeout:
                continue
            except OSError:
                break

    sock.close()
    print(f"\n[record] Done. Captured {frame_count} frames to {output_path}")

if __name__ == "__main__":
    main()
