+++
title = "UPDATELINKS"
description = "Refresh a BattleTech map's recursive building links"
keywords = ["updatelinks", "map links", "map entrances"]
article_tags = ["battletech"]
+++

# UPDATELINKS

`UPDATELINKS` rebuilds the current BattleTech map's `BUILD`, `LEAVE`, and
`ENTRANCE` objects from typed map-link configuration. Linked maps are updated
recursively.

Each child map has at most one configured parent. A parent may contain any
number of child maps.

Trusted Wizard Lua can read and write this configuration with
`btech.map.link(child)` and `btech.map.set_link(child, link_or_nil)`. The setter
does not rebuild live map objects; run `UPDATELINKS` on the affected parent
after completing the configuration edits.

Each map is processed at most once per command. Cyclic and repeated links are
left as `BUILD` objects, but the command does not descend into their target a
second time. Self-links remain excluded. The completion message reports how
many link descents were skipped because a target was already visited or the
safety depth limit was reached.
