+++
title = "@state/wipe"
description = "Wipe persistent state from an object"
keywords = ["@state/wipe"]
article_tags = ["state_switches"]
wizard_only = true
+++

# @state/wipe

Wipe persistent object state as a Wizard:

```text
@state/wipe <object>
@state/wipe <object>/<namespace>
```

The first form removes every value in every namespace on the object. The second
removes only values in the named namespace. The command reports how many values
were removed.
