---
title: btech.repair
linkTitle: btech.repair
type: docs
weight: 20
no_list: true
sidebar_root_for: self
---

`btech.repair` exposes damage reports, technician calculations, and repair
state.

## Functions

| Function | Description |
| --- | --- |
| [`damages`](damages/) | Returns a formatted repair-job description. |
| [`job_count`](job-count/) | Returns the number of pending repair jobs. |
| [`tech_list`](tech-list/) / [`tech_list_ref`](tech-list-ref/) | Lists parts needed for repairs. |
| [`tech_status`](tech-status/) | Returns formatted repair status. |
| [`tech_time`](tech-time/) | Runs the technician-time query. |
| [`under_repair`](under-repair/) | Tests for an active repair event. |
| [`unit_fixable`](unit-fixable/) | Tests whether a unit can be repaired. |
