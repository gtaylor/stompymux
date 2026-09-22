---
title: "btech.autopilot"
linkTitle: "btech.autopilot"
type: docs
weight: 5
sidebar_root_for: self
no_list: true
---

`btech.autopilot`: Lua control of unit-attached ground autopilots.

## Functions

- [`attach`](attach/)
- [`cancel`](cancel/)
- [`configure`](configure/)
- [`detach`](detach/)
- [`feedback`](feedback/)
- [`observe`](observe/)
- [`pause`](pause/)
- [`resume`](resume/)
- [`status`](status/)
- [`submit`](submit/)

## Constants

- [`btech.autopilot.fire_modes`](fire_modes/)
- [`btech.autopilot.order_states`](order_states/)
- [`btech.autopilot.orders`](orders/)
- [`btech.autopilot.reasons`](reasons/)
- [`btech.autopilot.states`](states/)
- [`btech.autopilot.submission_modes`](submission_modes/)

Attach a controller to a ground unit, submit typed orders, and inspect its observations and feedback. Controllers resume persisted work after restart.
