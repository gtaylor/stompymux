+++
title = "@state/move"
description = "Move a persistent state value"
keywords = ["@state/move"]
article_tags = ["state_switches"]
wizard_only = true
+++

# @state/move

Move a persistent value to another namespace or attribute on the same object:

```text
@state/move <object>/<namespace> <attribute_name>=<namespace> <attribute_name>
```

The move retains the source value's scalar type and replaces an existing
destination value. The destination write and source deletion are one
transaction. Moving a value onto itself succeeds without deleting it.
