"""QMP client for input injection (input-send-event) — works headless.

Usage: python qemu-qmp.py <port> <cmd...>
  type "text"          – send key down+up per char
  key <qcode>          – single key (e.g. ret, tab, up)
  abs <x> <y>          – absolute pointer move (screen pixels)
  btn <left|right> <0|1> – pointer button state
"""
import json, socket, sys, time

QCODES = {c: c for c in "abcdefghijklmnopqrstuvwxyz0123456789"}
QCODES.update({
    " ": "spc", "\n": "ret", "-": "minus", "=": "equal", "/": "slash",
    "\\": "backslash", ".": "dot", ",": "comma", "'": "apostrophe",
    '"': "shift-apostrophe", "_": "shift-minus", ":": "shift-semicolon",
    ";": "semicolon",
})

SW, SH = 1280, 800
ABS_MAX = 0x7FFF


class Qmp:
    def __init__(self, port):
        self.s = socket.create_connection(("127.0.0.1", port), timeout=5)
        self.s.settimeout(5)
        self.f = self.s.makefile("rb")
        self._read()  # greeting
        self.cmd({"execute": "qmp_capabilities"})

    def _read(self):
        line = self.f.readline()
        return json.loads(line) if line else {}

    def cmd(self, obj):
        self.s.sendall(json.dumps(obj).encode() + b"\r\n")
        while True:
            r = self._read()
            if "return" in r or "error" in r:
                if "error" in r:
                    print("QMP error:", r["error"], file=sys.stderr)
                return r


def key(q, ch, down):
    q.cmd({"execute": "input-send-event", "arguments": {"events": [
        {"type": "key", "data": {"down": down, "key": {"type": "qcode", "data": ch}}}
    ]}})


def main():
    port, args = int(sys.argv[1]), sys.argv[2:]
    q = Qmp(port)
    while args:
        op = args.pop(0)
        if op == "type":
            for ch in args.pop(0):
                qc = QCODES.get(ch)
                if not qc:
                    print("no qcode for", repr(ch)); continue
                for d in (True, False):
                    key(q, qc, d); time.sleep(0.03)
        elif op == "key":
            qc = args.pop(0)
            for d in (True, False):
                key(q, qc, d); time.sleep(0.03)
        elif op == "abs":
            x = int(int(args.pop(0)) * ABS_MAX / SW)
            y = int(int(args.pop(0)) * ABS_MAX / SH)
            q.cmd({"execute": "input-send-event", "arguments": {"events": [
                {"type": "abs", "data": {"axis": "x", "value": x}},
                {"type": "abs", "data": {"axis": "y", "value": y}},
            ]}})
        elif op == "rel":
            dx = int(args.pop(0)); dy = int(args.pop(0))
            q.cmd({"execute": "input-send-event", "arguments": {"events": [
                {"type": "rel", "data": {"axis": "x", "value": dx}},
                {"type": "rel", "data": {"axis": "y", "value": dy}},
            ]}})
        elif op == "btn":
            b = args.pop(0); down = args.pop(0) == "1"
            q.cmd({"execute": "input-send-event", "arguments": {"events": [
                {"type": "btn", "data": {"button": b, "down": down}},
            ]}})
        else:
            print("unknown:", op); break
        time.sleep(0.05)


if __name__ == "__main__":
    main()
