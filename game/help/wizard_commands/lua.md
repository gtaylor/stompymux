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

Trusted runtime scripts may create rooms, things, and exits with the
`mux.world.create_*` functions and schedule any supported object for deletion
with `mux.world.destroy`. `mux.world.link_exit` links an exit to a destination;
pass `nil` as the destination to unlink it without detaching it from its source.
`mux.world.teleport` moves a thing or player using an extensible options table.
`mux.world.zone` reads an object's zone, and `mux.world.set_zone` assigns a
thing or room as its zone or clears the assignment with `nil`.
`mux.world.affiliation` reads an object's affiliation, and
`mux.world.set_affiliation` assigns any live object as its affiliation or
clears the assignment with `nil`. Affiliations do not affect command matching,
events, or other server behavior.
`mux.world.lua_parent` reads an object's direct object-logic module path, and
`mux.world.set_lua_parent` assigns a validated path or clears it with `nil`.
Object handles also expose `object:flags()` and `object:powers()`. Their
collection methods use typed constants such as `mux.world.flags.SAFE` and
`mux.world.powers.IDLE`; raw name strings are not accepted. These changes run
immediately as God and are not rolled back if the callback later fails.
These operations are unavailable during `@lua/check`; use the scripting
package reference for their arguments and safeguards.
