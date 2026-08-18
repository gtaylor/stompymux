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

`@lua/test` runs fully mutable Lua tests against the currently loaded database.
Run it only with a scratch database, never production data.

Lua command entries may set `access = "wizard"` or `access = "god"` alongside
their `pattern` and `handler`. Omitting `access`, or setting it to `"public"`,
allows everyone. Unauthorized entries are skipped silently so later command
entries may still match.
Lua callback reporting is configured with `lua_error_reporting`: `off`,
`wizards` (the default), or `all`. Errors are always logged; this setting only
controls player-visible reporting.
