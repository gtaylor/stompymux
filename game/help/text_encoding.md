+++
title = "Text encoding"
description = "UTF-8 text and ASCII identifier rules"
keywords = ["utf8", "utf-8", "encoding", "player names", "aliases"]
article_tags = ["concepts"]
weight = 80
+++

# Text encoding

StompyMUX accepts and sends UTF-8 text. Messages, descriptions, object names,
exit aliases, macro expansions, and other player-authored text may use UTF-8.
Malformed UTF-8 input is rejected.

Player names and player aliases must use printable ASCII. Channel names,
channel command aliases, and macro aliases must also use printable ASCII;
channel names and channel aliases cannot contain spaces. These restrictions
keep login and command identifiers predictable while allowing UTF-8 content.
