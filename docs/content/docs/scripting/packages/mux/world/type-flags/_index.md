---
title: Flags
type: docs
toc_hide: false
no_list: true
sidebar_root_for: self
---

A `Flags` handle reads and changes the native flags on one live object. Create
one with [`Object:flags`](../type-object/flags/). Methods accept only typed
constants from [`mux.world.flags`](../flags/).

Changes run immediately as God through the native flag handlers, including
XCODE transition side effects. They are not part of the State transaction and
are not rolled back if the callback later fails.

| Method | Description |
| --- | --- |
| [`Flags:list`](list/) | Lists the flags currently set. |
| [`Flags:has`](has/) | Tests a flag's presence. |
| [`Flags:add`](add/) | Sets a flag and reports whether it changed. |
| [`Flags:remove`](remove/) | Clears a flag and reports whether it changed. |
