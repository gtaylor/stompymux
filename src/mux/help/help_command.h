/* help_command.h - `help` and `@helpreload` command handlers. */

#pragma once

#include "mux/database/db.h"

void do_help(DbRef player, DbRef cause, int key, char *message);
void do_helpreload(DbRef player, DbRef cause, int key);
