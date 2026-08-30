+++
title = "@lua"
description = "Administer the Lua scripting runtime"
keywords = ["@lua", "lua administration"]
article_tags = ["wizard_commands"]
wizard_only = true

show_index_for_article_tags = ["lua_switches"]
index_style = "list_with_description"
+++

# @lua

`@lua` groups the Wizard-only Lua administration commands under one command.
Type `@lua` by itself to see a short list of switches, or use one of the forms
indexed below.

Lua modules are the supported way to define programmable commands. Attribute
values beginning with `$` are not matched as commands.

Lua command entries may set `access = mux.world.access.WIZARD` or
`access = mux.world.access.GOD` alongside their `pattern` and `handler`.
Omitting `access`, or setting it to `mux.world.access.PUBLIC`, allows everyone.
Raw access strings are invalid. Unauthorized entries are skipped silently so
later command entries may still match.
