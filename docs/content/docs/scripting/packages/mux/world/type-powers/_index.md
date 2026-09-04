---
title: Powers
type: docs
weight: 40
toc_hide: false
no_list: true
sidebar_root_for: self
---

A `Powers` handle reads and changes the native powers on one live object.
Create one with [`Object:powers`](../type-object/powers/). Methods accept only
typed constants from [`mux.world.powers`](../powers/).

`tostring(powers)` produces `powers(#<dbref>)`. It raises
`mux.object.invalid` after the underlying object becomes stale.

Changes run immediately as God. They are not part of the State transaction and
are not rolled back if the callback later fails.

| Method | Description |
| --- | --- |
| [`Powers:list`](list/) | Lists the powers currently granted. |
| [`Powers:has`](has/) | Tests a power's presence. |
| [`Powers:add`](add/) | Grants a power and reports whether it changed. |
| [`Powers:remove`](remove/) | Removes a power and reports whether it changed. |
