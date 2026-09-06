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

The new index replaces the old one after directory traversal succeeds. Invalid
individual articles are skipped; duplicate keywords retain the first file in
lexical order. If the help directory cannot be read, the previous index remains
available. The summary reports article, keyword, error and warning counts.

Body-only edits appear on the next `help` request without reloading. Reload after
changing keywords, tags, visibility, index ordering, or adding/removing articles.
