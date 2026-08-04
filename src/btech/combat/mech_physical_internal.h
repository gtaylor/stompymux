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
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_bth_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_hitloc_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical.h"
#include "mech_physical_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

// Only allows arm physical attacks for CLASS_MECH.
#define ARM_PHYS_CHECK(a)                                                      \
  DOCHECK_CONTEXT(mech->xcode.context,                                         \
                  MechType(mech) == CLASS_MW || MechType(mech) == CLASS_BSUIT, \
                  tprintf("You cannot %s without a 'mech!", a));               \
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) != CLASS_MECH,           \
                  tprintf("You cannot %s with this vehicle!", a));

// Checks a unit's legs for kicking.
#define GENERIC_CHECK(a, wDeadLegs)                                            \
  ARM_PHYS_CHECK(a);                                                           \
  DOCHECK_CONTEXT(mech->xcode.context, !MechIsQuad(mech) && (wDeadLegs > 1),   \
                  "Without legs? Are you kidding?");                           \
  DOCHECK_CONTEXT(mech->xcode.context, !MechIsQuad(mech) && (wDeadLegs > 0),   \
                  "With one leg? Are you kidding?");                           \
  DOCHECK_CONTEXT(mech->xcode.context, wDeadLegs > 1,                          \
                  "It'd unbalance you too much in your condition..");          \
  DOCHECK_CONTEXT(mech->xcode.context, wDeadLegs > 2,                          \
                  "Exactly _what_ are you going to kick with?");

// If it's a quad, we can't play with sharp things (Swords, Axes, etc.)
#define QUAD_CHECK(a)                                                          \
  DOCHECK_CONTEXT(                                                             \
      mech->xcode.context, MechType(mech) == CLASS_MECH && MechIsQuad(mech),   \
      tprintf("What are you going to %s with, your front right leg?", a))

int have_punch(Mech *mech, int location);
int phys_common_checks(Mech *mech);
int get_arm_args(int *using, int *argument_count, char ***arguments, Mech *mech,
                 int (*has_weapon)(Mech *mech, int location), char *weapon);

#define MyDamageMech(a, b, c, d, e, f, g, h, i)                                \
  (a)->xcode.context->combat_overrides.damage_experience =                     \
      BTECH_DAMAGE_XP_PILOTING;                                                \
  DamageMech(a, b, c, d, e, f, g, h, i, -1, 0, -1, 0, 0);                      \
  (a)->xcode.context->combat_overrides.damage_experience =                     \
      BTECH_DAMAGE_XP_GUNNERY
#define MyDamageMech2(a, b, c, d, e, f, g, h, i)                               \
  (a)->xcode.context->combat_overrides.damage_experience =                     \
      BTECH_DAMAGE_XP_NONE;                                                    \
  DamageMech(a, b, c, d, e, f, g, h, i, -1, 0, -1, 0, 0);                      \
  (a)->xcode.context->combat_overrides.damage_experience =                     \
      BTECH_DAMAGE_XP_GUNNERY

enum { CHARGE_SECTIONS = 6 };

extern const int resect[CHARGE_SECTIONS];

/**
 * Checks to see if all limbs have recycled from any previous physical attacks.
 */
