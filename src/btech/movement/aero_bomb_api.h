
#pragma once

#include "mux/server/platform.h"

int bomb_weight(int i);
const char *bomb_name(int i);
void mech_bomb(DbRef player, Mech *mech, char *buffer);
