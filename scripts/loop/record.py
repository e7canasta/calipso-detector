#!/usr/bin/env python3
"""Records what a detector publishes: subscribes to NATS without dependencies
and writes each message as a line {"subject", "body"} for SECS seconds.

    record.py SUBJECT SECS OUT [NATS_HOST:PORT]
"""
import socket, sys, time

subject, secs, out = sys.argv[1], float(sys.argv[2]), sys.argv[3]
host, port = (sys.argv[4] if len(sys.argv) > 4 else "127.0.0.1:4322").split(":")
s = socket.create_connection((host, int(port)))
s.sendall(b'CONNECT {"verbose":false}\r\nSUB ' + subject.encode() + b" 1\r\n")
s.settimeout(1)
buf, end, n = b"", time.time() + secs, 0
with open(out, "w") as f:
    while time.time() < end:
        try:
            data = s.recv(1 << 20)
        except socket.timeout:
            continue
        buf += data
        while b"\r\n" in buf:
            line, rest = buf.split(b"\r\n", 1)
            if line.startswith(b"PING"):
                s.sendall(b"PONG\r\n")
                buf = rest
                continue
            if line.startswith(b"MSG"):
                parts = line.split()
                size = int(parts[-1])
                if len(rest) < size + 2:
                    break
                f.write('{"subject":"%s","body":%s}\n' % (parts[1].decode(), rest[:size].decode()))
                f.flush()
                n += 1
                buf = rest[size + 2:]
            else:
                buf = rest
print(n, "messages")
