+++
title = "home"
description = "Teleport to your configured home"
keywords = ["home", "go home"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# home

`home` teleports a Wizard to their configured home object. The command is
Wizard-only; other players receive `Permission denied.` and do not move.

The word `home` has no special meaning as an `@link` or `@teleport`
destination. In those commands it is matched as an ordinary object name.

Home uses the source's `leave/on_leave`, the traveler's `move/on_move` and the
destination's `enter/on_enter` actions, including cross-location messages. These
actions use cause `#-1`; a descriptor belongs only to the moving player. Arrival
appearance comes before move and enter actions. Returning to the current location
is a successful no-op. Failed callbacks or saves discard the move and its output.
