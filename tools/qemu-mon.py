#!/usr/bin/env python3
"""QEMU monitor helper: screenshots + input injection for headless testing.

Usage:
    python tools/qemu-mon.py <port> shot out.png
    python tools/qemu-mon.py <port> type "uname"          (types + Enter)
    python tools/qemu-mon.py <port> key ret|esc|up|down
    python tools/qemu-mon.py <port> move dx dy [hold]     (relative mouse move)
    python tools/qemu-mon.py <port> click [release]
    python tools/qemu-mon.py <port> cmd "<raw hmp line>"
"""
import re
import socket
import struct
import sys
import time
import zlib

KEYMAP = {
    " ": "spc", "\n": "ret", "-": "-", "=": "=", ".": "dot", ",": "comma",
    "/": "slash", "\\": "backslash", ";": "semicolon", "'": "apostrophe",
    "[": "bracket_left", "]": "bracket_right", "`": "grave_accent",
}


def connect(port):
    s = socket.create_connection(("127.0.0.1", int(port)), timeout=5)
    s.settimeout(2)
    try:
        s.recv(65536)
    except socket.timeout:
        pass
    return s


def cmd(s, line, wait=0.4):
    s.sendall((line + "\n").encode())
    time.sleep(wait)
    try:
        out = s.recv(262144).decode("utf-8", "replace")
    except socket.timeout:
        out = ""
    return re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)


def ppm_to_png(src, dst):
    b = open(src, "rb").read()
    assert b[:2] == b"P6"
    i, vals = 2, []
    while len(vals) < 3:
        while b[i:i + 1].isspace():
            i += 1
        j = i
        while not b[j:j + 1].isspace():
            j += 1
        vals.append(int(b[i:j])); i = j
    i += 1
    w, h, _ = vals
    rgb = b[i:i + w * h * 3]
    raw = b"".join(b"\x00" + rgb[y * w * 3:(y + 1) * w * 3] for y in range(h))

    def chunk(t, d):
        c = t + d
        return struct.pack(">I", len(d)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(raw))
           + chunk(b"IEND", b""))
    open(dst, "wb").write(png)
    return w, h


def main():
    port, op = sys.argv[1], sys.argv[2]
    s = connect(port)
    if op == "shot":
        tmp = "C:/Users/angel/Desktop/UEFI_OS/.shot.ppm"
        cmd(s, f"screendump {tmp}", 0.8)
        w, h = ppm_to_png(tmp, sys.argv[3])
        print(f"{w}x{h} -> {sys.argv[3]}")
    elif op == "type":
        for ch in sys.argv[3]:
            key = KEYMAP.get(ch, ch)
            cmd(s, f"sendkey {key}", 0.08)
        cmd(s, "sendkey ret", 0.08)
    elif op == "key":
        cmd(s, f"sendkey {sys.argv[3]}")
    elif op == "move":
        dx, dy = sys.argv[3], sys.argv[4]
        hold = len(sys.argv) > 5
        if hold:
            cmd(s, "mouse_button 1")
        cmd(s, f"mouse_move {dx} {dy}", 0.1)
        if hold:
            cmd(s, "mouse_button 0")
    elif op == "click":
        cmd(s, "mouse_button 1", 0.1)
        if len(sys.argv) <= 3 or sys.argv[3] != "hold":
            cmd(s, "mouse_button 0", 0.1)
    elif op == "release":
        cmd(s, "mouse_button 0")
    elif op == "cmd":
        print(cmd(s, " ".join(sys.argv[3:])))
    else:
        print(__doc__)
    s.close()


if __name__ == "__main__":
    main()
