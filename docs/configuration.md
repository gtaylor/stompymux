# Configuration reference

`Config::load` reads `stompymux.toml` from the game directory. The typed model in
`src/config/model.rs` is the authority for effective defaults. The compatibility
inventory in `tests/fixtures/config/legacy-catalog.json` records all 182 legacy
mappings, their directive names, registry types and compiled defaults. The
annotated game configuration intentionally overrides some defaults.

## Loading and validation

Includes are recursive and resolved relative to the file containing `include`.
Later includes override earlier includes, and the including file wins. Ordinary
maps merge, arrays replace, and an explicit `database.bootstrap.objects` map
replaces the entire inherited/default map. Defaults apply after merging. Cycles
and excessive nesting are errors.

Unknown keys warn and are skipped. Malformed known values report the source file
and configuration path. Types, numeric bounds, finite floats, RGB components,
site address/mask families and bootstrap structure are validated. Gameplay
semantics that require deferred systems are not evaluated. Integer-valued
BattleTech switches remain integers. Permissions accept either a whitespace
separated string or an array of strings. Site arrays retain order; aliases,
access tables, colors and OSC presets retain dynamic names.

BattleTech, extended rendering/OSC, logging and remaining legacy command-system
options are parsed and retained without enabling those features. A consolidated
capability diagnostic reports this. Site and access rules pass configuration
checks, but nonempty rules block `serve` before bootstrap, database writes or
listening. There is no live reload or configuration editing command.

Rust callers use typed fields. Lua `mux.config` lookups accept dotted TOML
names and legacy directive names, and see the same defaulted effective values,
including listener CLI overrides.

## Listener, storage and content

`--listen-address` and `--port` are optional overrides: CLI wins over TOML, which
wins over defaults. `server.listen_address` defaults to `127.0.0.1` and accepts
IPv4 or IPv6 literals. The compiled `server.port` default is 6250; the supplied
file selects 5555. Port zero requests an ephemeral port.

`database.game_database` defaults to `data/stompymux-rs.db` and must contain Rust
storage. `database.legacy_game_database` defaults to `data/stompymux.db` and is
used for read-only checks and detecting an existing world before bootstrap.
Import remains explicit; `import-legacy --source FILE` is authoritative. A legacy
database used as live storage is rejected with import instructions. Changing
these names never moves files or migrates data automatically.

Content and database paths are relative to the game directory, unless absolute.
This includes Lua modules, connection/quit files and bootstrap credentials.
Fresh bootstrap uses the configured object map; imported worlds never bootstrap.
The foundational GOD/Wizard IDs and required room references are validated.
Default object flags and Lua parents also come from configuration.

## Additional Rust settings

Counts and byte limits below must be positive. Durations specify their units in
the name. Existing keys remain authoritative for matching features: for example,
`mux.conn_timeout`, `mux.idle_timeout` and `mux.idle_interval` use seconds;
`mux.command_quota_interval` uses milliseconds. Password hashing costs and login
rate/burst settings remain under their legacy `security` names.

| Setting | Default | Meaning |
| --- | --- | --- |
| `runtime.max_connections` | 1024 | Open connections |
| `runtime.event_queue_capacity` | 256 | World events queued |
| `runtime.session_output_queue_capacity` | 128 | Messages queued per session |
| `runtime.input_line_limit` | 8192 | Bytes per input line |
| `runtime.telnet_subnegotiation_limit` | 8192 | Bytes per Telnet subnegotiation |
| `runtime.find_page_size` | 20 | Positive maximum rows per @find page |
| `runtime.output_message_limit` | 65536 | Bytes per output message |
| `runtime.write_timeout_ms` | 5000 | Socket write deadline |
| `runtime.shutdown_timeout_ms` | 5000 | Connection drain deadline |
| `runtime.maintenance_interval_ms` | 1000 | Connection/auth housekeeping cadence |
| `security.login_address_limit` | 4096 | Tracked source addresses |
| `security.login_address_retention_seconds` | 86400 | Address throttle retention |
| `security.login_hash_concurrency` | 5 | Concurrent password jobs |
| `security.login_history_limit` | 32 | Login records retained per account |
| `lua.instruction_limit` | 1000000 | Instructions per callback/module |
| `lua.output_entry_limit` | 1024 | Pending Lua output entries |
| `lua.output_byte_limit` | 1048576 | Pending Lua output bytes |
| `database.busy_timeout_ms` | 5000 | SQLite lock wait |
| `database.bootstrap.credentials_file` | `bootstrap-credentials.txt` | Restricted bootstrap credential file |

The other two additions are `server.listen_address` and
`database.legacy_game_database`, described above. Protocol bytes, schema limits,
cryptographic constants and socket read-buffer sizes remain implementation
constants.
