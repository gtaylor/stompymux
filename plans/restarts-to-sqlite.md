# Move restart continuation state to SQLite

## Summary

Replace the `restart.db` and `restart.xdr` sidecars with transient restart
continuation rows in the configured SQLite game database. A controlled re-exec
uses `--restart` to consume that state; ordinary starts discard it.

## Implementation

- Create restart metadata, descriptor, command-queue, environment, and
  register tables in every SQLite snapshot.
- Store live descriptor and queue state in one durable SQLite transaction only
  after a `DUMP_RESTART` snapshot succeeds.
- Restore and consume continuation state only for `netmux --restart <config>`.
  Validate the complete payload and rebuild descriptor and queue runtime state
  through normal helpers.
- Route `@restart`, `SIGUSR1`, and configured fatal-signal recovery through
  this path; retain `.CRASH` and `.KILLED` snapshots for diagnostics and
  controlled shutdowns.
- Remove legacy serializers, readers, the `mmdb` implementation, and
  `Startmux` cleanup for `restart.db`.

## Verification

- Assert restart tables exist and are empty in normal, crash, killed, and
  consumed-restart snapshots.
- Exercise a connected-client restart, queued command restoration, malformed
  payload rejection, and transactional failure preservation.
- Build the server and run the SQLite integration suite.
