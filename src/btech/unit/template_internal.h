#include "btech_event.h" // IWYU pragma: keep
#include "legacy_macros.h"
#include "map.h" // IWYU pragma: keep
#include "map_terrain.h"
#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 *  Copyright (c) 1999-2005 Kevin Stevens
 *	All rights reserved
 */

/*
   Code to read and write mech and vehicle templates
   Created by Nim 9/16/96

   $Id: template.c,v 1.9 2005/08/10 14:09:34 av1-op Exp $
   Last modified: Fri Sep 18 13:02:31 1998 fingon
 */

/* 01/21/02 Added many commods <KM> */

/* 09/16/96 Some touches by Markus Stenberg <fingon@iki.fi> */

/* 09/16/96 Some?? ... ya right ;-) (nim)                  */

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "mech_lifecycle.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "weapon_settings.h"

/* 09/17/96 Ok, ton of touches then :-P (Mark) */

#include "mux/server/platform.h"

#define MAX_STRING_LENGTH 8192
#include <ctype.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>

#include "aero_bomb_api.h"
#include "bsuit_api.h"
#include "coolmenu.h"
#include "map_conditions_api.h"
#include "mech.h"
#include "mech_c3_api.h"
#include "mech_consistency_api.h"
#include "mech_mechref_ident_api.h"
#include "mech_partnames_api.h"
#include "mech_utils_api.h"
#include "template_api.h"

#define MODE_UNKNOWN 0
#define MODE_NORMAL 1

extern char *load_cmds[];
extern char *internals[];
extern char *cargo[];
extern char *section_configs[];
extern char *move_types[];
extern char *mech_types[];
extern char *crit_fire_modes[];
extern char *crit_ammo_modes[];
extern char *specials[];
extern char *specialsabrev[];
extern char *specials2[];
extern char *specialsabrev2[];
extern char *infantry_specials[];
extern char *infspecialsabrev[];
extern const int num_def_weapons;
extern const int template_internal_count;
extern const int template_cargo_count;

#define TCAble(t)                                                              \
  ((MechWeapons[Weapon2I(t)].type == TBEAM ||                                  \
    MechWeapons[Weapon2I(t)].type == TAMMO) &&                                 \
   strcmp(&MechWeapons[Weapon2I(t)].name[3], "Flamer") &&                      \
   strcmp(&MechWeapons[Weapon2I(t)].name[3], "MachineGun") &&                  \
   strcmp(&MechWeapons[Weapon2I(t)].name[3], "LightMachineGun") &&             \
   strcmp(&MechWeapons[Weapon2I(t)].name[3], "HeavyMachineGun") &&             \
   !(MechWeapons[Weapon2I(t)].special & PCOMBAT))

#define MechComputersScanRange(mech)                                           \
  (generic_computer_multiplier(mech) * DEFAULT_SCANRANGE)
#define MechComputersLRSRange(mech)                                            \
  (generic_computer_multiplier(mech) * DEFAULT_LRSRANGE)
#define MechComputersTacRange(mech)                                            \
  (generic_computer_multiplier(mech) * DEFAULT_TACRANGE)
#define MechComputersRadioRange(mech)                                          \
  (DEFAULT_RADIORANGE * generic_radio_multiplier(mech))
