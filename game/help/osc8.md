+++
title = "OSC 8 hyperlinks"
description = "Add capability-aware command and web links to styled text"
keywords = ["osc8", "osc 8", "hyperlinks", "send links", "prompt links"]
article_tags = ["show_in_index"]
wizard_only = true
+++

# OSC 8 hyperlinks

Styled text supports OSC 8 links with the same scoped syntax used for colors.
The visible text remains readable when a client does not advertise the
corresponding capability.

```text
[link="https://example.com"]Visit the website[/]
[send="look"]Look[/]
[prompt="cast fireball"]Prepare a spell[/]
```

`[link]` accepts `http:`, `https:`, and `ftp:` URIs. `[send]` immediately sends
a command when clicked, while `[prompt]` places it in the client's input line.
Commands are written normally; the server performs the required URI percent
encoding. Double quotes and backslashes in targets use `\"` and `\\`.

Links may contain color and formatting tags, but links cannot be nested. Link
targets must be printable UTF-8, and web URIs must already use percent encoding
for bytes that are not valid URI characters.

The server emits each link only when the connection reports an exact value of
`1` for its NEW-ENVIRON USERVAR: `OSC_HYPERLINKS`,
`OSC_HYPERLINKS_SEND`, or `OSC_HYPERLINKS_PROMPT`. These capabilities are
independent of the player's ANSI setting. Without support, only the text inside
the tag is displayed.

Player-authored say, page, and ordinary channel messages continue to remove all
styled-text markup before delivery.
