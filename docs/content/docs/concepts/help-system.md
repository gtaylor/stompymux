---
title: Help system
linkTitle: Help system
description: How the in-game help system works
type: docs
weight: 20
---

The `help` command serves Markdown articles with TOML front matter. Rust code
in `src/help.rs` builds the index, and `src/help/render.rs` renders replies
through the shared text system. Per-article visibility is controlled by the
`wizard_only` front matter key.

This page documents the engine side of the system, for contributors to the
StompyMUX codebase itself. It is not the place to document any particular
game's help content - that content lives in `game/help/` and is up to each
game's admins to write and maintain.

## Article format

Articles are Markdown files under the directory configured by
`mux.help_directory` (default `help`, relative to the game root),
recursively. Each file starts with a TOML front matter block delimited by
`+++` lines, followed by
the markdown body:

```markdown
+++
title = "About this game"
description = "All about this game"
keywords = ["about"]
article_tags = ["show_in_index"]
+++

# About this game

Body content here.
```

### Frontmatter fields

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `title` | string | yes | |
| `description` | string | yes | |
| `keywords` | list of strings | yes | Matched exactly (case-insensitive) by `help <term>`. |
| `article_tags` | list of strings | no | Which indices this article shows up in. |
| `show_index_for_article_tags` | list of strings | no | Enables index-rendering mode; lists all articles matching at least one of these tags. |
| `index_style` | `columnar` or `list_with_description` | no | Defaults to `list_with_description`. |
| `weight` | integer | no | Sort order within an index; unweighted articles sort after weighted ones, alphabetically by their first `article_tags` value. |
| `wizard_only` | boolean | no | Restricts this article to Wizards in both index listings and `help` lookups. |

An article missing a required field is skipped (not indexed) and reported as
an error. If two articles declare the same keyword, the first one encountered
during a sorted, depth-first directory walk keeps it; the other is logged as
a warning and keeps its remaining keywords (if any) but loses reachability
via the duplicate.

### The default article

`help` with no arguments renders `help/index.md` if it has been indexed, or
else reports `Unable to render default help article`.

## Rendering

Article bodies use the Rust Markdown renderer in `src/text/markdown.rs`.
External `http:`, `https:`, and `ftp:` links can become capability-aware
OSC 8 links. Clients without OSC 8 support still receive the visible text.
Index entries are rendered as actions that send `help <topic>` when the client
supports them.

## Reindexing

`@help/reload` (Wizard-only) rebuilds the index, as startup does. Index
construction runs on a blocking worker; a failed rebuild leaves the previous
index installed. Individual invalid articles are skipped and reported. Both
paths log errors and a summary; reload also reports them to the invoking
player. Rust's TOML parser reads the front matter.

## Configuration

The `mux.help_directory` configuration value (also accepted as
`help_directory`) points at the article root, relative to the game root.
Changing it does not reindex automatically; run `@help/reload` afterward.
