+++
title = "@newpassword"
keywords = ["@newpassword"]
article_tags = ["wizard_commands"]
description = "Reset a player's password"
wizard_only = true
+++

# @newpassword

```text
@newpassword <player>=<password>
```

Resolve a player by name, alias or dbref. Wizards may reset other Wizards, but
nobody can reset GOD's password with this command. Empty passwords are rejected;
normal configured password validation applies.

Only one reset per target may be pending. Hashing runs asynchronously, and the
caller must remain connected and authorized. Confirmation and notification to
the target follow successful persistence. Existing sessions remain connected;
pending logins using the previous credentials cannot complete after the reset.
