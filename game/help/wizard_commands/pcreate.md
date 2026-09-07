+++
title = "@pcreate"
keywords = ["@pcreate"]
article_tags = ["wizard_commands"]
description = "Create an offline player account"
wizard_only = true
+++

# @pcreate

```text
@pcreate <name>=<password>
```

Create a player with the configured starting room, home, flags, Lua parent and
public-channel setup. Normal name and password validation applies. Creation
neither connects the player nor records a login. The confirmation includes the
name and dbref, never the password.

Hashing runs asynchronously. Success is reported only after the account is saved.
A competing registration can claim the name before hashing finishes. The caller
must remain connected and retain Wizard authority until completion.
