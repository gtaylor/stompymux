+++
title = "@log"
aliases = ["log"]
categories = ["wizard commands"]
+++

# @log

> `@log <filename>=<message>`

Writes a message to an existing file in the game's `logs` directory. The
filename may not contain `/` or `..`, and the file must already be readable
and writable by the game server.

Lua code can perform the same operation with `mux.log(filename, message)`.
It returns `true` when the message was written and `false` when validation or
the write fails. This operation is not available during `@lua/check`.
