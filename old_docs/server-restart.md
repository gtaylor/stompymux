# Server restarts

The configured `game_database` is the authoritative game snapshot. In the
standard configuration this is `game.run/data/netmux.db.sqlite`.

## Normal startup and shutdown

Start the server normally with `game.run/Startmux` or `netmux <config-file>`.
Normal starts do not restore a previous process's live connections or command
queue; any stale restart-continuation rows are discarded. `SIGTERM` and
`@shutdown` stop the server normally and do not create restart continuation
state. `SIGUSR2` performs the controlled `.KILLED` shutdown, writing
`netmux.db.sqlite.KILLED` for operator recovery.

Old `restart.db` and `restart.xdr` files are no longer read or written. They
are not migration inputs and may be removed manually once the new server is in
use.

## Controlled restart

Use `@restart` from the game or send `SIGUSR1` to the server process. The
server writes a complete `DUMP_RESTART` SQLite snapshot, then stores its live
descriptor/session fields and command queues in transient `restart_*` tables
inside the same configured database. It then re-execs itself with the internal
`--restart <config-file>` argument.

On that internal start, MUX loads the game snapshot and special-object state,
then validates and rebuilds descriptors, socket events, connected flags, wait
queues, semaphore queues, and object queues. After successful restoration it
deletes the transient rows in one transaction and announces that restart has
finished. Do not invoke `--restart` manually; it is valid only when inherited
file descriptors and a continuation payload exist.

If the restart snapshot or continuation transaction cannot be written, the
server does not re-exec. If a `--restart` payload is missing or invalid, the
resumed process exits rather than restoring partial state. Review the server
log and start normally after resolving the SQLite problem.

## Fatal recovery

With `signal_action default`, `SIGSEGV` and `SIGBUS` retain the existing
recovery behavior: the failing process writes a diagnostic
`netmux.db.sqlite.CRASH` snapshot, while its child saves restart continuation
state to the normal game database and re-execs with `--restart`. The restart
therefore resumes from the last reliable normal database snapshot while
preserving live inherited descriptors when possible. With `signal_action exit`,
MUX logs the fault and exits instead.

The `.CRASH` and `.KILLED` files are diagnostic/operator snapshots; normal
startup continues to read the configured `game_database` path.
