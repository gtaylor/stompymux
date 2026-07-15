/* speech.h - Player speech and private-message command declarations. */

#pragma once

#include "mux/database/db.h"

void do_pemit_list(DbRef player, char *list, const char *message);
