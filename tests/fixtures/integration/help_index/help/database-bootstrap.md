+++
title = "Database bootstrap"
keywords = ["database bootstrap", "new database", "initial password"]
article_tags = ["wizard_commands"]
description = "Explain automatic creation of a new game database"
wizard_only = true
+++

# Database bootstrap

When the configured SQLite game database does not exist, server startup creates
the objects declared in `[database.bootstrap.objects]` and saves them before
accepting connections. Existing database files are never replaced by this
process.

The stock seed creates `#1` GOD and `#2` Wizard in `#4` Starter Room. Both
player entries explicitly set `wizard = true`; an omitted setting defaults to
false. The server refuses to start unless `#1` is a player with Wizard
privileges. Their distinct generated passwords are printed to the server
console once. Only the password hashes are stored, so change or securely retain
the printed passwords.
