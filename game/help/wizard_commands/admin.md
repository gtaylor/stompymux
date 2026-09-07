+++
title = "@admin"
description = "Change runtime configuration, aliases and access"
keywords = ["@admin", "runtime configuration"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @admin

Use `@admin <directive>=<value>`. The command requires Wizard access, and most
individual directives require GOD. No switches are supported.

```text
@admin max_players=50
@admin alias=inspect @examine/brief
@admin access=@shutdown !wizard god
@admin config_access=max_players !god wizard
@admin default_thing_flags=ansi safe
@admin bad_name=Guest*
@admin good_name=Guest*
@admin forbid_site=192.0.2.0 255.255.255.0
```

Use exact legacy directive names. `@list config_permissions` lists effective
permissions and live, restart-only or unsupported status. Changing permissions
cannot enable unavailable features or live changes to restart-only resources.

Edits are runtime-only: they survive Lua reload and disappear on restart. Nothing
is written to configuration files or the game database. Rust commands and Lua
configuration queries observe new settings after `Set.`.

Multi-token access and flag edits can partially succeed: valid tokens take effect
and invalid tokens produce diagnostics. An invalid scalar or structural constraint
leaves the setting unchanged. New site rules take precedence, applying only to
new connections. `good_name` removes a bad-name pattern.

New defaults affect future objects. Directory edits require explicit help/Lua
reload before new content is indexed or loaded. Existing deadlines are retained;
new intervals apply when scheduling the next check. Existing queued work and
object state are not deleted when limits decrease.

See [configuration reports](list.md).
