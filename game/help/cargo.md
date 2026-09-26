+++
title = "Cargo"
description = "Inspect and transfer loose parts and supplies"
keywords = ["fixstuff", "cargo", "manifest", "stores", "loadcargo", "unloadcargo"]
article_tags = ["show_in_index"]
+++

# Cargo

`manifest [pattern]` lists the stock in your current location, including carried
stock when you are inside a unit. `stores [pattern]` lists the hangar's stock
from inside a running unit at its loading point.

`loadcargo <pattern> <quantity>` moves matching hangar stock into your unit.
The unit must have CargoTech, be running and stationary, and be on a map that
is not marked InCharacter. If the map has a configured loading point, move to
that hex first. A hidden loading point does not disclose its coordinates.

`unloadcargo <pattern> <quantity>` moves carried stock onto the current map.
It also works while shut down or moving, and does not require the loading point
or a hangar map. Both transfer commands require you to be inside the unit,
with the unit physically on its assigned map. In an InCharacter unit, only its
assigned pilot or a Wizard can transfer cargo. An unconscious pilot cannot
operate the controls.

Names ignore case. Use `*` for any sequence of characters and `?` for one
character, for example `loadcargo Gold 10` or `loadcargo Ammo_* 5`.
A numeric part identifier, optionally prefixed by `#`, selects that part across
manufacturers. A weapon name such as `Magna.IS.MediumLaser` selects its named
manufacturer; `MediumLaser` selects all stocked manufacturers.

Transfers first resolve an exact abbreviation, then an exact full catalogue
name, and finally a wildcard pattern. `ML` and `IS.MediumLaser` select the
unbranded medium laser. Abbreviations retain capitals, digits and underscores:
`AC20` means `AC/20`, and `A_LRM10` means `Ammo_LRM-10`. Manufacturer
abbreviations precede the part abbreviation, for example `Lo.ML` for Lords.
If names collide, the lowest manufacturer number and then lowest part identifier
wins. `Ma.ML` selects Martell; spell out Magna to distinguish it. An exact
selection that is out of stock does not fall through to another manufacturer.
Use a numeric part identifier to distinguish duplicate names such as `Steel`,
or a wildcard to request multiple matches. Manifest and stores filters use
wildcard names directly, without transfer abbreviation priority.

The positive quantity applies to each matching stock row. Each row transfers
at most 50,000 items and never more than its available stock. All selected rows
transfer together: an error leaves the stock unchanged. Carried mass affects
movement, and transfers immediately reduce throttle settings when needed.
Stock and loading points survive restart.

The server's cargo-command setting controls all four commands, including for
Wizards. Cargo stock does not install equipment. These commands move loose
parts and supplies; carrying another unit is not supported.


Wizards can use `fixstuff` to clean loose inventory at their current location.
It removes unknown part identifiers and structural critical placeholders,
combines matching part/manufacturer records and retains positive quantities.
The summary reports the old and new record counts and the total items kept.
Manufacturers remain separate. Installed equipment and armor are unaffected;
carrying limits are refreshed immediately for units. No cockpit, startup or
cargo-command setting is required. Lua uses `btech.inventory.fix(actor, object)`
and returns the same totals. A failed callback restores the inventory.
