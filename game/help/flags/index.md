+++
title = "Flags"
description = "Index of Flags"
keywords = ["flags", "flag"]
article_tags = ["show_in_index"]
wizard_only = true

show_index_for_article_tags = ["flags"]
index_style = "list_with_description"
+++

# Flag Reference

Flags are boolean properties stored on objects. Their compact display letters
appear after an object's type letter in object descriptions. Use the full flag
name when setting or clearing one:

```text
@set <object>=<flag>
@set <object>=!<flag>
```

The normal rules for controlling the target still apply. Lua may inspect and
change flags as privileged server logic.
