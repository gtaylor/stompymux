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

Wizard-only Lua administration provides `/parent`, `/viewparent`, `/check`,
`/reload`, `/schedule` and `/test`. Bare `@lua` lists these switches. Interactive
flows remain unavailable.

Use `/parent` to attach active object modules and `/reload` to discover new files
or apply edits. `/viewparent` shows current disk source; `/check` validates a
candidate runtime without changing the world or running code in the active VM.

Command declarations require `name`, `permission`, `pattern` and `handler`.
Permissions are strings: `everyone`, `wizard` or `god`. Restricted Lua handlers
are skipped, allowing later handlers or exits to match.

Trusted scripts use `mux.world.create_object` with typed `mux.world.types`
constants. Objects expose destination, home, location, zone, affiliation and Lua
parent getters, and corresponding setters except for location. Clear optional
relationships with explicit `nil`; a home must remain a valid container.

Flags and powers use typed constants and provide `has`, `add`, `remove` and
`list`. `mux.comsys` manages channels. World mutations and output participate in
the enclosing transaction: callback or persistence failure restores mutations
and discards staged messages. Session-owned CONNECTED always reflects sessions.
Lua globals themselves are not rolled back. Test execution is a deliberate
exception: valid world mutations survive assertions and runtime errors in
`@lua/test`. See `help @lua/test` before running suites.
