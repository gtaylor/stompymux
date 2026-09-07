# Broader behavioral comparison — round two

This audit found **seven functional/API discrepancies (D06–D12)**. **D06–D12 are now resolved.** The highest-priority finding was D09: a private page reaches an unnamed player contained inside the recipient. The audit itself changed no server behavior; the subsequent D09 correction uses direct player delivery. The pending D01–D05 movement corrections remain the baseline.

## Baseline and evidence

C: `2bbbe6fcdbabe69e229d73089f44bf38f91c0591` (clean). Rust: `3a1c55947f00fee05a69c7df5398e0c1e0398664` plus the pending movement changes. The [audit fixture](../tests/fixtures/behavioral-audit-round2.json) records pending paths and a SHA-256 of the complete Rust `src` tree, so the comparison does not pretend those corrections were committed.

The [matrix](../tests/fixtures/behavioral-parity.json) retains earlier IDs, adds specific findings and control cases, and subdivides the nine broad unverified areas. The [TCP transcript](../tests/fixtures/behavioral-wire.json) captures both rebuilt servers using separate temporary copies of the test world. The probe logs in GOD and Wizard by dbref with a public fixture password, executes identical commands, and records both recipients. Telnet negotiation is declined; only CRLF is normalized. ANSI bytes and ordering remain visible in JSON. No operator database is opened.

Each finding below has paired execution, source tracing and a Rust characterization test. Passing characterization tests preserve evidence of the current defect; they must become positive regressions when a fix is approved. Source review and existing tests in other areas are not represented as new differential execution.

## Exclusions

The narrowly scoped X01–X07 entries in the audit fixture trace exclusions to prior decisions: transactional side effects and rollback; original movement causes and safe descriptors; CONNECTED reconciliation and bounded queues; Markdown/Unicode rendering and session IDs; stable appended containment order; explicitly removed command forms; and deferred BattleTech/HTTP/protocol work. Password redaction, staged Lua logs and path confinement also remain deliberate. These do not excuse wrong recipients, looser type validation, or missing **plain** `goto`. C accepting but ignoring `@destroy/recursive` is not evidence of recursive destruction.

## Confirmed findings

| Priority | ID | Difference |
|---|---|---|
| Resolved | D09 · high | Private pages now exclude contained non-recipients. |
| Resolved | D12 · medium | Plain `goto` and configured aliases now share exit travel. |
| Resolved | D06 · medium | Ordinary matching now accepts later word prefixes. |
| Resolved | D07 / D08 · medium | Stored descriptions take precedence after providers; remote rooms use internal appearance. |
| 5 | D11 / D10 · low | Lua numeric coercion and partial-page group annotation differ. |

### D06 — Ordinary object word-prefix matching (resolved)

**Setup/input:** Create a carried thing named Red Sword; use look sword.

**Original evidence:** C selected and described Red Sword; Rust returned `I don't see that here.` The captured transcript is retained unchanged.

**Effect:** Valid abbreviations for later words fail. The shared matcher also serves inventory operations; the paired probe exercises look.

**Correction:** Ordinary matching now uses the existing C word-prefix helper on style-stripped names. Exact-match precedence, lock/type preferences and candidate scope remain intact. Regression coverage includes punctuation, case, multiword prefixes, ambiguity, visibility, possessive lookup and inventory callers; TCP coverage verifies abbreviation-based transfers and restart durability.

Evidence: [C source](../../btmux-khi/src/mux/world/match.c), [Rust source](../src/commands/objects/mod.rs), `ordinary_word_prefix_matches_c` in [characterizations](../tests/behavioral_audit.rs), and `look sword` in the paired transcript.

### D07 (resolved) — Fallback description content loses precedence over its provider (medium)

**Setup/input:** Give Red Sword stored description STORED DESCRIPTION and audit.lua messages.describe returning PROVIDER DESCRIPTION while setting audit.provider=true.

**C:** STORED DESCRIPTION; provider still executes and sets state. **Rust:** PROVIDER DESCRIPTION; provider executes and sets state.

**Effect:** Stored descriptions become invisible whenever a describe provider supplies direct text, even though C gives stored content priority.

**Recommended correction:** Model stored description/internal-description priority explicitly after provider evaluation. Do not skip provider mutations or on_describe.

Evidence: [C source](../../btmux-khi/src/mux/commands/look.c), [Rust source](../src/commands/objects/look.rs), `stored_description_overrides_provider_content` in [characterizations](../tests/behavioral_audit.rs), and `look Red Sword` in the paired transcript.

### D08 (resolved) — Transparent exit selects the remote room external renderer (medium)

**Setup/input:** Link a transparent exit to a room with distinct INTERNAL ROOM VIEW and EXTERNAL ROOM VIEW callbacks; look at the exit from its source.

**C:** INTERNAL ROOM VIEW. **Rust:** EXTERNAL ROOM VIEW.

**Effect:** Copied room modules commonly supply only internal_appearance; remote room views fall back or lose their intended rendering.

**Recommended correction:** Select internal appearance for rooms regardless of viewer location, retaining the approved container distinction and transaction staging.

Evidence: [C source](../../btmux-khi/src/mux/commands/look.c), [Rust source](../src/commands/objects/look.rs), `remote_room_uses_internal_appearance` in [characterizations](../tests/behavioral_audit.rs), and `look audit-window` in the paired transcript.

### D09 — Private pages reach contained non-recipients (resolved)

**Setup/input:** Connect GOD and Wizard, teleport Wizard inside GOD, then GOD pages GOD with PRIVATE-PAGE.

**C:** Only GOD receives the page and sender confirmation; Wizard receives no text. **Rust:** Wizard also receives GOD pages: PRIVATE-PAGE, despite last-page recipients containing only GOD.

**Effect:** A player inside another player receives private communications without being named as a recipient. This is recipient expansion, not a multiple-session difference.

**Recommended correction:** Fix page delivery first. Match the C direct-player notification policy; audit related channel/non-player delivery separately rather than globally disabling legitimate broadcasts.

Evidence: [C source](../../btmux-khi/src/mux/communication/page_commands.c), [Rust source](../src/communication/delivery.rs), `private_page_excludes_contained_bystander` in [characterizations](../tests/behavioral_audit.rs), and `page GOD=PRIVATE-PAGE` in the paired transcript.

### D10 — Mixed valid/unknown page recipients lose group annotation (low, resolved)

**Setup/input:** Connect Wizard and page Wizard MissingPlayer=hello.

**C:** Wizard receives To (Wizard), GOD pages you: hello. Sender reports the missing name. **Rust:** Wizard receives GOD pages: hello. Sender reports the same missing name.

**Effect:** The recipient cannot see the original multi-target context. Delivery and successful-recipient history are otherwise correct in this case.

**Recommended correction:** Preserve C target-token/group semantics separately from the resolved/delivered list. Review partial/offline and repeated recipients together.

Evidence: [C source](../../btmux-khi/src/mux/communication/page_commands.c), [Rust source](../src/communication/page.rs), `mixed_recipient_page_preserves_group_annotation` in [characterizations](../tests/behavioral_audit.rs), and `page Wizard MissingPlayer=hello` in the paired transcript.

### D11 — Printable-ASCII predicate silently coerces numeric input (low, resolved)

**Setup/input:** Evaluate pcall(mux.text.is_printable_ascii,123), plus ASCII and newline-containing string controls.

**C:** The number is rejected; strings are accepted and return true/false as appropriate. **Rust:** The number is accepted and returns true; string controls match C.

**Effect:** Callers that depend on argument validation receive a successful boolean instead of a type error. This is looser validation, not the approved stricter-input behavior.

**Recommended correction:** Require an actual Lua string for this predicate, preserving binary-safe checks and documenting its argument error.

Evidence: [C source](../../btmux-khi/src/mux/lua/packages/mux/text/mux_text_bindings.c), [Rust source](../src/lua/packages/text/mod.rs), `printable_ascii_requires_strings` in [characterizations](../tests/behavioral_audit.rs), and `audit-ascii` in the paired transcript.

### D12 — Plain goto and configured movement aliases do not work (medium)

**Setup/input:** Create a linked exit audit-window; execute goto audit-window. The bare exit name is a working control.

**C:** The caller traverses the exit and sees INTERNAL ROOM VIEW. **Rust:** Unknown-command response; location does not change. Rust tests also reproduce go and move aliases.

**Effect:** Existing help and aliases advertise an unavailable core movement command. Only goto/quiet was intentionally excluded, not plain goto.

**Recommended correction:** Register plain goto using the shared exit path and its access policy; retain rejection of /quiet and test native/alias/forced execution.

Evidence: [C source](../../btmux-khi/src/mux/world/movement_commands.c), [Rust source](../src/commands/registry.rs), `goto_and_aliases_dispatch_exit_travel` in [characterizations](../tests/behavioral_audit.rs), and `goto audit-window` in the paired transcript.

The D07 source chain continues into C `notify_action` in `commands/action_messages.c`: providers still execute, but nonempty stored content takes precedence. D08 comes from `look_custom_appearance`: room targets always select internal appearance. D09 traces `page_commands.c` through `notify_checked(MSG_ME_ALL | MSG_F_DOWN)`; this fork does not implement contents forwarding for the `MSG_INV_L` bit behind `MSG_F_DOWN`. Rust passes `down=true` from `communication/page.rs` into its recursive delivery routine. The observed private-page leak is therefore not inferred from flag names.

## Coverage and remaining questions

The following distinguishes what was reviewed from what still needs a focused comparison. Existing regression suites were retained and run; their presence alone does not close an entire area.

| Area | Evidence this round | Still unverified |
|---|---|---|
| Dispatch and scoped permissions | Traced exit/native/Lua entry order, command registration, shadowing and aliases. D12 reproduced. | README explicitly documents Rust shadowing/first-handler semantics; native-first C behavior is a policy question P01, not silently classified as a bug. |
| Matching and visibility | Exact/leading-prefix controls match on TCP; D06 later-word matching fails. Read C match_list and Rust ordinary/builder matchers separately. | Ambiguous lock-preference/type combinations and possessive matching remain only partially covered. |
| Object lifecycle and repair | Reviewed SAFE/GOING/Wizard/foundation guards, replacement-home selection and the supported ownership cleanup map against C destruction/home code. Existing malformed-list/purge regressions retained. | No new paired C/Rust malformed-database destruction run; do not promote full repair equivalence. Unknown-dependency failure and append ordering remain intentional. |
| Channels and pages | Traced join/listening/receive/IC gates and direct vs recursive delivery. Paired page controls, partial targets and containment establish D09/D10. | Non-player channel recipient forwarding and partial/offline/repeated page combinations need focused follow-up. |
| Macros, queues and fallback | Read macro expansion and common queued dispatch/commit paths; retained the recently executed nested-force regression. Checked zone-room fallback suspicion in both servers. | Zonegate was unknown in both probes (P02); C declaration alone is not proof of working zone exits. Multi-handler scope semantics depend on P01. |
| Lua coercions and callback results | Compared binary state scalar conversion, strict message fields, describe side effects and ASCII predicate validation. Paired evidence establishes D07/D11. | All API coercions, coroutine/reload cross-products and malformed-return combinations are not verified; Rust rollback/checking restrictions excluded. |
| Configuration and runtime settings | Reviewed log_options partial edits and alias/directive plumbing; rebuilt and ran C configuration registry/value/interpreter/TOML tests, retaining Rust live-edit/include tests. | No exhaustive paired live reconfiguration across existing sessions; dormant directives and Rust-only runtime knobs excluded. |
| Telnet and rendering | Compared startup option directions and declined negotiation on real TCP for both engines. Reviewed legacy rendering boundaries; D08 room-mode selection reproduced. Rebuilt C text/Telnet/environment tests. | Full ANSI/OSC/capability/environment cross-product remains unverified; raw SGR redundancy alone is not a rendering defect. |
| Logging producers and formats | Reviewed audit placement, unknown-command logging, category defaults, cache expiry and filename/write entrypoints. Rebuilt C log tests and retained Rust logging regressions. | Complete event/category combinations and loss/saturation behavior were not paired; redaction, staged logs and confinement excluded. |
| Accounts/admission | Both engines authenticate the same controlled schema-32 Argon2id fixture by dbref with unanswered/refused optional negotiation. C password/account tests rebuilt. | All retry/site/edit/multiple-session combinations rely on existing Rust tests and source review, not a new differential run. |
| Help | Reviewed ASCII keyword case folding and metadata/body separation; rebuilt C frontmatter/index/render tests and retained Rust browsing/reload tests. | Not every supplied article has a new paired rendering transcript; Markdown extensions and Unicode layout excluded. |
| Operational reports | Reviewed shared command access and report/session accounting boundaries, using existing report regressions and the C connection command source. | No new byte-for-byte WHO/session/log-report comparison; IDs, units, timestamps and bounded chunking remain excluded. |

### Policy and C preconditions

**P01 — dispatch precedence:** C dispatches global native commands before Lua and can accumulate matching local handlers. The existing Rust README documents Lua shadowing and first-handled behavior. This audit preserves that documented contract and asks for an explicit disposition; it neither silently changes it nor declares a new intentional exception. Source references: C `commands/command_dispatch.c`, Rust `commands/mod.rs`.

**P02 — zone exits:** source suggests C zone-room exit fallback, but the recorded `zonegate` setup returned unknown in both servers. A working C reproduction is required before treating zone fallback as a missing feature. The unsuccessful probe is retained as negative evidence.

Three additional presentation differences are recorded separately, pending disposition: `@open` includes the created dbref in Rust, `@state/examine` adds a trailing blank line, and unknown-command guidance differs. These are observed differences, not approved exceptions or additional functional port gaps.

## Reproduction and validation

Run from `stompymux-rs` after configuring the sibling C CMake build:

```sh
cmake --build ../btmux-khi/build --target stompymux -j 2
cargo build
python3 tests/tools/behavioral_probe.py > /tmp/behavioral-wire.json
cargo test --test behavioral_audit
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The optional probe accepts `--c-binary` and `--rust-binary`. Normal Cargo tests read the captured fixture and never require a C executable. File-size/input limits and the fixed small test world keep the probe bounded; it closes clients, terminates both temporary servers and removes temporary worlds on completion or error.

The current C server and 32 selected C test targets were rebuilt. All 32 passed: command/configuration catalogs and interpretation, commac/speech/flags/powers, password/accounts/cache/page recipients, validation/UTF-8/styled text/state/names, Lua modules/access/locks/logging/error handling, Telnet/environment, help front matter/rendering/indexing, and libuv TCP integration. Selection and exact test names are recorded in the audit fixture.

Validation passed: `cargo fmt --check`, Clippy with warnings denied, and the full Rust suite (302 passed, zero failed or ignored), including eight new audit tests. Results are recorded in the fixture. No claim of complete non-BattleTech parity is made. D06–D12 are resolved; P01/P02 remain separate until their policy/preconditions are resolved.

## D09 correction evidence

Pages now stage output directly for each authorized player; their contents do not
receive it. All authenticated sessions of the addressed player still receive it.
Channel forwarding and page formatting remain unchanged. The examples above and
the original transcript describe the pre-fix observation. The
[corrected paired TCP transcript](../tests/fixtures/behavioral-wire-d09-resolved.json)
shows neither C nor Rust delivering `PRIVATE-PAGE` to the contained Wizard.

Positive tests cover nested occupants, ordinary players, explicit contained
recipients, aliases, saved recipients, poses, multiple sessions and rollback on
output-limit or page-history persistence failure. D10–D11 were subsequently resolved as recorded below.

D09 validation: 304 Rust tests passed; formatting and Clippy with warnings denied
passed. The optional paired TCP probe used isolated temporary worlds and confirmed
identical private-page recipients in C and Rust. No production files were changed.

## D12 resolution

Plain `goto` is registered with the C location prerequisite. Configured aliases
and shorthand share local exit matching, lock preference, traversal and the
existing transactional movement sequence. Live `goto` restrictions also gate
bare exit dispatch. No switches or new target forms are introduced, and the
existing Rust Lua/native precedence is preserved.

The historical transcript remains unchanged. `behavioral-wire-d12-resolved.json`
records the subsequent paired run. Positive tests cover all supplied aliases,
missing/unlinked/ambiguous exits, live access, and forced nested movement over TCP
with copied exit locks, multiple sessions, failed writes and restart durability.

### D06 correction validation

The shared ordinary matcher now accepts later-word prefixes. Formatting, Clippy
with warnings denied, and the full Rust suite passed (311 tests). Added regressions
cover matching boundaries, scope and precedence, inventory and MATCH-lock behavior,
and real TCP transfers with restart durability. The original audit transcript and
baseline fingerprint remain historical evidence, not fingerprints of the corrected tree.

## D07/D08 correction evidence

Description actions evaluate providers, then read the selected live description.
Nonempty stored content wins; empty or absent content falls back to provider text
and the applicable native default. Provider mutations and neighbor messages remain
active, and `on_describe` runs once after message delivery is staged. Internal
descriptions retain the C `inside_describe` selection rule.

Rooms always select internal appearance, including views through transparent exits.
A missing renderer permits description fallback; a defined empty renderer suppresses
it. Container internal/external selection is unchanged. The destination fallback
now includes its description without recursively following another exit.

Positive regressions cover live changes/clears, output ordering, callback rollback,
empty renderers, copied room modules, container descriptions and TCP restart
durability. Historical transcripts remain unchanged; the
[corrected paired TCP transcript](../tests/fixtures/behavioral-wire-d07-d08-resolved.json)
records matching stored-description and remote-room output.

D07/D08 validation: 315 Rust tests passed, including TCP persistence-failure and
restart checks; formatting and Clippy with warnings denied passed.

## D10–D11 correction

D10 now preserves group recipient formatting for mixed valid/unknown targets,
including speech, quoted speech and poses. Whole-name lookup still takes priority.
Saved pages use their saved recipient list; sender confirmations, delivery checks,
private-player routing and successful-recipient history are unchanged. C's unrelated
multi-recipient sender-confirmation truncation is not reproduced.

D11 now rejects every non-string Lua value with an argument error. Empty strings
are printable; byte values outside `0x20–0x7e`, including NUL and non-ASCII bytes,
are not. No UTF-8 conversion or numeric coercion takes place.

The original transcript remains historical. The [corrected paired run](../tests/fixtures/behavioral-wire-d10-d11-resolved.json)
and positive regressions in `behavioral_audit` cover both fixes; the TCP communication
regression covers partial-target formatting across multiple sessions and saved-page reuse.
The seven recorded findings are resolved; broad unverified areas and P01/P02 remain explicit.

D10–D11 validation: formatting, Clippy with warnings denied, and all 316 Rust tests passed.
The isolated C/Rust probe matched both corrected responses exactly.
