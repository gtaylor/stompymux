#!/usr/bin/env python3
"""Run one Lua probe against isolated C and Rust servers and compare exact JSON."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time

MARKER = b"LUA_PARITY:"
OUTPUT_START_MARKER = b"LUA_PARITY_OUTPUT_BEGIN"
PINNED_C_REVISION = "2bbbe6fcdbabe69e229d73089f44bf38f91c0591"

# The pinned C revision's part-name registry exposes manufacturer-branded forms only.  Its
# shipped templates still contain legacy unbranded critical names and consequently fail to load.
# Keep one deliberately small, parser-valid asset in each isolated game so probes can exercise
# the template API itself without mutating either repository's game data.
PARITY_TEMPLATE = """Name { Parity Probe }
Reference { PARITY-PROBE }
Type { Mech }
Move_Type { Biped }
Tons { 75 }
Heat_Sinks { 10 }
Max_Speed { 64.50 }
Specials { ICEEngine_Tech }
Left_Arm
Armor { 10 }
Internals { 5 }
CRIT_1-3 { Agra.IS.PPC - - 5 }
Right_Arm
Armor { 11 }
Internals { 6 }
Left_Torso
Armor { 12 }
Internals { 7 }
Rear { 3 }
Right_Torso
Armor { 13 }
Internals { 8 }
Rear { 4 }
Center_Torso
Armor { 20 }
Internals { 12 }
Rear { 5 }
Left_Leg
Armor { 14 }
Internals { 9 }
Right_Leg
Armor { 15 }
Internals { 10 }
Head
Armor { 9 }
Internals { 3 }
"""


def free_port() -> int:
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def configure(directory: Path, port: int) -> None:
    path = directory / "stompymux.toml"
    source = path.read_text()
    source, count = re.subn(r"(?m)^port\s*=\s*\d+\s*$", f"port = {port}", source)
    if count != 1:
        raise RuntimeError("fixture configuration must contain one server port")
    # Probe serialization is intentionally outside the product contract. Give both isolated
    # fixtures enough room to record large catalogues; semantic resource-limit probes use their
    # own baseline configuration and the frozen AD-RESOURCE-LIMITS rules.
    if not re.search(r"(?m)^instruction_limit\s*=", source):
        source = source.replace(
            "[lua]\n",
            "[lua]\ninstruction_limit = 10000000\noutput_entry_limit = 10000\noutput_byte_limit = 16777216\n",
            1,
        )
    path.write_text(source)


def seed_game(repo: Path, c_root: Path, probe: Path, target: Path) -> None:
    shutil.copytree(repo / "tests/fixtures/game", target, dirs_exist_ok=True)
    lua = target / "lua"
    shutil.copytree(c_root / "game/lua/packages", lua / "packages", dirs_exist_ok=True)
    (lua / "object_logic").mkdir(parents=True, exist_ok=True)
    for name in ("default_thing.lua", "default_room.lua", "default_exit.lua", "default_player.lua"):
        shutil.copy2(c_root / "game/lua/object_logic" / name, lua / "object_logic" / name)
    (lua / "global_logic").mkdir(parents=True, exist_ok=True)
    shutil.copytree(c_root / "game/mechs", target / "mechs", dirs_exist_ok=True)
    shutil.copytree(c_root / "game/maps", target / "maps", dirs_exist_ok=True)
    (target / "mechs/PARITY-PROBE").write_text(PARITY_TEMPLATE)
    probe_templates = repo / "tests/fixtures/lua-probes/templates"
    if probe_templates.is_dir():
        shutil.copytree(probe_templates, target / "mechs", dirs_exist_ok=True)
    for database in (target / "data").glob("stompymux.db*"):
        database.unlink()
    destination = target / "lua/global_logic/lua_parity_probe.lua"
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(probe, destination)


class Client:
    def __init__(self, socket_: socket.socket):
        self.socket = socket_
        self.pending = bytearray()

    def until(self, needle: bytes, deadline: float) -> bytes:
        while needle not in self.pending:
            if time.monotonic() >= deadline:
                raise TimeoutError(
                    f"timed out waiting for {needle!r}; received {self.pending[-1000:]!r}"
                )
            try:
                chunk = self.socket.recv(65536)
            except socket.timeout:
                continue
            if not chunk:
                raise RuntimeError(f"server closed while waiting for {needle!r}")
            self.pending.extend(chunk)
        end = self.pending.index(needle) + len(needle)
        result = bytes(self.pending[:end])
        del self.pending[:end]
        return result

    def send(self, line: bytes) -> None:
        self.socket.sendall(line + b"\r\n")


def connect(port: int, deadline: float) -> Client:
    while time.monotonic() < deadline:
        if time.monotonic() >= deadline:
            break
        try:
            socket_ = socket.create_connection(("127.0.0.1", port), timeout=0.2)
            socket_.settimeout(0.2)
            return Client(socket_)
        except OSError:
            time.sleep(0.05)
    raise TimeoutError("server did not accept a loopback connection")


def invoke(
    command: list[str],
    directory: Path,
    port: int,
    timeout: float,
    run_probe: bool = True,
    setup: list[str] | None = None,
) -> dict | None:
    environment = os.environ.copy()
    environment["BTECH_TEST_GOD_PASSWORD"] = "btmuxr0x"
    log_path = directory / "lua-parity-server.log"
    log = log_path.open("wb")
    process = subprocess.Popen(command, cwd=directory, env=environment, stdout=log, stderr=subprocess.STDOUT)
    deadline = time.monotonic() + timeout
    try:
        client = connect(port, deadline)
        with client.socket:
            if not run_probe:
                return None
            client.until(b"Who are you?", deadline)
            client.send(b"GOD")
            client.until(b"Password:", deadline)
            client.send(b"btmuxr0x")
            client.until(b"Connected.", deadline)
            for line in setup or []:
                # Probe-declared prelude: identical socket lines on both servers,
                # processed in order before the first probe command.
                client.send(line.encode("utf-8"))
                time.sleep(0.1)
            def read_result(command_name: bytes) -> dict:
                client.send(command_name)
                preceding = client.until(MARKER, deadline)[: -len(MARKER)]
                payload = client.until(b"\r\n", deadline)[:-2]
                chunk = re.fullmatch(rb"(\d+)/(\d+):(.*)", payload, re.DOTALL)
                if chunk:
                    index, total = int(chunk.group(1)), int(chunk.group(2))
                    if index != 1 or total < 1:
                        raise RuntimeError("probe emitted an invalid first chunk")
                    chunks = [chunk.group(3)]
                    for expected in range(2, total + 1):
                        client.until(MARKER, deadline)
                        next_payload = client.until(b"\r\n", deadline)[:-2]
                        next_chunk = re.fullmatch(rb"(\d+)/(\d+):(.*)", next_payload, re.DOTALL)
                        if not next_chunk or int(next_chunk.group(1)) != expected or int(next_chunk.group(2)) != total:
                            raise RuntimeError(f"probe emitted an invalid chunk {expected} of {total}")
                        chunks.append(next_chunk.group(3))
                    payload = b"".join(chunks)
                decoded = json.loads(payload.decode("utf-8"))
                if decoded.pop("__capture_output", False):
                    start = preceding.rfind(OUTPUT_START_MARKER)
                    if start < 0:
                        raise RuntimeError("output-capturing probe omitted its start marker")
                    start += len(OUTPUT_START_MARKER)
                    if preceding[start : start + 2] == b"\r\n":
                        start += 2
                    decoded["captured_output_bytes"] = preceding[start:].hex()
                return decoded

            result = read_result(b"luaparity")
            commands = result.pop("__next_commands", [])
            if commands:
                pages = [result]
                for command_name in commands:
                    if not isinstance(command_name, str) or not re.fullmatch(r"luaparity[0-9]+", command_name):
                        raise RuntimeError("probe requested an invalid follow-up command")
                    pages.append(read_result(command_name.encode("ascii")))
                return {"pages": pages}
            return result
    except Exception:
        log.flush()
        print(log_path.read_text(errors="replace"))
        raise
    finally:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=3)
        log.close()


def normalize_incidental(value, path):
    """Normalize only the isolated probe file spelling in Lua argument errors."""
    if not (
        re.search(r"(?:^|\.)error\.entries\[[0-9]+\]\.value\.bytes$", path)
        and isinstance(value, str)
    ):
        return value
    try:
        text = bytes.fromhex(value).decode("utf-8")
    except (ValueError, UnicodeDecodeError):
        return value
    c_match = re.fullmatch(
        r".+/c/lua/global_logic/lua_parity_probe\.lua:(\d+): (bad argument #[0-9]+ to .*)",
        text,
    )
    rust_match = re.fullmatch(
        r'\[string "global_logic/lua_parity_probe\.lua"\]:(\d+): (bad argument #[0-9]+ to .*)',
        text,
    )
    match = c_match or rust_match
    if match is None:
        return value
    return (f"<probe>:{match.group(1)}: {match.group(2)}").encode().hex()


def compare(c_value, rust_value, path="") -> list[dict]:
    if type(c_value) is not type(rust_value):
        return [{"kind": "mismatch", "path": path, "c": c_value, "rust": rust_value}]
    if isinstance(c_value, dict):
        issues = []
        for key in sorted(c_value.keys() - rust_value.keys()):
            issues.append({"kind": "rust_missing", "path": f"{path}.{key}".strip("."), "c": c_value[key]})
        for key in sorted(rust_value.keys() - c_value.keys()):
            issues.append({"kind": "rust_extra", "path": f"{path}.{key}".strip("."), "rust": rust_value[key]})
        for key in sorted(c_value.keys() & rust_value.keys()):
            child_path = f"{path}.{key}".strip(".")
            issues.extend(compare(c_value[key], rust_value[key], child_path))
        return issues
    if isinstance(c_value, list):
        if c_value == rust_value:
            return []
        # These two probe shapes are explicitly sorted unique key inventories. Preserve their
        # useful per-item extension classification; every other list is an ordered Lua sequence.
        set_like_inventory = path == "globals" or (
            path.startswith("namespaces.") and path.endswith(".keys")
        )
        if set_like_inventory and all(
            isinstance(item, (str, int, float, bool)) for item in c_value + rust_value
        ):
            if c_value != sorted(set(c_value)) or rust_value != sorted(set(rust_value)):
                return [{
                    "kind": "mismatch",
                    "path": path,
                    "c": c_value,
                    "rust": rust_value,
                    "detail": "key inventories must be sorted and unique",
                }]
            issues = []
            for item in c_value:
                if item not in rust_value:
                    issues.append({"kind": "rust_missing", "path": path, "c": item})
            for item in rust_value:
                if item not in c_value:
                    issues.append({"kind": "rust_extra", "path": path, "rust": item})
            return issues
        issues = []
        for index, (left, right) in enumerate(zip(c_value, rust_value)):
            issues.extend(compare(left, right, f"{path}[{index}]"))
        if len(c_value) != len(rust_value):
            issues.append({"kind": "mismatch", "path": f"{path}.length", "c": len(c_value), "rust": len(rust_value)})
        return issues
    c_compared = normalize_incidental(c_value, path)
    rust_compared = normalize_incidental(rust_value, path)
    return [] if c_compared == rust_compared else [
        {"kind": "mismatch", "path": path, "c": c_value, "rust": rust_value}
    ]


def verify_comparator() -> None:
    if not compare([1, 2], [2, 1], "ordered"):
        raise RuntimeError("probe comparator failed to detect reordered values")
    if not compare([1, 1], [1], "ordered"):
        raise RuntimeError("probe comparator failed to detect duplicate-count changes")
    if not compare([1, 2], [2, 1], "state.keys"):
        raise RuntimeError("probe comparator treated an ordinary keys() result as a set")
    if not compare([1, 2], [2, 1], "namespaces.example.keys"):
        raise RuntimeError("probe comparator accepted an unsorted namespace key inventory")
    c_error = "/tmp/run/c/lua/global_logic/lua_parity_probe.lua:7: bad argument #2 to 'f' (detail one)".encode().hex()
    rust_error = '[string "global_logic/lua_parity_probe.lua"]:7: bad argument #2 to \'f\' (detail one)'.encode().hex()
    error_path = "probe.error.entries[2].value.bytes"
    if compare(c_error, rust_error, error_path):
        raise RuntimeError("probe comparator retained an incidental Lua source spelling")
    changed = "/tmp/run/c/lua/global_logic/lua_parity_probe.lua:7: bad argument #2 to 'f' (detail two)".encode().hex()
    if not compare(changed, rust_error, error_path):
        raise RuntimeError("probe comparator normalized an argument error detail")
    unrelated = "/tmp/not-the-probe.lua:7: bad argument #2 to 'f' (detail one)".encode().hex()
    if not compare(unrelated, rust_error, error_path):
        raise RuntimeError("probe comparator normalized an unrelated source name")
    if not compare(c_error, rust_error, "state.value.bytes"):
        raise RuntimeError("probe comparator normalized ordinary string data")


def main() -> int:
    verify_comparator()
    parser = argparse.ArgumentParser()
    parser.add_argument("--probe", type=Path, required=True)
    parser.add_argument("--c-binary", type=Path)
    parser.add_argument("--rust-binary", type=Path)
    parser.add_argument("--timeout", type=float, default=15.0)
    parser.add_argument("--keep", action="store_true")
    parser.add_argument(
        "--allow-rust-extra",
        action="append",
        default=[],
        metavar="PATH=JSON_VALUE",
        help="permit one explicitly documented Rust-only list value",
    )
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    c_root = repo.parent / "btmux-khi"
    c_binary = (args.c_binary or c_root / "build/stompymux").resolve()
    rust_binary = (args.rust_binary or repo / "target/debug/stompymux-rs").resolve()
    probe = args.probe.resolve()
    for path in (c_binary, rust_binary, probe):
        if not path.is_file():
            parser.error(f"required file is unavailable: {path}")
    # Opt-in prelude declared by the probe itself. A `-- PARITY_SETUP: <line>`
    # comment makes the harness send <line> to both isolated servers, in file
    # order, after login and before the first `luaparity` command. Probes
    # without the marker run exactly as before. Used to build registered live
    # units (create via a probe command, then `@btech/register <name>=MECH`).
    setup = [
        match.group(1).strip()
        for match in (
            re.fullmatch(r"--\s*PARITY_SETUP:\s*(.*)", line.strip())
            for line in probe.read_text(encoding="utf-8").splitlines()
        )
        if match and match.group(1).strip()
    ]
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=c_root, text=True
    ).strip()
    if revision != PINNED_C_REVISION:
        parser.error(f"C checkout is {revision}, expected pinned {PINNED_C_REVISION}")
    allowed = set()
    for item in args.allow_rust_extra:
        if "=" not in item:
            parser.error("--allow-rust-extra requires PATH=JSON_VALUE")
        path, value = item.split("=", 1)
        allowed.add((path, json.dumps(json.loads(value), sort_keys=True)))

    temporary = Path(tempfile.mkdtemp(prefix="lua-parity-"))
    try:
        seed = temporary / "seed"
        seed_game(repo, c_root, probe, seed)
        (seed / "lua/global_logic/lua_parity_probe.lua").unlink()
        seed_port = free_port()
        configure(seed, seed_port)
        invoke([str(c_binary), "stompymux.toml"], seed, seed_port, args.timeout, False)
        if not (seed / "data/stompymux.db").is_file():
            raise RuntimeError("C bootstrap did not create the fixture database")
        c_game = temporary / "c"
        rust_game = temporary / "rust"
        shutil.copytree(seed, c_game)
        shutil.copytree(seed, rust_game)
        for game in (c_game, rust_game):
            destination = game / "lua/global_logic/lua_parity_probe.lua"
            shutil.copy2(probe, destination)
        c_port = free_port()
        configure(c_game, c_port)
        c_result = invoke([str(c_binary), "stompymux.toml"], c_game, c_port, args.timeout, setup=setup)
        rust_port = free_port()
        configure(rust_game, rust_port)
        rust_result = invoke(
            [str(rust_binary), "serve", "--game-dir", str(rust_game), "--port", str(rust_port)],
            rust_game,
            rust_port,
            args.timeout,
            setup=setup,
        )
        issues = compare(c_result, rust_result)
        permitted = []
        failures = []
        for issue in issues:
            key = (issue["path"], json.dumps(issue.get("rust"), sort_keys=True))
            if issue["kind"] == "rust_extra" and key in allowed:
                permitted.append(issue)
            else:
                failures.append(issue)
        report = {"c": c_result, "rust": rust_result, "permitted": permitted, "failures": failures}
        print(json.dumps(report, indent=2, sort_keys=True))
        if failures:
            return 1
        return 0
    finally:
        if args.keep:
            print(f"kept isolated games at {temporary}", file=sys.stderr)
        else:
            shutil.rmtree(temporary, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
