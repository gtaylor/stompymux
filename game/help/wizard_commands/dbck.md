+++
description = "Check and repair the world database"
title = "@dbck"
keywords = ["@dbck", "dbck"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @dbck

> `@dbck`

Checks the game database for inconsistencies, repairs damage, repairs containment lists, and purges objects marked for destruction.
Purged dbrefs remain Garbage tombstones and are not reused. The command writes
damage details to the server log and reports `Done.` to the invoking Wizard
when complete.

Trusted Lua code can perform the same default check with `mux.check_db()`. The
Lua function does not send a completion message to a player and is unavailable
during `@lua/check`.
