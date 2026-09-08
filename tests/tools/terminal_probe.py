#!/usr/bin/env python3
"""Optional terminal discovery comparison using isolated C/Rust worlds.

Negotiate TTYPE only; keep raw diagnostics as evidence rather than normalizing
away capability differences. Credentials belong solely to the public fixture.
"""
import json
import pathlib
import select
import shutil
import signal
import socket
import sqlite3
import subprocess
import tempfile
import time

from behavioral_probe import Client, HASH, ROOT


class TerminalClient(Client):
    def __init__(self, port, names):
        super().__init__(port)
        self.names = iter(names)
        self.payload = bytearray()

    def read(self):
        output = bytearray()
        deadline = time.monotonic() + 4
        while time.monotonic() < deadline:
            if not select.select([self.socket], [], [], 0.15)[0]:
                break
            data = self.socket.recv(65536)
            if not data:
                break
            for byte in data:
                if self.state == "text":
                    if byte == 255:
                        self.state = "iac"
                    else:
                        output.append(byte)
                elif self.state == "iac":
                    if byte in (251, 252, 253, 254):
                        self.command, self.state = byte, "option"
                    elif byte == 250:
                        self.payload.clear()
                        self.state = "sb"
                    else:
                        if byte == 255:
                            output.append(byte)
                        self.state = "text"
                elif self.state == "option":
                    if self.command == 253 and byte == 24:
                        self.socket.sendall(bytes([255, 251, byte]))
                    elif self.command in (251, 253):
                        reply = 254 if self.command == 251 else 252
                        self.socket.sendall(bytes([255, reply, byte]))
                    self.state = "text"
                elif self.state == "sb":
                    if byte == 255:
                        self.state = "sb_iac"
                    else:
                        self.payload.append(byte)
                elif self.state == "sb_iac":
                    if byte == 240:
                        self.state = "text"
                        if self.payload == b"\x18\x01":
                            name = next(self.names, None)
                            if name is not None:
                                payload = name.replace(b"\xff", b"\xff\xff")
                                self.socket.sendall(b"\xff\xfa\x18\x00" + payload + b"\xff\xf0")
                    else:
                        if byte == 255:
                            self.payload.append(byte)
                        self.state = "sb"
        return output.decode("utf-8", errors="backslashreplace").replace("\r\n", "\n")


def probe(engine):
    cases = [b"MTTS 265", b"MTTS  265", b"MTTS \t265", b"MTTS \x0b265", b"MTTS +265",
             b"MTTS -0", b"MTTS ", b"MTTS   ", b"MTTS 4294967560",
             b"MTTS 9223372036854775807", b"MTTS 9223372036854775808",
             b"MTTS 265 ", b"MTTS 265\x00ignored", b"bad\xfftype",
             b"X" * 70 + b"TRUECOLOR"]
    with tempfile.TemporaryDirectory(prefix="mux-terminal-audit-") as directory:
        game = pathlib.Path(directory)
        shutil.copytree(ROOT / "tests/fixtures/game", game, dirs_exist_ok=True)
        with sqlite3.connect(game / "data/stompymux.db") as db:
            db.execute("UPDATE player_state SET password_hash=?", [HASH])
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        path = game / "stompymux.toml"
        path.write_text(path.read_text().replace("port = 5555", f"port = {port}")
                        .replace("login_attempt_burst = 3", "login_attempt_burst = 100")
                        .replace("login_hash_limit = 5", "login_hash_limit = 100"))
        binary = ROOT.parent / "btmux-khi/build/stompymux" if engine == "c" else ROOT / "target/debug/stompymux-rs"
        args = [str(binary), "stompymux.toml"] if engine == "c" else [str(binary), "serve", "--game-dir", str(game)]
        with (game / "process.log").open("w+") as log:
            process = subprocess.Popen(args, cwd=game, stdout=log, stderr=log)
            try:
                for _ in range(200):
                    try:
                        admin = Client(port)
                        break
                    except OSError:
                        if process.poll() is not None:
                            log.seek(0)
                            raise RuntimeError(log.read())
                        time.sleep(0.05)
                else:
                    raise RuntimeError("startup timeout")
                try:
                    admin.login("#1")
                    result = []
                    for name in cases:
                        client = TerminalClient(port, [b"AuditClient", b"XTERM", name])
                        try:
                            client.login("#2")
                            admin.read()
                            result.append({"ttype_hex": name.hex(), "output": admin.send("@telnet #2")})
                        finally:
                            client.socket.close()
                        admin.read()
                    return result
                finally:
                    admin.socket.close()
            finally:
                if process.poll() is None:
                    process.send_signal(signal.SIGTERM)
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


if __name__ == "__main__":
    print(json.dumps({engine: probe(engine) for engine in ["c", "rust"]}, indent=2))
