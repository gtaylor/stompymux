+++
title = "@session"
keywords = ["@session"]
article_tags = ["wizard_commands"]
description = "Inspect active client sessions"
wizard_only = true
+++

# @session

`@session` is a wizard-only connection diagnostic. It lists each active client
session with its stable Rust session ID, connection and idle time, and input/output queue and
byte counters. Give the first part of a player's name to limit the listing:

## Example

```text
@session Alex
```

Use `@telnet <player>` for negotiated options, environment values and compression
state. Session diagnostics are private to your invoking connection. Reports are
bounded and explicitly mark truncation. Both commands accept configured aliases
and reject switches. Idle times of ten minutes or less display as zero, matching
the C server.

Input totals count socket bytes. Output totals count logical Telnet bytes before
compression; `@telnet` separately shows actual wire output bytes. Pending output
includes the message currently being written; failed messages count as lost.
