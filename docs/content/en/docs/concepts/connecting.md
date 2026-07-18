---
title: Connecting and creating a character
linkTitle: Connecting
type: docs
weight: 15
---

The connect screen is backed by the same interactive-flow engine
[Lua uses](../../scripting/flows/) (`src/mux/network/input_flow.c`), with the
connect/create logic itself in `src/mux/network/connect_flow.c`.

## The flow

A new connection drops straight into the flow - there's no banner-only
pause, no menu, and no need to press enter first. The very first thing
shown is:

```text
Character name:
```

What happens next depends on whether that name belongs to an existing
character:

- **Existing name** - prompts for a password. A wrong password re-prompts
  for the name (not the password) and decrements a per-connection retry
  counter; hitting zero disconnects.
- **Unrecognized name** - asks `Create a new one? (Y/n)`. Answering yes (or
  just pressing enter, per the `Y/n` convention) prompts for a password and
  then a confirmation of that password, retrying the password step on a
  mismatch. Answering no goes back to the name prompt.

New character names must start with a letter and contain at least two
characters. Existing characters with older names remain able to log in.

Every step is fully modal: whatever line the player sends is always the
answer to the current prompt. There's no escape hatch back to ordinary
commands, and pre-login `WHO`/`DOING`/`SESSION`/`QUIT` are no longer
reachable - quitting is done by disconnecting the client. After logging in,
use `quit` to disconnect the current session.

## Rate limiting

A wrong password decrements the per-connection retry counter described
above. Separately, a global address-keyed token bucket throttles rapid
repeated connect/create attempts regardless of which step triggered them,
disconnecting the attempt outright rather than allowing a retry.
