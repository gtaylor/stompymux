+++
title = "@last"
keywords = ["@last"]
article_tags = ["wizard_commands"]
description = "Inspect player login history"
wizard_only = true
+++

# @last

`@last [player]` displays successful and failed login history for a player.
When no player is supplied, it displays your own history. The command is
Wizard-only, and Wizards may inspect any player, including themselves and
other Wizards.

Login times are displayed in ISO 8601 UTC, with a trailing `Z`, for example
`2026-08-07T03:47:59Z`.

## Examples

```text
@last
@last Alex
```
