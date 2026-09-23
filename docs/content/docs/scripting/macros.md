---
title: "Managing player macros"
type: docs
---

`mux.macro` manages the same sets used by the dot-command macro system. It is a
trusted administrative API: game scripts must enforce player-facing access rules.
Lua may inspect private sets, edit locked sets, change owners, and attach private
sets to players. The stored `LOCKED`, `READ`, and `WRITE` permissions still apply
to ordinary player commands.

```lua
local macros = mux.macro
local set = macros.create_set(ctx.enactor, "Travel shortcuts")
set:add_macro("home", "go home")
set:add_macro("hi", "say Hello, *!")
set:update_macro("HI", "say Welcome, *!")
set:flags():add(macros.flags.READ)
local slot = macros.attach(ctx.enactor, set)

for _, attachment in ipairs(macros.list_player_sets(ctx.enactor)) do
    -- Slots are 0–4; the returned array is dense and indexed from 1.
    local number = attachment.set:number()
end

set:delete_macro("hi")
macros.detach(ctx.enactor, slot)
macros.destroy_set(set)
```

See the [package reference](../packages/mux/macro/) for all signatures.

## Sets and handles

`list_sets()` returns all set handles in current set-number order. `set(number)`
resolves a nonnegative integer set number, returning `nil` when no set occupies it.
Set numbers start at zero and change when earlier sets are destroyed. Handles
retain their identity across renumbering and commits; equality compares identity.
Destroyed handles and their flags handles raise `mux.macro.invalid`. Recreating
another set at the same number does not revive an old handle. Handles are runtime
values, not persistent identifiers: reacquire them after restart.

Creation requires an explicit live owner and description. Owners may be players,
rooms, things, or exits. New sets are private, unlocked, empty, and unattached.
`owner()` returns an Object handle; `set_owner(owner)` accepts an Object or dbref.
`description()` reads the description; this package does not rename sets.

`list_macros()` returns detached `{alias, expansion}` records sorted by
case-insensitive alias. Aliases contain 1–4 printable ASCII characters without
whitespace and match case-insensitively. Updates replace only expansion text and
preserve the alias's original spelling. Expansions are nonempty UTF-8 strings;
expansions and descriptions allow up to 8,191 bytes and cannot contain NULs.
Text is preserved without trimming. Existing substitution and command precedence
rules apply when players invoke these macros.

## Permissions and attachments

`set:flags()` returns a live collection supporting `has`, `list`, `add`, and
`remove`. These accept only the immutable `mux.macro.flags.LOCKED`, `.READ`, and
`.WRITE` constants. `list()` returns enabled constants in that order. Changes
preserve unknown stored permission bits.

Players have five attachment slots, numbered 0–4. `attach(player, set)` returns
the first free slot and permits duplicate attachments. It leaves the editing
selection unchanged and works for offline players. `list_player_sets(player)`
returns occupied `{slot, set, selected}` records in slot order. `detach(player,
slot)` returns whether it removed an attachment and clears the editing selection
if necessary. Destroying a set clears its attachments for every player; surviving
slots keep their positions. List results are snapshots; editing them changes no
server data.

Mutators return no values except `create_set` (handle), `attach` (slot), and
`detach` (boolean).

## Transactions and errors

All live operations require an active game callback transaction. Module-loading
code can access constants but cannot inspect or mutate macro state. Checking mode
raises `mux.unavailable.checking`; calls outside callbacks raise
`mux.state.unavailable`.

Failed operations validate before changing data. Catching a failed operation does
not undo earlier successful operations in the same callback. An enclosing callback
failure restores the world; a persistence failure restores the server's candidate
transaction. Handles to rolled-back creations remain invalid. Database writes use
the existing macro tables, preserving extension columns and supported mode bits.

Errors work with `mux.error.pcall`, `mux.error.is`, and the checked
`mux.error.codes.macro` tree:

| Code | Meaning |
| --- | --- |
| `mux.macro.invalid` | Set handle no longer exists. |
| `mux.macro.not_found` | Alias to update or delete does not exist. |
| `mux.macro.exists` | Alias to add already exists, ignoring case. |
| `mux.macro.slots_full` | Player has no free attachment slots. |
| `mux.arg.invalid` | Invalid text, number, slot, handle argument, or flag. |
| `mux.object.invalid` | Invalid owner or non-player attachment target. |

## Default sets for new players

Configure an ordered list of zero-based set numbers in `stompymux.toml`:

```toml
[mux]
default_player_macros = [0]
```

The compiled default is `[0]`. Use `[]` to disable automatic attachments or a list
such as `[0, 2]` for multiple defaults. Entries must be nonnegative integers, with
at most five entries to fit the player's attachment slots. Duplicate numbers are
allowed and retain their order.

Configuration validates the list's format, not whether its sets exist. Lua
bootstrapping can therefore create set 0 with `mux.macro.create_set` before later
players are created. Each newly allocated player resolves the list against the
current catalog, attaches found sets in order to the first free slots, and leaves
the editing selection unset. This trusted policy also attaches private or locked
sets. Missing sets are skipped, with a `MAC/WARN` server diagnostic naming the
player and missing set; creation still succeeds and later entries are processed.

GOD can change the list live with `@admin default_player_macros=[0, 2]` (or `[]`).
The change affects subsequent player creation only. Existing players, including
bootstrap players created before a set exists, are not retroactively updated.
As with other macro APIs, set numbers refer to current catalog positions and may
change after sets are destroyed.
