# Telnet and rendering audit

Audited C baseline `2bbbe6fc` against Rust baseline `74246d9` plus the
correction below. The scope covers TTYPE/MTTS metadata, NAWS, NEW-ENVIRON,
CHARSET, MSSP, GMCP, MCCP2, effective color and screen-reader selection,
OSC capability projection, preset fallback, styled-text rendering, protocol
diagnostics, and output limits.

No production game, database, port, or process was used. The paired probe
copied `tests/fixtures/game`, replaced only the fixture password hash and
loopback port, launched each compiled server, and queried `@telnet` through
separate fixture accounts. Before evidence is retained in
`tests/fixtures/terminal-audit-before.json`; after evidence was written to
`tests/fixtures/terminal-audit-after.json`. Rust unit and integration tests use in-memory
decoders, duplex streams, temporary game trees, and loopback sockets.

## Negotiation and bounded state

The C state machine is in
`btmux-khi/src/mux/network/telnet_handler.c:50-475`, backed by bundled
libtelnet's RFC 1143 negotiation. Rust uses `src/telnet.rs`, `src/telnet/q.rs`,
`src/telnet/environment.rs`, and `src/telnet/transport.rs`.

Both advertise remote TTYPE, NAWS, and NEW-ENVIRON plus local MSSP, MCCP2,
CHARSET, and GMCP in the same order and directions. Confirmed transitions send
the same TTYPE SEND, empty NEW-ENVIRON SEND, UTF-8 CHARSET REQUEST, MSSP status,
and MCCP2 start effects. Refusal resets terminal/NAWS/environment or pending
CHARSET state as applicable; re-enablement triggers the effect again. GMCP
answers only `Core.Ping` with an exact package boundary. MSSP derives name,
connected-player count, process start time, codebase, and listening port from
the live server snapshot.

The explicit approved protocol boundary remains: Rust applies subnegotiation
effects only after the corresponding Q state is enabled, even where C accepts
an unsolicited payload. Rust also retains bounded malformed-input handling,
does not accept inbound compression, and never places invalid UTF-8 in the game
command stream.

NEW-ENVIRON matches C's atomic IS replacement and INFO patch semantics,
separate VAR/USERVAR namespaces, byte escaping, deletion without VALUE, empty
values, and 64-entry, 256-byte name, 4096-byte value, and 65536-byte aggregate
limits (`btmux-khi/src/mux/network/telnet_environment.c`). Invalid updates keep
the previous map; oversized subnegotiations drain through IAC SE so following
text remains framed. WONT clears the stored environment.

CHARSET collision and refusal behavior is covered: the server requests only
UTF-8, rejects a peer request while its own request is pending, accepts UTF-8
case-insensitively from a peer request, records a peer's unsupported ACCEPTED
value, emits C's TELNET/CHARSET diagnostic under the Problems category, and
clears pending state on REJECTED or DONT. NEW-ENVIRON parse/limit diagnostics
likewise use Problems and TELNET/ENVIRON rather than the unrelated Network
switch. Rust's
input policy remains printable valid UTF-8 regardless of the peer claim.

MCCP2 writes its marker before one persistent zlib stream, does not restart an
active stream, sync-flushes prompts, accounts compressed wire bytes, and
finishes on connection shutdown. A DONT received after compression starts is
ordinary data inside that persistent stream, matching the one-way nature of C's
active compressor. Queue saturation or socket failure cannot falsely mark a
failed start as active.

## TTYPE and MTTS metadata

C parses metadata through `telnet_handle_terminal_type` in
`telnet_handler.c:149-157` and `terminal_mtts_parse` /
`terminal_color_depth_from_type` in
`src/mux/support/styled_text/renderer.c:132-174`. Its MTTS number is a
nonnegative `strtol` C string, not an unsigned Rust token.

`TR01` corrected that parser and storage boundary. Rust previously decoded the
whole payload lossily to `String` and used `u32::parse`, so it rejected forms C
accepts and exposed different terminal metadata. Rust now:

- stops at the first NUL;
- accepts an empty suffix as zero, C ASCII whitespace (`0x09` through `0x0D`
  plus space), `+`, `-0`, and
  values through signed 64-bit maximum;
- rejects whitespace-only suffixes, trailing bytes, negative nonzero values,
  and signed overflow;
- stores the first 63 raw bytes for client/terminal display, matching C's
  64-byte NUL-terminated fields;
- derives DUMB/XTERM/256COLOR/TRUECOLOR capabilities from the full original C
  string before display truncation; and
- preserves invalid terminal bytes for escaped diagnostics without placing
  them in UTF-8 output.

TTYPE discovery still sends at most three requests, while the response counter
continues to record every confirmed response. Rust now uses a wide saturating
counter rather than freezing observable diagnostics at 255.

The after probe exercises ordinary MTTS, two spaces, tab and vertical-tab whitespace, plus,
negative zero, empty and whitespace-only suffixes, values above `u32`, signed
maximum and overflow, trailing whitespace, embedded NUL, invalid UTF-8, and a
TRUECOLOR marker beyond byte 63. All fifteen cases now agree semantically:
client, terminal display, advertised color depth, and screen-reader state are
identical; Rust retains its approved diagnostic label
`Color depth (advertised)`.

## Effective rendering and diagnostics

C chooses rendering options in
`btmux-khi/src/mux/network/network_output.c:133-175`; Rust uses
`Session::render_options` and the modules under `src/text`. The object ANSI flag
is the outer gate. A connection color override wins next, including for a
screen-reader client; without an override, screen reader disables color before
the advertised depth is selected. Width zero is clamped to one for safe layout.

Only USERVAR values exactly equal to byte `1` enable the known
`OSC_HYPERLINKS[_CAPABILITY]` names. Unknown, invalid UTF-8, wrong-kind, and
wrong-value entries do not create capabilities. Presets emit once per
connection when advertised; when PRESETS or COMPACT is absent, equivalent
supported configuration is expanded or the base ANSI style remains. Existing
C-derived fixtures compare visible styled text and OSC payload semantics.

Private `@telnet` diagnostics retain the approved session identifier, Q-state,
and counter definitions. Every client-supplied byte is escaped, report size is
bounded, and truncation remains visible. Rust's safe Markdown extensions,
Unicode display widths, complete ANSI/OSC closing sequences, safe links, and
bounded terminal/HTML chunks are intentional rendering differences from C's
cmark and fixed-buffer output.

## Executed evidence

- Rebuilt `target/debug/stompymux-rs`, then ran
  `PYTHONDONTWRITEBYTECODE=1 python3 tests/tools/terminal_probe.py`; all fifteen
  paired TTYPE cases matched the C metadata semantics described above.
- `cargo test --test core_03 telnet::`: seven Q-state, echo, TTYPE/MTTS, CHARSET, and
  malformed-subnegotiation tests passed.
- `cargo test --test core_02 telnet_extensions::`: eight NEW-ENVIRON, GMCP, MCCP2,
  diagnostic, counter, duration, and slow-writer tests passed.
- `cargo test --test core_01 text::`: twelve C-derived styled-text, OSC, preset,
  Markdown, Unicode, HTML, and compression tests passed.
- Focused loopback server coverage for extended telnet/session diagnostics and
  negotiated rendering is retained in `tests/foundation.rs`; the root agent
  runs the combined suite.

## Remaining limits

The paired after probe negotiates TTYPE only; the other options are covered by
source tracing and isolated Rust protocol/transport tests rather than a new
dual-server transcript. Operating-system socket timing and every possible RFC
1143 crossed negotiation are not exhaustively enumerated. Confirmed Q gating,
private diagnostics/session IDs, resource limits, persistent one-way
compression, safe UTF-8 output, and Markdown/rendering extensions are explicit
approved differences and were preserved.
