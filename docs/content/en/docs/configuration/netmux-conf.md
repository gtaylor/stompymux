---
title: netmux.conf reference
linkTitle: netmux.conf
weight: 20
---

`game/netmux.conf` controls the running game server. Configuration changes
can be made through the appropriate wizard configuration commands or by editing
the file before starting the server.

## Password and login security

Passwords are stored as Argon2id hashes through the vendored libsodium library.
Each stored hash includes its salt, algorithm, and work factors. Password hashes
are never recoverable, and legacy `crypt(3)` password hashes are not accepted.

| Parameter | Default | Description |
| --- | ---: | --- |
| `player_password_length_limit` | `64` | Maximum password length in characters. Password creation and password changes reject longer values; login attempts longer than this limit are rejected before password hashing. |
| `password_hash_opslimit` | `3` | Argon2id CPU work factor. Increase only after measuring login latency on the game host. Values below `1` disable password hashing and prevent password changes and new player creation. |
| `password_hash_memlimit` | `12582912` | Argon2id memory work factor in bytes (12 MiB). Values below 1 MiB are rejected. A higher value makes offline password guessing harder but consumes more memory per password operation. |
| `login_attempt_burst` | `3` | Number of password operations a source address may make immediately. |
| `login_attempt_refill` | `10` | Seconds required to restore one attempt for a source address. |
| `login_hash_limit` | `5` | Global maximum number of password operations admitted per second. This protects the single game event loop from a distributed login flood. |

The per-source tracker has room for 1,024 recent addresses and evicts the least
recently refilled entry when full. A throttled connection receives the same
generic failure response as an invalid login and is disconnected. Keep the
global rate low enough that password verification cannot consume all event-loop
time, and tune the Argon2id settings on the production host rather than aiming
for a one-second hash.

The defaults intentionally favor a responsive telnet game server. They are
lighter than libsodium's interactive preset, so the firewall or host should
also rate-limit new TCP connections to the game port.
