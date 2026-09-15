+++
title = "UPDATELINKS"
description = "Refresh a BattleTech map's recursive building links"
keywords = ["updatelinks", "map links", "map entrances"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# UPDATELINKS

`UPDATELINKS` rebuilds reachable building entrances and return links from authored
child-map configuration, starting at your current map. It reports the number of
building, return and arrival records created and the descents skipped for cycles
or the 1,024-level depth limit. Existing runtime routes on visited maps are rebuilt;
the root's own return and arrival records are retained unless reciprocal cleanup
removes them. Unrelated map objects and units remain in place.

Trusted Lua configures each child's parent placement and four cardinal entrance
modes with `btech.map.set_link(child, link)`. An entrance can be absent, an inward
offset from its map edge, or an exact coordinate. `btech.map.link(child)` inspects
that configuration. `btech.map.update_links(actor, map)` performs the same rebuild
as the native command. Configuration changes take effect when links are rebuilt.
