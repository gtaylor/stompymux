#!/usr/bin/env python3
"""Optional C/Rust TCP comparison using separate temporary schema-32 game copies.

Run from the Rust repository: python3 tests/tools/behavioral_probe.py
No C dependency is required by cargo test. Raw transcripts preserve ordering;
Telnet negotiations are declined so output is readable and uncompressed.
"""
import argparse
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

ROOT = pathlib.Path(__file__).resolve().parents[2]
# Public fixture credential: auditpass. Never an operator credential.
HASH = "$argon2id$v=19$m=1024,t=2,p=1$b4UCiMYsjSN0pkluxvuHDw$qktKB/MSoMQBK3/76PJC5vgm86umPr0pOIupBhUEKR8"

class Client:
    """Incrementally decline negotiation, preserving ordinary text and wire order."""
    def __init__(self, port):
        self.socket = socket.create_connection(("127.0.0.1", port), timeout=3)
        self.state = "text"
        self.command = 0

    def read(self):
        output = bytearray()
        deadline = time.monotonic() + 4
        while time.monotonic() < deadline:
            if not select.select([self.socket], [], [], 0.25)[0]:
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
                        self.state = "sb"
                    else:
                        if byte == 255:
                            output.append(byte)
                        self.state = "text"
                elif self.state == "option":
                    if self.command in (251, 253):
                        self.socket.sendall(bytes([255, 254 if self.command == 251 else 252, byte]))
                    self.state = "text"
                elif self.state == "sb":
                    if byte == 255:
                        self.state = "sb_iac"
                elif self.state == "sb_iac":
                    self.state = "text" if byte == 240 else "sb"
        return output.decode("utf-8", errors="backslashreplace").replace("\r\n", "\n")

    def send(self, command):
        self.socket.sendall(command.encode() + b"\r\n")
        return self.read()

    def login(self, name):
        self.read()
        prompt = self.send(name)
        if "Password:" not in prompt:
            raise RuntimeError(f"Missing password prompt: {prompt!r}")
        output = self.send("auditpass")
        if "Connected" not in output:
            raise RuntimeError(f"Login failed: {output!r}")


def probe(engine, binary):
    """Never open a production database; all writes stay inside this temp directory."""
    with tempfile.TemporaryDirectory(prefix=f"mux-parity-{engine}-") as directory:
        game = pathlib.Path(directory)
        shutil.copytree(ROOT / "tests/fixtures/game", game, dirs_exist_ok=True)
        with sqlite3.connect(game / "data/stompymux.db") as db:
            db.execute("UPDATE player_state SET password_hash=?", [HASH])
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        config = game / "stompymux.toml"
        config.write_text(config.read_text().replace("port = 5555", f"port = {port}"))
        (game / "lua/object_logic/audit.lua").write_text('''return {
          messages={describe=function(ctx)
            mux.world.object(ctx.object):state('audit'):set('provider',true)
            return {enactor_message='PROVIDER DESCRIPTION'}
          end}
        }''')
        (game / "lua/object_logic/audit_room.lua").write_text("""return {
          internal_appearance=function() return 'INTERNAL ROOM VIEW' end,
          external_appearance=function() return 'EXTERNAL ROOM VIEW' end
        }""")
        (game / "lua/global_logic/audit.lua").write_text("""return {commands={{
          name='audit-ascii',permission='everyone',pattern='^audit%-ascii$',
          handler=function(ctx)
            for _,value in ipairs({123,'ASCII','line\\nfeed'}) do
              local ok,result=pcall(mux.text.is_printable_ascii,value)
              mux.world.pemit(ctx.enactor,'ASCII '..type(value)..' accepted='..tostring(ok)..(ok and ' result='..tostring(result) or ''))
            end
            return true
          end
        }}}""")
        args = [str(binary.resolve())]
        args += ["stompymux.toml"] if engine == "c" else ["serve", "--game-dir", str(game)]
        clients = []
        with (game / "process.log").open("w+") as log:
            process = subprocess.Popen(args, cwd=game, stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 15
                while True:
                    if process.poll() is not None:
                        log.seek(0)
                        raise RuntimeError(log.read())
                    try:
                        admin = Client(port)
                        break
                    except OSError:
                        if time.monotonic() >= deadline:
                            raise
                        time.sleep(0.05)
                clients.append(admin)
                admin.login("#1")
                observer = Client(port)
                clients.append(observer)
                observer.login("#2")
                admin.read()
                results = []
                for command in [
                    "audit-ascii", "@create Red Sword", "look sword", "look red",
                    "@description Red Sword=STORED DESCRIPTION",
                    "@lua/parent Red Sword=audit.lua", "look Red Sword",
                    "@state/examine Red Sword/audit",
                    "page Wizard MissingPlayer=hello",
                    "page Wizard=hello",
                    "@dig Audit Room", "@lua/parent #17=audit_room.lua",
                    "@open audit-window=#17", "@flag audit-window=transparent",
                    "look audit-window", "goto audit-window", "@teleport #0",
                    "go audit-window", "@teleport #0", "move audit-window", "@teleport #0",
                    "@teleport #17", "@open zonegate=#4", "@teleport #0",
                    "@chzone here=#17", "zonegate", "@chzone #0=#0",
                    "@teleport #0", "@teleport Wizard=me", "page GOD=PRIVATE-PAGE",
                ]:
                    results.append({"command": command, "sender": admin.send(command),
                                    "observer": observer.read()})
                return results
            finally:
                for client in clients:
                    client.socket.close()
                if process.poll() is None:
                    process.send_signal(signal.SIGTERM)
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--c-binary", type=pathlib.Path, default=ROOT.parent / "btmux-khi/build/stompymux")
    parser.add_argument("--rust-binary", type=pathlib.Path, default=ROOT / "target/debug/stompymux-rs")
    args = parser.parse_args()
    print(json.dumps({"c": probe("c", args.c_binary), "rust": probe("rust", args.rust_binary)}, indent=2))

if __name__ == "__main__":
    main()
