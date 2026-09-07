+++
title = "Speech and poses"
keywords = ["say", "pose", "speech", "pose/nospace"]
article_tags = ["show_in_index"]
description = "Speak, pose and emit in your surroundings"
+++

# Speech and poses

| Input | What others see |
| --- | --- |
| `say Hello` or `"Hello` | `YourName says "Hello"` |
| `pose waves` or `:waves` | `YourName waves` |
| `pose/nospace 's hat falls` or `;'s hat falls` | `YourName's hat falls` |
| `: waves` | `YourNamewaves` |
| `\A bell rings.` | `A bell rings.` |

The quoted speaker sees `You say "Hello"`. Poses and emits include the speaker.
Bracket styling is stripped from these message bodies, and Markdown is not
interpreted. Configured command aliases work normally.

GAGGED prevents ordinary players from speaking, posing or emitting. In an
AUDITORIUM, the location's SPEAK lock decides whether these actions are allowed.
Wizards bypass GAGGED but still encounter auditorium speech policy.

Messages follow the enclosing object's routing rules. AUDIBLE containers can
forward outward with `From <container>,` prefixes; audible exits can relay with
`From a distance,` prefixes. Private delivery does not automatically descend into
containers: the C fork's listener-only forwarding branch is not implemented.

The backslash shorthand is available to everyone, matching C. The explicit
`@emit` command remains Wizard-only. See `help message commands` for administrative
messages and broadcasts.
