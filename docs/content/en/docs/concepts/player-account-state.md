+++
title = "Player account state"
description = "How authentication, login history, and page memory are stored"
+++

# Player account state

Player authentication and activity metadata are typed server state rather than
object attributes. The SQLite `player_state` table stores password hashes,
aliases, the last login and site, and aggregate login counters. Login attempts
are normalized in `player_login_history`; remembered page recipients are stored
in `player_last_page_recipients`.

All wall-clock values use signed Unix epoch seconds in SQLite. Commands that
show login history render those values as ISO 8601 UTC. Game ticks and elapsed
durations are not wall-clock timestamps and remain in their native units.

`Alias` remains a native object attribute because changing it participates in
the live player-name index. Passwords, login metadata, and page memory are not
available through the attribute interface.
