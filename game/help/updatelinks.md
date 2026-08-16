+++
title = "UPDATELINKS"
description = "Refresh a BattleTech map's recursive building links"
keywords = ["updatelinks", "buildlinks", "buildcoord", "buildentrance"]
article_tags = ["battletech"]
+++

# UPDATELINKS

`UPDATELINKS` rebuilds the current BattleTech map's `BUILD`, `LEAVE`, and
`ENTRANCE` objects from `BUILDLINKS`, `BUILDCOORD`, and `BUILDENTRANCE`
attributes. Linked maps are updated recursively.

Each map is processed at most once per command. Cyclic and repeated links are
left as `BUILD` objects, but the command does not descend into their target a
second time. Self-links remain excluded. The completion message reports how
many link descents were skipped because a target was already visited or the
safety depth limit was reached.
