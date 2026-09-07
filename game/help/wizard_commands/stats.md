+++
title = "@stats"
keywords = ["@stats"]
article_tags = ["wizard_commands"]
description = "Display database object counts by type"
wizard_only = true
+++

# @stats

`@stats` displays the total number of database objects together with counts for
rooms, exits, things, players, and garbage.

## Example

```text
@stats
```

GOING non-room objects count as garbage; GOING rooms remain rooms. The total
includes allocated dbref slots, with gaps counted as garbage. This does not create
objects or change the database. The command takes no arguments or switches.
