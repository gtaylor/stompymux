# Architecture and source ownership

StompyMUX runs game operations on one serialized world owner. Tokio socket tasks
exchange bounded messages with that owner; Lua values remain on its thread.
Persistence may await SQLite while networking continues, but another game operation
does not interleave with the pending commit.

## Transaction contract

A game operation can change the world, stage notifications, advance an interactive
flow, request maintenance, or stage a logfile append. These effects belong to the
operation, including when a native command invokes Lua. Validate and persist before
publishing success output or applying external effects.

Nested callbacks have savepoints. A failed nested callback restores its candidate
world and effects even when Lua catches the error. An outer persistence failure
restores the world and discards pending output. Session membership is authoritative
runtime state: rollback must not revive a disconnected session or restore stale
CONNECTED flags. Object and channel incarnations must not be reused by rollback.

Interactive flow script errors cancel the flow; persistence errors retain its last
committed step for retry. Hosted Lua tests intentionally retain valid mutations even
when an assertion fails; invalid or unsaved mutations still roll back. These are
explicit execution policies, not interchangeable error paths.

Output directed to an object is rendered for each receiving session. Private flow
prompts retain their ordering relative to ordinary notifications. Queue admissions,
cancellations, and maintenance session detachments take effect only after commit.

## Dependency direction

- World and account models describe game data and its invariants.
- Native services implement game operations using those models and narrow callback
  contracts. Command parsing and Lua argument conversion adapt inputs to services.
- Runtime transaction state owns staged effects; Lua bindings participate in that
  state and do not own general logging or persistence responsibilities.
- The server coordinates admission, execution, persistence, delivery, and shutdown.
- Persistence owns SQLite representations and preservation of unsupported columns.
- Text constructs and renders documents; Telnet encodes transport bytes and manages
  protocol negotiation. Wire-byte budgets belong at the output boundary.

The crate remains a single package. Extract another crate only when its dependency
boundary is already clear and there is a concrete benefit to separate compilation
or reuse.

## Source map

| Area | Home | Responsibility |
| --- | --- | --- |
| Process entry | `main.rs` | CLI and process signals |
| Server lifecycle | `server/` | Startup, event loop, connections, authentication, shutdown |
| Command attempts | `server/execution.rs` | Shared interactive/background orchestration and reply routing |
| Commit publication | `server/transaction.rs` | Persistence, rollback, post-commit delivery and detachments |
| Candidate effects | `runtime/transaction.rs` | VM-independent checkpoints, flow data, output, logs, maintenance |
| World | `world/` | Objects, allocation, containment, validation |
| Accounts | `accounts/` | Account records, credentials, player-name policy |
| Communication | `communication/` | Channel data, policy, membership, paging, speech |
| Script adapter | `lua/` | Sandboxing, module loading, callbacks, flow-step execution |
| Commands | `commands/` | Registration, parsing, source selection, command results |
| Storage | `persistence/` | SQLite loading, selective writes, maintenance SQL |
| Shared tests | `tests/support/` | Fixture isolation, command text collection, TCP clients and startup |

The root library exports core model, command, configuration, and embedding types.
Use those exports from downstream code. Subsystem namespaces remain useful for
operations with common names; private implementation modules are not an API.

## Adding behavior

Put durable data beside its domain model, validation beside the invariant it
protects, and protocol conversion at the appropriate adapter. Native commands and
trusted Lua setters may have different authorization and hook semantics; share the
underlying invariant without silently merging those policies.

Keep command registration metadata authoritative for dispatch and discovery. Decide
whether a new operation supports background execution and how its reply is routed.
Background execution never borrows a player's session or its authority.

Use wall-clock time for persisted timestamps and calendar schedules. Use monotonic
deadlines for elapsed-time policies such as throttling and idle checks.

## Verification

Unit tests live beside implementations. Integration tests use isolated copies of
`tests/fixtures/game`; scenario-specific world edits and failure injection remain
visible in the scenario. Parity tests protect user-visible ordering, output,
authorization, and rollback semantics during structural changes.

Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`
after integration. Keep mechanical moves separate from changes to behavior.
