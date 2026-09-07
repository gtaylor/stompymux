#!/usr/bin/env python3
"""Lock and messaging probes in separate temporary schema-32 game copies.

Run from the Rust repository: python3 tests/tools/lock_message_probe.py
No C dependency is required by cargo test. Raw transcripts preserve ordering;
Telnet negotiations are declined so output is readable and uncompressed.
"""
import argparse
import json
import pathlib
import shutil
import signal
import socket
import sqlite3
import subprocess
import tempfile
import time

from behavioral_probe import Client, ROOT, HASH


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
        keys = "match traverse take use drop give receive enter leave teleport teleport_out link set_home speak channel_join channel_transmit channel_receive".split()
        events = "on_fail on_drop_fail on_give_fail on_give_receive_fail on_use_fail on_enter_fail on_leave_fail on_teleport_destination_fail on_teleport_out_fail on_leave".split()
        for mode in ["allow", "deny", "bare", "error"]:
            body = "local m={locks={},events={}}\n"
            for key in keys:
                result = "return false" if mode == "bare" else "error('PROBE ERROR')" if mode == "error" else "return {passes=" + str(mode == "allow").lower() + ",enactor_message='DENY " + key + "',other_message='BLOCKED " + key + "'}"
                body += "m.locks." + key + "=function(c) mux.world.pemit(1,'LOCK " + key + " object='..tostring(c.object)..' enactor='..tostring(c.enactor)..' subject='..tostring(c.subject)..' silent='..tostring(c.silent)); " + result + " end\n"
            for key in events:
                body += "m.events."+key+"=function(c) mux.world.pemit(1,'EVENT "+key+" object='..tostring(c.object)..' enactor='..tostring(c.enactor)..' subject='..tostring(c.subject)..' operation='..tostring(c.operation)) end\n"
            body += "return m"
            (game / ("lua/object_logic/probe_"+mode+".lua")).write_text(body)
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
                commands = [(1, '@create LockBox'),
                 (1, '@lua/parent #16=probe_deny.lua'),
                 (1, 'drop LockBox'),
                 (1, '@lua/parent #16=probe_bare.lua'),
                 (1, 'give Wizard=LockBox'),
                 (1, '@lua/parent #16=probe_allow.lua'),
                 (1, '@lua/parent #2=probe_bare.lua'),
                 (1, 'give Wizard=LockBox'),
                 (1, '@lua/parent #2=default_player.lua'),
                 (1, '@lua/parent #16=probe_deny.lua'),
                 (1, 'use LockBox'),
                 (1, 'give Wizard=LockBox'),
                 (1, '@lua/parent #16=probe_allow.lua'),
                 (1, 'drop LockBox'),
                 (1, '@lua/parent #16=probe_deny.lua'),
                 (1, 'get LockBox'),
                 (1, 'enter LockBox'),
                 (1, '@lua/parent #16=probe_bare.lua'),
                 (1, 'get LockBox'),
                 (1, '@lua/parent #16=probe_deny.lua'),
                 (1, '@open ear=#4'),
                 (1, 'get ear'),
                 (1, '@flag ear=audible'),
                 (1, '@teleport Wizard=#4'),
                 (1, 'use LockBox'),
                 (1, '@pemit GOD=CONTROL'),
                 (1, 'page GOD=ROUTED PAGE'),
                 (1, '@teleport #2=#0'),
                 (1, '@flag ear=!audible'),
                 (1, '@lua/parent #0=probe_deny.lua'),
                 (1, '@flag here=auditorium'),
                 (1, 'say LOCKED SPEECH'),
                 (1, 'pose LOCKED POSE'),
                 (1, '@emit LOCKED EMIT'),
                 (1, '@flag here=!auditorium'),
                 (1, '@lua/parent #0=default_room.lua'),
                 (1, '@lua/parent #16=probe_error.lua'),
                 (1, 'get LockBox'),
                 (1, 'use LockBox'),
                 (1, '@lua/parent #16=probe_allow.lua'),
                 (1, '@chan/create Probe'),
                 (1, '@force #16=addcom box=Probe'),
                 (1, '@teleport Wizard=#16'),
                 (1, '@chan/emit Probe=CHANNEL LEAK'),
                 (1, '@teleport #2=#0'),
                 (1, '@chan/object Probe=#16'),
                 (1, '@flag #2=!wizard'),
                 (2, 'addcom p=Probe'),
                 (2, 'p TEST'),
                 (1, '@lua/parent #16=probe_deny.lua'),
                 (2, 'p TEST DENIED LOCK OPEN FLAGS'),
                 (2, 'p off'),
                 (2, 'p on'),
                 (1, '@chan/pflags Probe=!receive'),
                 (1, '@chan/emit Probe=RECEIVE DENIED'),
                 (1, '@chan/pflags Probe=!transmit'),
                 (2, 'p TRANSMIT DENIED'),
                 (1, '@chan/pflags Probe=!join'),
                 (2, 'delcom p'),
                 (2, 'addcom p=Probe')]
                for actor, command in commands:
                    client, other = (admin, observer) if actor == 1 else (observer, admin)
                    sent = client.send(command)
                    received = other.read()
                    results.append({"actor": actor, "command": command,
                                    "god": sent if actor == 1 else received,
                                    "wizard": received if actor == 1 else sent})
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
