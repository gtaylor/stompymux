/* access.h - Object visibility, lock, and hearing permission interfaces. */

#pragma once

#include "mux/database/db.h"

int could_doit(DbRef player, DbRef thing, int lock_number);
int can_see(DbRef player, DbRef thing, int can_see_location);
void handle_ears(DbRef thing, int could_hear, int can_hear);
