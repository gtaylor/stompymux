+++
title = "@help/reload"
description = "Rebuild the help index"
keywords = ["@help/reload"]
article_tags = ["help_switches"]
weight = 10
wizard_only = true
+++

# @help/reload

Rebuild the complete help index from the configured help directory:

```text
@help/reload
```

This performs the same indexing operation used during server startup. Errors,
duplicate-keyword warnings, and the indexing summary are written to the server
log and reported to the invoking Wizard.
