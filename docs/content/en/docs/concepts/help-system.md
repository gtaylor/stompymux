---
title: Help system
linkTitle: Help system
type: docs
weight: 20
---

The `help` command serves markdown articles with TOML frontmatter, indexed
and rendered entirely inside `stompymux` (see `src/mux/help`). There is no
separate indexing binary and no distinction between `help` and `wizhelp` -
per-article visibility is controlled by the `wizard_only` frontmatter key
instead.

This page documents the engine side of the system, for contributors to the
StompyMUX codebase itself. It is not the place to document any particular
game's help content - that content lives in `game/help/` and is up to each
game's admins to write and maintain.

## Article format

Articles are markdown files under the directory named by the `help_directory`
mudconf directive (default `help`, i.e. `game/help/`), recursively. Each file
starts with a TOML frontmatter block delimited by `+++` lines, followed by
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

Article bodies are parsed with a vendored `cmark` (CommonMark) and walked as
an AST, not passed through any of cmark's built-in renderers - headers are
emitted as literal `#`/`##` lines, links and images are reduced to their
visible text, and emphasis/strong markers are stripped. This is a deliberate,
minimal plain-text format, not a general Markdown-to-ANSI renderer.

## Reindexing

`@help/reload` (Wizard-only) rebuilds the entire index from scratch - this is
also what happens once at server startup. Both paths log errors and a
summary to the server log; `@help/reload` also reports them to the invoking
player. Frontmatter is parsed with a vendored `tomlc17`.

## Configuration

The `help_directory` mudconf directive (God-settable) points at the article
root, relative to the server's working directory. Changing it does not
reindex automatically - run `@help/reload` afterward.
