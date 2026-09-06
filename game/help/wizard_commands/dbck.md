+++
title = "@dbck"
aliases = ["dbck"]
categories = ["wizard commands"]
+++

# @dbck

> `@dbck`

Checks the game database for inconsistencies, repairs damage, rebuilds the
free-object list, and purges objects marked for destruction. The command writes
damage details to the server log and reports `Done.` to the invoking Wizard
when complete.

Trusted Lua code can perform the same default check with `mux.check_db()`. The
Lua function does not send a completion message to a player and is unavailable
during `@lua/check`.
