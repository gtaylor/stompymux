+++
title = "@name"
description = "Change an object's name"
keywords = ["@name", "name"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @name

```text
@name <object>=<new name>
```

Changes the name of an object you control. Object names accept the syntax
described in `help color`.

Player display names may use validated markup. Login and uniqueness checks use
the style-stripped name. Case-only changes are permitted.
Player names and player aliases are restricted to printable ASCII. Names for
rooms, things, and exits may contain UTF-8. An exit's semicolon-separated
aliases may also contain UTF-8.
