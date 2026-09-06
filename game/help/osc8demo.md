+++
title = "OSC8DEMO"
keywords = ["osc8demo", "osc 8 demo", "hyperlink demo"]
article_tags = ["show_in_index"]
description = "Display examples of OSC 8 hyperlink features"
+++

# OSC8DEMO

`osc8demo` displays examples from the first six OSC 8 tiers:

- Tier 1 web, immediate-command, and input-prompt links.
- Tier 2 base colors, formatting, and hover, active, visited, and keyboard-focus
  styles.
- Tier 3 custom tooltip text and a titled right-click menu containing send,
  prompt, and web actions.
- Tier 4 timed conceal/reveal behavior, spoilers, reveal-only spoilers, and
  permanently disabled links.
- Tier 5 exclusive radio choices, non-exclusive checkboxes, initial selection,
  non-toggleable selection, and a disabled selection option.
- Tier 6 reusable preset buttons, a per-link preset override, and compact wire
  configuration for clients that advertise it.

Try hovering over and focusing the styled links, left-clicking each link, and
right-clicking the Tier 3 example, and activating the Tier 4 through Tier 6
examples. The
exact enhancements depend on the OSC 8 capabilities advertised by your
client. Clients without OSC 8 support still display the explanatory text and
link labels.

See `help osc8` for the builder markup and capability variables used by the
demo.
