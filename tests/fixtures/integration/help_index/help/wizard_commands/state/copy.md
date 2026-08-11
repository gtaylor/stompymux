+++
title = "@state/copy"
description = "Copy a persistent state value"
keywords = ["@state/copy"]
article_tags = ["state_switches"]
wizard_only = true
+++

# @state/copy

Copy a persistent value to another namespace or attribute on the same object:

```text
@state/copy <object>/<namespace> <attribute_name>=<namespace> <attribute_name>
```

The copy retains the source value's scalar type. An existing destination value
is replaced. Namespace and attribute names are case-sensitive.
