+++
title = "Powers"
description = "Index of object powers"
keywords = ["powers", "power"]
article_tags = ["show_in_index"]
wizard_only = true

show_index_for_article_tags = ["powers"]
index_style = "list_with_description"
+++

# Power Reference

Powers are boolean privileges stored on objects separately from flags. The
current MUX server has one power, `IDLE`.

Only Wizards and God may grant or remove powers, and the normal control rules
apply to the target:

```text
@power <object>=<power>
@power <object>=!<power>
```

Use `@list powers` to list registered powers, `@examine <object>` to inspect a
target's powers, or `@search power=idle` to find objects with the stored power.
Power names are case-insensitive in both `@power` and `@search`.
