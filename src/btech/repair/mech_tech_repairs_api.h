/* Declares the BattleTech unit tech repairs API. */

#pragma once

#include "mux/server/platform.h"

/* mech.tech.repairs.c */
void tech_repairs(DbRef player, Mech *mech, char *buffer);
