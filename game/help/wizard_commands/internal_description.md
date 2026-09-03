+++
title = "@internal-description"
description = "Set or clear an object's internal description"
keywords = ["@internal-description", "internal description"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @internal-description

Set the styled description shown from inside a non-room object, or clear it
with an empty value.

```text
@internal-description <object>=<description>
@internal-description <object>=
```

When unset, looking from inside the object falls back to its ordinary
description. Use `@examine <object>` to inspect the stored value.
