+++
title = "@lua/viewparent"
description = "Display an object Lua parent's source"
keywords = ["@lua/viewparent"]
article_tags = ["lua_switches"]
weight = 50
wizard_only = true
+++

# @lua/viewparent

Display the raw source of an object-logic module. Supply either an object dbref
or a Lua parent path relative to `game/lua/object_logic`:

```text
@lua/viewparent #4
@lua/viewparent hello.lua
```

The dbref form displays the Lua parent attached directly to that object. The
path form must use the same safe relative `.lua` path accepted by
`@lua/parent`.

Source is read from disk on each request and displayed literally in private,
bounded chunks with whitespace preserved. It may differ from active code; use
`@lua/reload` to activate edits. Viewing a new file does not register it.
