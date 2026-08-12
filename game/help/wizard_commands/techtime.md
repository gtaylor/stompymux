+++
title = "Tech-time multiplier"
keywords = ["techtime multiplier", "repair time", "btech_techtime_multiplier"]
article_tags = ["wizard_commands"]
description = "Configure the BattleTech repair-time scale"
wizard_only = true
+++

# Tech-time multiplier

`battletech.techtime_multiplier` in `stompymux.toml` scales newly scheduled
BattleTech repairs. `1.0` is normal time, `0.5` is half time, and `1.5` is 150%
time. Values must be finite and between `0.0` and `10.0` inclusive.

Set it while the server is running with:

    @admin btech_techtime_multiplier=0.5

At `0.0`, new repairs add no player tech-time debt and complete after the event
scheduler's one-second minimum delay. Changing the value does not alter
existing debt or already scheduled event deadlines.
