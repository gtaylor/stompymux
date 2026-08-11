/* Declares the BattleTech unit lite API. */

#pragma once

#include "mux/server/platform.h"

/* mech.lite.c */
void cause_lite(Mech *mech, Mech *temp_mech);
void end_lite_check(Mech *mech);
