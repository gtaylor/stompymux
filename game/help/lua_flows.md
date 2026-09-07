+++
title = "Lua interactive flows"
description = "Conversation prompts and multi-step Lua input"
keywords = ["lua flows", "flow-demo", "interactive flows"]
+++

# Lua interactive flows

Try `flow-demo confirm`, `flow-demo menu` or `flow-demo signup` to explore the
supplied examples. Prompts belong to your current connection. While a flow is
active, every line, including `quit` and blank input, goes to its current step.
Follow that flow's cancellation instructions, or disconnect to abandon it.

Scripts start a conversation with
`mux.session.flow_start(ctx.descriptor, "module.lua", "first_step")`.
The module's `flows` table contains step functions. Each receives `ctx.input`
(nil for initial entry), `ctx.flow` scratch data and the target player's identity.
Return `repeat` with a `prompt`, `goto` with a `step`, or `done`/`cancel` with an
optional `message`. Prompts do not append a newline.

Scratch stores at most 16 string/number fields; numbers become strings between
steps. Keys are at most 31 bytes, values 8,191 bytes, and step names 63 bytes.
Invalid or excessive data fails atomically. Normal Lua resource limits also apply.

World changes are saved before output. Failed saves permit input retry; script
errors cancel the flow. Reload keeps scratch and step data, using the new code for
the next input. Disconnect and restart discard flows.
