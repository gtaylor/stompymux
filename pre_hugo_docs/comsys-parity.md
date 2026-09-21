# Comsys behavioral audit and corrections

Comparison baseline: C `2bbbe6fcdbabe69e229d73089f44bf38f91c0591` and Rust
`3a1c55947f00fee05a69c7df5398e0c1e0398664` plus the pending parity corrections.
This audit preserves the earlier lock/message and movement work. It covers the
channel service, native channel commands, channel aliases, presence, history and
`mux.comsys`; paging retains the preceding lock/message audit's coverage.

## Executed evidence

The optional [probe](../tests/tools/comsys_probe.py) starts each binary against
its own temporary copy of `tests/fixtures/game`, uses a public fixture password,
and captures both players' output. It declines optional Telnet negotiation and
never opens the operator game directory.

The [before transcript](../tests/fixtures/comsys-wire-before.json) records the
original 43-command comparison. The [corrected transcript](../tests/fixtures/comsys-wire.json)
extends it to **74 command invocations**. All 74 produce identical text for both
players, including recipient selection, message order, errors and row padding.
No text normalization beyond the shared Telnet client's CRLF decoding is needed.
The C side of this transcript is replayed by `tests/comsys_parity.rs`, without a
C binary dependency in normal Cargo testing.

## Corrected behaviors

C references below are relative to `btmux-khi/src/mux`; Rust references are
relative to `stompymux-rs/src`.

| Behavior | C reference | Rust correction and evidence |
|---|---|---|
| Alias removal | `communication/channel_aliases.c`: `do_delcom`, `comsys_delete_channel_alias` | `communication/membership.rs`: deleting any alias removes membership, retaining other aliases. Departing members receive the direct notice, not their own leave broadcast. Paired TCP and restart tests. |
| Boot and destruction | `channel_administration.c`: `do_chboot`; `channel_management.c`: `comsys_channel_destroy`; `mux_comsys_bindings.c`: `boot_player` | Native/Lua boots retain aliases; destruction removes channel state but retains player-owned alias rows. Existing macro/unknown-dependency persistence tests retained. |
| Clear and bulk operations | `communication/channel_presence.c`: `comsys_clear_player`, `do_allcom` | Clear processes aliases in reverse order without invented alias confirmations. `allcom` processes each slot, including duplicates, and suppresses the empty `who` notification as C `raw_notify` does. TCP plus off-member callback regression. |
| Add validation and confirmations | `communication/channel_aliases.c`: `comsys_add_alias`, `comsys_channel_add_player` | Native validation order, duplicate-channel warning and alias-specific refusal restored. Lua receives the same final join confirmation, including quiet joins. Quiet suppresses the broadcast only. Paired TCP. |
| Presence | `communication/channel_presence.c`: `do_comconnect`, `do_comdisconnect` | Alias-ordered LOUD announcements, including duplicate aliases, and stale-alias notices on connect. Restart and presence regressions. First/final-session lifecycle remains unchanged. |
| Online recipients and lock timing | `communication/channel_delivery.c`: `comsys_channel_printf`; `commac_persistence_sqlite.c`: membership loading | Online insertion order is runtime-only and independent of persisted membership slots. Counter increments precede RECEIVE locks; locks run before IC filtering. Source-grounded Lua context/order regression. |
| History routing | `communication/channel_delivery.c`: `do_show_com` | Replay uses the notification router, including AUDIBLE exits, while live player channel traffic stays direct. Dedicated routing/contents-exclusion regression. |
| Native administration | `communication/channel_administration.c`, `channel_presence.c`, `channel_management.c` | Switch help, operation-specific errors, empty emits, ignored unused arguments, word-prefix object matching and clearing unmatched channel objects now follow C. `who` preserves slot order across object types and C padding; status columns clip. Paired TCP and mixed-member test. |
| Presentation | `communication/channel_delivery.c`: `do_cemit`; `channel_management.c`: `comlist_description` | Emit headers retain the invoked channel spelling. `comlist` retains description styling with safe Unicode truncation. Focused Rust tests backed by C source. |
| Lua handles and failure isolation | `lua/packages/mux/comsys/mux_comsys_bindings.c`, `mux_comsys_channel_flag_bindings.c` | `tostring` returns `channel(name)` / `channel_flags(name)`; stale channel text remains printable while stale flag handles reject use. Add/emit/boot have nested rollback protection, including caught errors. Lua API and bounded-output tests. |

Leave callbacks retain the current C reverse-slot loop excluding slot zero.
Removal invokes callbacks even for off/stale membership aliases. Consequently,
a second removal after the first can see a different slot-zero object; the tests
cover this rather than assuming every thing always receives a callback.

## Retained behavior and audit limits

The existing tests still cover independent JOIN/TRANSMIT/RECEIVE grants, Wizard
bypass, lock-error fallback to access bits, DARK and IC visibility, channel flags,
poses, twenty-message newest-first history, immutable Lua constants, stale
handles, page privacy and selective relational writes. TRANSPARENT does not reveal
DARK players in this C fork: `is_hidden` itself tests DARK.

The approved differences remain: transactional persistence and staged output,
rollback of failed callbacks, bounded resources and notification propagation,
Unicode-safe rendering, and session-owned CONNECTED with first/final-session
announcements. Invalid references are rejected rather than reproducing unsafe C
results from ambiguous object matches. No schema, protocol or configuration
changes are part of this work.

Native channel lists use the same case-insensitive name order: despite its name,
C `support/hash_table.c` wraps a red-black tree and iterates FIRST/GT. Lua channel
lists explicitly sort using the same comparison.

This is not a claim of exhaustive comsys parity. The audit does not reproduce allocator failures, invalid C pointer
behavior or every callback that changes membership while an outer traversal is
in progress. The existing resource and transaction rules govern those cases;
this report does not silently classify new observable differences as approved.

## Verification

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 tests/tools/comsys_probe.py
ctest --test-dir ../btmux-khi/build --output-on-failure \
  -R '^(commac|lua_module_smoke|lua_command_access|lua_lock_checks|speech_format|page_recipients|lua_pcall_checked|flag_privileges)$'
```

The probe is optional for normal development. Its checked-in transcript supplies
executable C evidence; the tests identified as source-grounded do not imply that
those exact Lua fixtures were executed by C. No production files, server restart
or commit are included.

Final validation: 333 Rust tests and eight C tests passed; formatting and Clippy passed.
