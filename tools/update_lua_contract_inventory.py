#!/usr/bin/env python3
"""Regenerate the pinned C Lua callable and contract inventories."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
C_REPO = ROOT.parent / "btmux-khi"
C_REVISION = "2bbbe6fcdbabe69e229d73089f44bf38f91c0591"
RUST_BASELINE = "7917a95642e2fe690bf0ac03a018a0e2b7342b21"

SOURCE_BY_PREFIX = {
    "btech.autopilot": "src/mux/lua/packages/btech/autopilot/btech_autopilot_bindings.c",
    "btech.character": "src/mux/lua/packages/btech/character/btech_character_bindings.c",
    "btech.map.line_of_sight": "src/mux/lua/packages/btech/map/btech_map_los_bindings.c",
    "btech.map": "src/mux/lua/packages/btech/map/btech_map_bindings.c",
    "btech.parts": "src/mux/lua/packages/btech/parts/btech_parts_bindings.c",
    "btech.player": "src/mux/lua/packages/btech/player/btech_player_bindings.c",
    "btech.repair": "src/mux/lua/packages/btech/repair/btech_repair_bindings.c",
    "btech.system": "src/mux/lua/packages/btech/system/btech_system_bindings.c",
    "btech.template": "src/mux/lua/packages/btech/template/btech_template_bindings.c",
    "btech.unit": "src/mux/lua/packages/btech/unit/btech_unit_bindings.c",
    "ChannelFlags:": "src/mux/lua/packages/mux/comsys/mux_comsys_channel_flag_bindings.c",
    "Channel:": "src/mux/lua/packages/mux/comsys/mux_comsys_bindings.c",
    "mux.comsys": "src/mux/lua/packages/mux/comsys/mux_comsys_bindings.c",
    "mux.config": "src/mux/lua/packages/mux/config/mux_config_bindings.c",
    "mux.error": "src/mux/lua/packages/mux/error/mux_error_bindings.c",
    "mux.session": "src/mux/lua/packages/mux/session/mux_session_bindings.c",
    "mux.telnet": "src/mux/lua/packages/mux/telnet/mux_telnet_bindings.c",
    "mux.text": "src/mux/lua/packages/mux/text/mux_text_bindings.c",
    "Object:": "src/mux/lua/packages/mux/world/mux_object_bindings.c",
    "Flags:": "src/mux/lua/packages/mux/world/mux_flag_power_bindings.c",
    "Powers:": "src/mux/lua/packages/mux/world/mux_flag_power_bindings.c",
    "State:": "src/mux/lua/packages/mux/world/mux_state_bindings.c",
    "Error:": "src/mux/lua/lua_error.c",
    "mux.world": "src/mux/lua/packages/mux/world/mux_world_bindings.c",
    "mux.": "src/mux/lua/packages/mux/mux_package.c",
}

CONSTANT_CLASSES = {
    "ChannelFlagNamespace": "mux.comsys.flags",
    "AccessNamespace": "mux.world.access",
    "FlagNamespace": "mux.world.flags",
    "LockNamespace": "mux.world.locks",
    "PowerNamespace": "mux.world.powers",
    "ObjectTypeNamespace": "mux.world.types",
    "BtechAutopilotOrderNamespace": "btech.autopilot.orders",
    "BtechAutopilotDirectionNamespace": "btech.autopilot.directions",
    "BtechAutopilotRoamModeNamespace": "btech.autopilot.roam_modes",
    "BtechAutopilotAutogunModeNamespace": "btech.autopilot.autogun_modes",
    "BtechRepairOperationNamespace": "btech.repair.operations",
    "BtechUnitTypeNamespace": "btech.unit.types",
    "BtechMovementTypeNamespace": "btech.unit.movement_types",
    "BtechSectionNamespace": "btech.unit.sections",
    "BtechTechnologyCodeNamespace": "btech.unit.technology",
    "BtechTechnologyGroupNamespace": "btech.unit.technology_groups",
    "BtechFireModeNamespace": "btech.unit.fire_modes",
    "BtechAmmunitionModeNamespace": "btech.unit.ammunition_modes",
}

ERROR_CODES = [
    "mux.arg.invalid", "mux.unavailable.checking", "mux.runtime",
    "mux.state.invalid", "mux.state.value_too_large", "mux.state.unavailable",
    "mux.object.invalid", "mux.object.unavailable", "mux.flag.invalid",
    "mux.power.invalid", "mux.access.invalid", "mux.connection.invalid",
    "mux.connection.unavailable", "mux.channel.invalid", "mux.channel_flag.invalid",
    "mux.text.invalid", "mux.module.invalid", "mux.module.unavailable",
    "mux.config.not_found", "mux.config.unsupported", "btech.part.not_found",
    "btech.part.ambiguous", "btech.part.wrong_kind", "btech.template.not_found",
    "btech.template.invalid", "btech.operation.failed", "testing.assertion",
    "testing.runtime", "mux.internal",
]

BLOCKED_PREFIXES = ("btech.autopilot.", "btech.repair.")

MUX_EXACT_SOURCES = {
    **{f"Object:{name}": "src/mux/lua/packages/mux/world/mux_object_relationship_bindings.c" for name in
       ("destination", "set_destination", "home", "set_home", "location", "zone", "set_zone",
        "affiliation", "set_affiliation", "lua_parent", "set_lua_parent")},
    "Object:flags": "src/mux/lua/packages/mux/world/mux_flag_power_bindings.c",
    "Object:powers": "src/mux/lua/packages/mux/world/mux_flag_power_bindings.c",
    "Object:state": "src/mux/lua/packages/mux/world/mux_state_bindings.c",
    "Channel:flags": "src/mux/lua/packages/mux/comsys/mux_comsys_channel_flag_bindings.c",
    "mux.world.object": "src/mux/lua/packages/mux/world/mux_object_bindings.c",
    "mux.world.lock_passes": "src/mux/lua/packages/mux/world/mux_lock_bindings.c",
}
def git_show(path: str) -> str:
    return subprocess.check_output(
        ["git", "show", f"{C_REVISION}:{path}"], cwd=C_REPO, text=True
    )


def source_for(symbol: str) -> str:
    if symbol in MUX_EXACT_SOURCES:
        return MUX_EXACT_SOURCES[symbol]
    if symbol.startswith("btech."):
        package, leaf = symbol.rsplit(".", 1)
        package_name = package.split(".")[1]
        prefix = f"src/mux/lua/packages/btech/{package_name}/"
        matches = subprocess.run(
            ["git", "grep", "-l", f'\"{leaf}\"', C_REVISION, "--", prefix],
            cwd=C_REPO, text=True, capture_output=True, check=False,
        ).stdout.splitlines()
        paths = [row.split(":", 1)[1] for row in matches]
        if len(paths) == 1:
            return paths[0]
    for prefix in sorted(SOURCE_BY_PREFIX, key=len, reverse=True):
        if symbol.startswith(prefix):
            return SOURCE_BY_PREFIX[prefix]
    raise ValueError(f"no source mapping for {symbol}")


def registered_btech_symbols() -> list[str]:
    """Read BattleTech public paths from the typed native registration arrays."""
    paths = subprocess.check_output(
        ["git", "ls-tree", "-r", "--name-only", C_REVISION, "src/mux/lua"],
        cwd=C_REPO,
        text=True,
    ).splitlines()
    symbols: set[str] = set()
    for path in paths:
        if "/packages/btech/" not in path or not path.endswith(("_bindings.c", "_operations.c")):
            continue
        text = git_show(path)
        for array in re.finditer(
            r"(?:static\s+)?const BtechLuaNativeEntry\s+\w+\[\]\s*=\s*\{(.*?)\n\s*\};",
            text,
            re.S,
        ):
            for match in re.finditer(r'\{\s*"[^"]+"\s*,\s*"([A-Za-z][A-Za-z0-9_.]+)"\s*,', array.group(1)):
                symbols.add("btech." + match.group(1))
    return sorted(symbols)


def registered_mux_symbols(entries: list[dict[str, object]]) -> list[str]:
    """Fail closed unless every declared MUX callable has a native registration site."""
    registered = []
    for entry in entries:
        symbol = str(entry["symbol"])
        leaf = symbol.rsplit(":", 1)[-1].rsplit(".", 1)[-1]
        source = str(entry["reference"])
        text = git_show(source)
        literal_field = re.search(
            rf'lua_setfield\s*\([^;]*"{re.escape(leaf)}"\s*\)', text, re.S
        )
        typed_array = re.search(
            rf'\{{\s*"{re.escape(leaf)}"\s*,\s*lua_[A-Za-z0-9_]+\s*\}}', text
        )
        if literal_field is None and typed_array is None:
            raise ValueError(f"MUX callable lacks native registration evidence: {symbol} in {source}")
        registered.append(symbol)
    if len(registered) != 89:
        raise ValueError(f"expected 89 registered MUX callables, found {len(registered)}")
    return sorted(registered)


def package_for(symbol: str) -> str:
    if ":" in symbol:
        return "mux.userdata"
    return ".".join(symbol.split(".")[:-1])


def status_for(symbol: str) -> tuple[str, str, str]:
    if symbol.startswith(BLOCKED_PREFIXES):
        domain = "autopilot queue/runtime" if ".autopilot." in symbol else "repair queue and technician runtime"
        return ("prerequisite-blocked", domain, "no corresponding Rust gameplay subsystem at the pinned baseline")
    if symbol.startswith("mux.") or ":" in symbol:
        return ("actionable", "", "Rust binding is present; per-callable C contract audit is still pending")
    return ("actionable", "", "Rust domain support exists or the gap is Lua binding/instrumentation plumbing")


def callable_entries(path: str, root: str) -> list[dict[str, object]]:
    text = git_show(path)
    lines = text.splitlines()
    aliases = {"mux_comsys": "mux.comsys", "mux_config": "mux.config", "mux_error": "mux.error",
               "mux_session": "mux.session", "mux_telnet": "mux.telnet", "mux_text": "mux.text",
               "mux_world": "mux.world"}
    aliases.update({f"btech_{name}": f"btech.{name}" for name in
                    ("autopilot", "character", "map", "parts", "player", "repair", "system", "template", "unit")})
    found = []
    for index, line in enumerate(lines):
        match = re.match(r"function ([A-Za-z_][\w.:]*?)\((.*?)\) end$", line)
        if not match:
            continue
        raw, arguments = match.groups()
        head = re.split(r"[.:]", raw, maxsplit=1)[0]
        symbol = aliases.get(head, head) + raw[len(head):]
        block = []
        cursor = index - 1
        while cursor >= 0 and lines[cursor].startswith("---"):
            block.append(lines[cursor][3:].strip())
            cursor -= 1
        block.reverse()
        returns = [row.removeprefix("@return ") for row in block if row.startswith("@return ")]
        prose = " ".join(row for row in block if row and not row.startswith("@") and row != "---")
        status, prerequisite, rust_evidence = status_for(symbol)
        found.append({
            "symbol": symbol, "kind": "callable", "package": package_for(symbol),
            "arguments": arguments, "returns": returns, "reference": source_for(symbol),
            "contract": prose or "Contract is defined by the pinned native C implementation and generated Lua declaration.",
            "status": status, "prerequisite": prerequisite, "rust_evidence": rust_evidence,
            "acceptance": "compare argument conversion, return count/shape, structured errors, and state/output effects against the pinned C contract",
            "accepted_difference_ids": [],
        })
    return found


def constants(text: str) -> list[dict[str, object]]:
    entries = []
    current = None
    for line in text.splitlines():
        class_match = re.match(r"---@class(?: \(exact\))? (\w+)", line)
        if class_match:
            current = class_match.group(1)
            continue
        field = re.match(r"---@field ([A-Z][A-Z0-9_]*) (\w+)(?: (.*))?", line)
        if not field:
            continue
        if field.group(2) in {
            "Access", "Flag", "Lock", "Power", "ObjectType", "RoomObjectType",
            "ThingObjectType", "ExitObjectType", "PlayerObjectType", "ChannelFlag",
            "BtechAutopilotOrderName", "BtechAutopilotDirection", "BtechAutopilotRoamMode",
            "BtechAutopilotAutogunMode", "BtechRepairOperation", "BtechUnitType",
            "BtechMovementType", "BtechSection", "BtechTechnologyCode",
            "BtechTechnologyGroup", "BtechFireMode", "BtechAmmunitionMode",
        } and current not in CONSTANT_CLASSES:
            raise ValueError(f"unmapped typed constant namespace class {current}")
        if current not in CONSTANT_CLASSES:
            continue
        namespace = CONSTANT_CLASSES[current]
        symbol = f"{namespace}.{field.group(1)}"
        status = "actionable"
        entries.append({
            "symbol": symbol, "kind": "constant", "package": namespace,
            "arguments": "", "returns": [field.group(2)],
            "reference": "game/lua/types/btech.d.lua" if symbol.startswith("btech.") else "game/lua/types/mux.d.lua",
            "contract": (field.group(3) or "Typed immutable constant.").strip(), "status": status,
            "prerequisite": "", "rust_evidence": "typed constant contract audit remains pending; BattleTech canonical namespaces are absent",
            "acceptance": "constant exists at the exact path, has typed identity/equality and tostring behavior, and its namespace rejects mutation and unknown keys",
            "accepted_difference_ids": [],
        })
    return entries


def supplemental_entries() -> list[dict[str, object]]:
    entries = []
    metamethods = {
        "Object:__tostring": ("object", "Formats a generation-checked Object as object(#dbref); stale handles raise mux.object.invalid."),
        "Object:__eq": ("object", "Compares Object runtime, dbref, and generation identity and returns one boolean."),
        "Channel:__tostring": ("channel", "Formats a live Channel; stale channels raise mux.channel.invalid."),
        "Channel:__eq": ("channel", "Compares channel package, native identity, and generation and returns one boolean."),
        "Channel:__newindex": ("channel", "Rejects assignment to Channel userdata with mux.arg.invalid."),
        "ChannelFlags:__tostring": ("channel_flags", "Formats a live channel flag-set handle and preserves stale-channel errors."),
        "Flags:__tostring": ("object_set", "Formats a live object flag-set handle and preserves stale-object errors."),
        "Powers:__tostring": ("object_set", "Formats a live object power-set handle and preserves stale-object errors."),
        "State:__tostring": ("state", "Formats a generation-checked state handle and preserves stale-object errors."),
    }
    meta_sources = {
        "object": "src/mux/lua/packages/mux/world/mux_object_bindings.c",
        "channel": "src/mux/lua/packages/mux/comsys/mux_comsys_bindings.c",
        "channel_flags": "src/mux/lua/packages/mux/comsys/mux_comsys_channel_flag_bindings.c",
        "object_set": "src/mux/lua/packages/mux/world/mux_flag_power_bindings.c",
        "state": "src/mux/lua/packages/mux/world/mux_state_bindings.c",
    }
    for symbol, (family, contract) in metamethods.items():
        entries.append({
            "symbol": symbol, "kind": "metamethod", "package": "mux.userdata",
            "arguments": "", "returns": [], "reference": meta_sources[family],
            "contract": contract, "status": "actionable", "prerequisite": "",
            "rust_evidence": "native registration inventoried; exact behavior audit pending",
            "acceptance": "direct field access and metamethod invocation match the registered C function, including return arity and stale/error behavior",
            "accepted_difference_ids": [],
        })
    for code in ERROR_CODES:
        root = code.split(".", 1)[0]
        entries.append({
            "symbol": code, "kind": "error", "package": f"{root}.error-catalog",
            "arguments": "", "returns": ["table"], "reference": "src/mux/lua/lua_error.c:318-366",
            "contract": "Stable dotted code node exposed as a plain table: .code and child fields are ordinary existing fields; assigning a new field is rejected by __newindex.",
            "status": "actionable", "prerequisite": "", "rust_evidence": "src/lua/packages/error/catalog.rs; exact plain-table and mutation audit pending",
            "acceptance": "lookup and tostring return this exact dotted code; type is table; existing fields are writable while missing-field assignment is rejected",
            "accepted_difference_ids": [],
        })
    modules = {
        "access_policy.evaluate": "ctx, options", "object_appearances.render_contents": "ctx",
        "object_appearances.render_exits": "ctx", "object_appearances.render_internal_appearance": "ctx",
        "testing.test": "name, callback", "testing.suite": "name, definition",
        "testing.suite-result.expect.equal": "actual, expected", "testing.suite-result.expect.not_equal": "actual, expected",
        "testing.suite-result.expect.truthy": "actual", "testing.suite-result.expect.falsy": "actual",
        "testing.suite-result.expect.is_nil": "actual", "testing.suite-result.expect.contains": "actual, expected",
        "testing.suite-result.expect.near": "actual, expected, tolerance", "testing.suite-result.expect.error_matches": "callback, pattern",
        "testing.suite-result.expect.raises": "callback", "testing.suite-result.expect.raises_code": "callback, code",
        "testing.suite-result.expect.no_error": "callback", "testing.suite-result.expect.is_error": "value, code",
    }
    for symbol, arguments in modules.items():
        module = symbol.split(".", 1)[0]
        entries.append({
            "symbol": symbol, "kind": "receiver-callable" if ".suite-result." in symbol else "module-callable", "package": module,
            "arguments": arguments, "returns": [], "reference": f"game/lua/packages/{module}.lua",
            "contract": "Shipped Lua module contract; source file at the pinned revision is the behavioral oracle.",
            "status": "actionable", "prerequisite": "", "rust_evidence": "Rust ships a byte-identical module source; isolated-module loader and host integration audit remains pending",
            "acceptance": "require resolves the module and export; focused module probes preserve return/error behavior",
            "accepted_difference_ids": [], **({"receiver_factory": "testing.suite", "receiver_path": "expect"} if ".suite-result." in symbol else {}),
        })
    packages = ["mux", "mux.comsys", "mux.config", "mux.error", "mux.error.codes", "mux.session", "mux.telnet", "mux.text", "mux.world",
                "btech", "btech.autopilot", "btech.character", "btech.map", "btech.parts", "btech.player",
                "btech.repair", "btech.system", "btech.template", "btech.unit", "btech.error", "btech.error.codes", "access_policy", "object_appearances", "testing"]
    packages = sorted(set(packages) | set(CONSTANT_CLASSES.values()))
    for symbol in packages:
        entries.append({
            "symbol": symbol, "kind": "package", "package": symbol.rsplit(".", 1)[0] if "." in symbol else symbol,
            "arguments": "", "returns": ["table"],
            "reference": "src/mux/lua/packages/btech/btech_package.c" if symbol.startswith("btech") else "src/mux/lua/packages/mux/mux_package.c",
            "contract": "Canonical require/package namespace identity; child namespaces are stable tables and typed catalogs are immutable.",
            "status": "actionable" if symbol.startswith("btech.") or symbol in CONSTANT_CLASSES.values() or symbol.endswith(".error.codes") else "verified", "prerequisite": "",
            "rust_evidence": "canonical namespace installation",
            "acceptance": "the exact package path resolves and require of a root/shipped package returns the canonical table identity",
            "accepted_difference_ids": [],
        })
    allowed = ["assert", "error", "gcinfo", "getmetatable", "ipairs", "newproxy", "next", "pairs", "pcall", "print", "rawequal", "rawget", "rawset", "require", "select", "setmetatable", "tonumber", "tostring", "type", "unpack", "xpcall", "math", "string", "table", "bit", "_G", "_VERSION"]
    blocked = ["io", "os", "debug", "package", "coroutine", "jit", "ffi", "dofile", "loadfile", "loadstring", "load", "collectgarbage", "module", "getfenv", "setfenv"]
    for name in allowed + blocked:
        expected = name in allowed
        rust_matches = name not in {"package", "coroutine", "loadstring", "load", "collectgarbage", "module", "getfenv", "setfenv"}
        entries.append({
            "symbol": name, "kind": "sandbox-global", "package": "sandbox", "arguments": "", "returns": [],
            "reference": "src/mux/lua/lua_runtime.c:272", "contract": f"global is {'available' if expected else 'absent'} after sandbox installation",
            "status": "verified" if rust_matches else "actionable", "prerequisite": "",
            "rust_evidence": "src/lua/sandbox.rs and src/lua/sandbox.lua",
            "acceptance": f"type(_G[{name!r}]) is {'not nil' if expected else 'nil'} in an initialized callback VM",
            "expected_presence": expected, "accepted_difference_ids": [],
        })
    libraries = {
        "math": "abs acos asin atan atan2 ceil cos cosh deg exp floor fmod frexp huge ldexp log log10 max min modf pi pow rad random randomseed sin sinh sqrt tan tanh",
        "string": "byte char dump find format gmatch gsub len lower match rep reverse sub upper",
        "table": "concat foreach foreachi getn insert maxn move remove sort",
        "bit": "arshift band bnot bor bswap bxor lshift rol ror rshift tobit tohex",
    }
    for library, names in libraries.items():
        for name in names.split():
            constant = (library, name) in {("math", "huge"), ("math", "pi")}
            entries.append({
                "symbol": f"{library}.{name}", "kind": "sandbox-constant" if constant else "sandbox-function",
                "package": f"sandbox.{library}", "arguments": "", "returns": [],
                "reference": "src/mux/lua/lua_runtime.c:272 and LuaJIT 2.1 standard libraries",
                "contract": "LuaJIT 2.1 standard-library member retained by the native sandbox.",
                "status": "verified", "prerequisite": "", "rust_evidence": "mlua LuaJIT runtime",
                "acceptance": f"{library}.{name} has type {'number' if constant else 'function'} after sandbox installation",
                "expected_type": "number" if constant else "function", "accepted_difference_ids": [],
            })
    return entries


def main() -> None:
    check = "--check" in sys.argv[1:]
    if any(argument != "--check" for argument in sys.argv[1:]):
        raise SystemExit("usage: update_lua_contract_inventory.py [--check]")
    mux = callable_entries("game/lua/types/mux.d.lua", "mux")
    btech = callable_entries("game/lua/types/btech.d.lua", "btech")
    # Keep the original callable-only fixture compatible with its live MUX
    # resolver. The full cross-package inventory lives in the companion file.
    callables = sorted(mux + btech, key=lambda row: row["symbol"])
    legacy_text = json.dumps(sorted(mux, key=lambda row: row["symbol"]), indent=2) + "\n"
    all_entries = callables + constants(git_show("game/lua/types/mux.d.lua"))
    all_entries += constants(git_show("game/lua/types/btech.d.lua")) + supplemental_entries()
    overrides_path = ROOT / "tests/fixtures/lua-api-contract-overrides.json"
    overrides = json.loads(overrides_path.read_text())["overrides"]
    for entry in all_entries:
        if entry["symbol"] in overrides:
            entry.update(overrides[entry["symbol"]])
    registered_btech = registered_btech_symbols()
    registered_mux = registered_mux_symbols(mux)
    registered = sorted(registered_btech + registered_mux)
    callable_symbols = {entry["symbol"] for entry in callables}
    missing = sorted(set(registered) - callable_symbols)
    if missing:
        raise ValueError(f"registered native callables missing from declarations/inventory: {missing}")
    payload = {
        "schema_version": 1, "reference_revision": C_REVISION,
        "rust_baseline_revision": RUST_BASELINE,
        "constant_namespace_classes": CONSTANT_CLASSES,
        "registered_public_symbols": registered,
        "registered_public_symbols_by_family": {
            "btech_native_arrays": registered_btech,
            "mux_native_installers": registered_mux,
        },
        "accepted_differences": [
            {"id": "AD-TRANSACTION-001", "contract": "Nested callback validation and failures roll back world, output, flow, maintenance, and staged log effects transaction-wide.", "evidence": "docs/audit-lua-edges.md LE06 and LE08"},
            {"id": "AD-MOVEMENT-CAUSE-001", "contract": "Rust retains the original command cause for exit/enter/leave movement actions.", "evidence": "docs/lua-api-parity.md:78"},
            {"id": "AD-FLOW-BOUNDS-001", "contract": "Flow scratch and malformed outcomes reject atomically within configured bounds instead of truncating or retaining callback effects.", "evidence": "docs/audit-lua-edges.md LE09"},
            {"id": "AD-SCHEDULE-CAPTURE-001", "contract": "Schedules capture validated handlers/incarnations, do not backfill missed minutes, and queued jobs clear on successful reload.", "evidence": "docs/audit-lua-edges.md LE10 and docs/lua-schedules.md"},
            {"id": "AD-RESOURCE-LIMITS-001", "contract": "Configured instruction, memory, state, output, and bounded-report limits apply.", "evidence": "docs/audit-lua-edges.md baseline and docs/lua-api-parity.md"},
            {"id": "AD-CONNECTED-OWNERSHIP-001", "contract": "CONNECTED remains session-owned and is not writable through object flags or detached snapshots.", "evidence": "docs/lua-api-parity.md Intentional Rust behavior"},
            {"id": "AD-TEXT-EXTENSIONS-001", "contract": "Markdown documents, defaulted TOML lookup, Unicode width, and bracket markup remain supported Rust extensions.", "evidence": "docs/lua-api-parity.md Intentional Rust behavior"},
            {"id": "AD-HOST-ISOLATION-001", "contract": "Native services use shared domain logic and async outer persistence; filesystem/debug/native loading remains unavailable.", "evidence": "docs/lua-api-parity.md Intentional Rust behavior"},
        ],
        "entries": sorted(all_entries, key=lambda row: (row["kind"], row["symbol"])),
    }
    contracts_text = json.dumps(payload, indent=2) + "\n"
    outputs = {
        ROOT / "tests/fixtures/lua-api.json": legacy_text,
        ROOT / "tests/fixtures/lua-api-contracts.json": contracts_text,
    }
    if check:
        stale = [str(path.relative_to(ROOT)) for path, text in outputs.items() if path.read_text() != text]
        if stale:
            raise SystemExit("stale generated Lua inventory: " + ", ".join(stale))
    else:
        for path, text in outputs.items():
            path.write_text(text)


if __name__ == "__main__":
    main()
