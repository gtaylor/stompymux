# Six-area non-BattleTech behavioral audit

This audit follows the communication and lock/message comparisons. It compares
observable behavior, rather than treating a registered command or Lua symbol as
proof of parity. Remediation is performed by Sol Medium agents and reviewed in
the combined working tree.

## Baseline and boundaries

- C: `2bbbe6fcdbabe69e229d73089f44bf38f91c0591`.
- Rust: `74246d92d570fb3e3b8778dc0e28071e9cca2548`, plus this working tree.
- Temporary servers use copies of `tests/fixtures/game` and public fixture
  credentials. No production database or server is used.
- C source tracing and executed comparisons are identified separately in each
  area report. Rust regressions alone do not establish executed C equivalence.

The existing approved differences remain: transactional persistence and staged
effects, original movement causes, descriptor ownership, session-owned CONNECTED,
monotonic object identities, resource/queue bounds, unsafe-dependency refusal,
new-member containment append order, password redaction, staged Lua logging,
restricted file access, session IDs and transport counter definitions, Markdown
extensions and Unicode layout, successful same-location movement no-ops, relative
help article-path lookup, session-private help and administrative reports where
specified, and retaining the active help index after a fatal reload traversal
failure. Removed command forms and switches remain absent.
BattleTech behavior is excluded.

A Rust test or README statement does not by itself authorize another difference.
In particular, the earlier P01 dispatch question requires comparison with the C
entrypoint; the earlier unsuccessful P02 zone-exit probe is not evidence that the
C feature is dormant.

## Coverage and evidence

| Area | Compared behavior | Detailed evidence |
|---|---|---|
| Dispatch, matching and queues | Native/Lua precedence, scoped accumulation, aliases/macros, zone fallback, exit ties, queued executor/cause and permissions | [Dispatch audit](audit-dispatch.md) |
| Object lifecycle and repair | Defaults, cloning, linking, GOING evacuation, callback state, tombstones, dependent cleanup and persistence | [Lifecycle audit](audit-lifecycle.md) |
| Lua edges | Missing/nil/false values, numeric identities, visibility, errors, stale handles, nested calls, flows, schedules and reload | [Lua audit](audit-lua-edges.md) |
| Accounts and live configuration | Registration/login boundaries, validation, history, reconnects, live policy consumers and durability | [Accounts audit](audit-accounts.md) |
| Telnet and styled rendering | Negotiation, terminal identity, color/OSC capabilities, option state and transport boundaries | [Terminal audit](audit-terminal.md) |
| Reports, logging and help | Report visibility/values, auditing, logging settings, front matter, lookup and refresh | [Operations audit](audit-operations.md) |

The reports intentionally limit claims to inspected and tested paths. They do
not certify every possible malformed database graph, Lua value combination or
terminal capability cross-product.

## Corrections

- **Dispatch (D13–D17):** restored native/Lua precedence, local attachment
  accumulation and per-stage filtering, interactive-only dot macros, lifecycle
  preflight, and the working C zone-exit fallback. Explicit `goto` ambiguity and
  bare-exit tie selection retain their distinct C contracts.
- **Lifecycle (LC01–LC03):** corrected clone confirmation text/order and full
  GOING-container evacuation against live source state. Player/thing departures
  now run their actions before tombstoning, callback relocations are retained,
  and scheduled player destruction captures the correct runtime cause.
- **Lua (LE01–LE03, LE05, LE12):** corrected false-versus-nil options, state error
  codes, numeric identity coercion, and API-specific contents visibility.
- **Accounts (AC02–AC07):** corrected capacity boundaries, failure notices and
  reset, per-outcome history ordering, registration prompts/validation timing,
  throttle closure, and direct login appearance without command dispatch.
- **Presentation (TR01–TR02):** corrected MTTS parsing, raw bounded terminal
  metadata, response counting, and peer-protocol diagnostic categories.
- **Operations (OP01–OP06):** corrected idle/maximum reports, pre-audit rejection,
  queued macro classification, retention of nonsecret entered command text,
  and permissive C help metadata parsing. Approved help lookup/reload behavior
  remains intact.

The [machine-readable matrix](../tests/fixtures/behavioral-parity.json) records
these narrowly scoped contracts alongside the earlier findings. The prior P01
and P02 questions are resolved for the paths reproduced here. No confirmed
uncorrected defect remains in the audited paths; that is not a claim of complete
MUX equivalence.

## Remaining verification limits

- Corrupt containment/list ownership and dependency combinations have not been
  differentially executed across both servers. Safe repair fallback does not
  prove the full C movement sequence on every malformed graph.
- Lua flows, reloads, schedules and malformed values rely primarily on source
  traces and Rust regressions rather than exhaustive paired execution.
- The terminal probe covers fifteen TTYPE metadata cases; other options and
  rendering combinations rely on source traces and existing protocol fixtures.
- Live configuration consumers and operating-system logging/timing failures
  have not each been exercised in paired runs.
- History preserves C's per-outcome positions. The schema cannot recover exact
  cross-outcome insertion order after equal or backward timestamps.
- Destroyed-player detach remains staged until commit. Its CONNECTED-dependent
  callback context can differ from C's earlier socket detach, as approved.

## Verification

The 32 selected C tests passed, covering command/configuration registries,
configuration parsing, builder names, network formatting, communication macros,
speech, flags/powers, password/validation, UTF-8 and styled text, object state and
names, accounts/cache, paging, Lua modules/access/locks/logs/errors/protected calls,
libtelnet/environment, and help parsing/rendering/indexing.

Final combined working-tree verification passed:

- `cargo fmt --all --check`.
- `cargo clippy --all-targets -- -D warnings`.
- `cargo test --no-fail-fast`: 346 tests passed, no failures.
- `git diff --check`; audit links, JSON syntax, unique matrix IDs and new source/
  test references checked.
- Paired fixture consistency: all fifteen terminal metadata cases and the distinct
  zone-exit sender/observer outputs agree. Admission transcripts preserve the
  selected matching prompts, counts, history notice and capacity outcomes.

No production files, schema, running server or C sources changed. No commit was
created. The test updates preserve rollback coverage while replacing old
expectations for native precedence, local source accumulation, GOING preflight,
clone wording and repair callback context.
