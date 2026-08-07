#pragma once
/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *  Copyright (c) 2005-2006 Gregory Taylor
 *       All rights reserved
 */

#include <ctype.h>
#include <math.h>
#include <stdio.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_bth_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_hitloc_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_physical.h"
#include "mech_physical_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

int have_punch(Mech *mech, int location);
int phys_common_checks(Mech *mech);
int get_arm_args(int *using, int *argument_count, char ***arguments, Mech *mech,
                 int (*has_weapon)(Mech *mech, int location), char *weapon);

void physical_damage_apply(Mech *target, Mech *attacker, int cause_pilot,
                           DbRef pilot, int hit_location, int rear,
                           int critical, int damage, int glancing);
void physical_damage_apply_without_experience(Mech *target, Mech *attacker,
                                              int cause_pilot, DbRef pilot,
                                              int hit_location, int rear,
                                              int critical, int damage,
                                              int glancing);

enum { CHARGE_SECTIONS = 6 };

extern const int resect[CHARGE_SECTIONS];

/**
 * Checks to see if all limbs have recycled from any previous physical attacks.
 */
