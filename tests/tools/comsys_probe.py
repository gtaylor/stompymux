#!/usr/bin/env python3
"""Comsys command and Lua API probes in separate temporary schema-32 game copies.

Run from the Rust repository: python3 tests/tools/comsys_probe.py
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
        (game/'lua/global_logic/comsys_probe.lua').write_text("""return {commands={{name='lua-add',permission='wizard',pattern='^lua%-add$',handler=function(ctx) mux.comsys.channel('Probe'):add_player(2,'lua',true);return true end},{name='lua-boot',permission='wizard',pattern='^lua%-boot$',handler=function(ctx) mux.comsys.channel('Probe'):boot_player(2);return true end}}}""")
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
                commands = [(1, '@chan'),
                 (1, '@chan/create Probe'),
                 (1, '@chan/create probe'),
                 (1, '@chan/create Other'),
                 (1, 'addcom a=Probe'),
                 (1, 'addcom b=Probe'),
                 (1, 'addcom a=Probe'),
                 (1, 'addcom a=Other'),
                 (1, 'allcom on'),
                 (1, 'a first'),
                 (1, 'a :poses'),
                 (1, 'a ;suffix'),
                 (1, '@chan/status Probe/full'),
                 (1, '@chan/status/full Probe'),
                 (1, '@chan/who Probe'),
                 (1, 'delcom a'),
                 (1, 'comlist'),
                 (1, 'b afterdel'),
                 (1, 'addcom a=Probe'),
                 (1, 'allcom off'),
                 (1, 'allcom on'),
                 (1, 'delcom b'),
                 (1, 'delcom missing'),
                 (1, '@chan/destroy Probe'),
                 (1, 'comlist'),
                 (1, 'a absent'),
                 (1, '@chan/create Probe'),
                 (1, 'a recreated'),
                 (1, 'addcom p=Probe'),
                 (1, 'lua-add'),
                 (2, 'lua hello'),
                 (1, 'lua-boot'),
                 (2, 'lua afterboot'),
                 (2, 'comlist'),
                 (1, 'clearcom'),
                 (1, 'comlist'),
                 (1, '@chan/who Missing'),
                 (1, '@chan/destroy Missing'),
                 (1, 'addcom'),
                 (1, 'addcom =Probe'),
                 (1, 'addcom a='),
                 (1, 'addcom a=Missing'),
                 (1, 'allcom bad'),
                 (1, '@chan/pflags Probe=bad'),
                 (1, '@chan/oflags Probe=bad'),
                 (1, '@chan/flags Probe=bad'),
                 (1, '@chan/pflags Missing=join'),
                 (1, '@chan/oflags Missing=join'),
                 (1, '@chan/flags Missing=public'),
                 (1, '@chan/emit Missing=hello'),
                 (1, '@chan/status/full Missing'),
                 (1, '@chan/status Probe'),
                 (1, '@chan/emit Probe='),
                 (1, '@chan/emit/noheader Probe='),
                 (1, '@chan/create Probe=ignored'),
                 (1, 'addcom p=Probe'),
                 (1, '@chan/boot Probe=#2'),
                 (1, '@chan/boot Probe=Missing'),
                 (1, '@create Long Channel Object'),
                 (1, '@chan/object Probe=Channel'),
                 (1, '@chan/object Probe=Missing'),
                 (1, '@chan/who Probe/all'),
                 (1, '@chan/status Probe=ignored'),
                 (1, 'comlist ignored'),
                 (1, 'clearcom ignored'),
                 (1, 'addcom too-long=Missing'),
                 (1, 'addcom a=Two Words'),
                 (1, '@chan/create ' + 'x'*50),
                 (1, '@chan/list'),
                 (1, '@chan/list/full ignored'),
                 (1, 'addcom p=Probe'),
                 (1, '@chan/emit probe=case retained'),
                 (1, 'allcom who'), (1, '@chan/emit/noheader Probe=')]
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
