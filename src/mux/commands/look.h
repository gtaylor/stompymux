/* look.h - Object look and inventory display helper interface. */

#pragma once

#include "mux/database/db.h"

void look_in(DbRef player, DbRef location, int key);
