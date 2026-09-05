---
title: btech.parts
linkTitle: btech.parts
type: docs
weight: 0
no_list: true
sidebar_root_for: self
---

`btech.parts` provides the part catalogue and object stores.

Where a `BtechPartRef` accepts a packed integer ID, that ID must be from 0
through 2,147,483,647; values outside this range raise `mux.arg.invalid`.

## Functions

| Function | Description |
| --- | --- |
| [`adjust_stores`](adjust-stores/) | Adjusts a stored part quantity. |
| [`categories`](categories/) | Lists the canonical part categories. |
| [`list`](list/) | Lists parts, optionally restricted to one category. |
| [`resolve`](resolve/) | Resolves a part reference to its canonical record. |
| [`search`](search/) | Searches the part catalogue by name. |
| [`set_cost`](set-cost/) | Sets a part's configured cost. |
| [`store_quantity`](store-quantity/) | Returns one stored part quantity. |
| [`stores`](stores/) | Lists every stored part with a nonzero quantity. |
