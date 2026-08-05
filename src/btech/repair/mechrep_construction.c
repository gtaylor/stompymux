/*
 * Last modified: Thu Aug 13 23:41:12 1998 fingon
 * Copyright (c) 1999-2005 Kevin Stevens
 *   All right reserved
 */

#include <ctype.h>
#include <dirent.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/stat.h>
#include <unistd.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map_terrain.h" // IWYU pragma: keep
#include "mech_events.h"
#include "mech_lifecycle.h" // IWYU pragma: keep
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#define MECH_STAT_C /* want to use the POSIX stat() call. */

#include "equipment_types.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_consistency_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_restrict_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/network/mux_event_alloc.h"
#include "section_types.h"
#include "template_api.h"

/* Selectors */
#define SPECIAL_FREE 0
#define SPECIAL_ALLOC 1

extern char *strtok(char *s, const char *ct);

#define MECHREP_COMMON(a)                                                      \
  struct RepairFacility *rep = (struct RepairFacility *)data;                  \
  Mech *mech;                                                                  \
  DOCHECK_CONTEXT(rep->xcode.context,                                          \
                  !is_god(rep->xcode.context->database, player) &&             \
                      !is_wizard(rep->xcode.context->database, player),        \
                  "I'm sorry Dave, can't do that.");                           \
  if (a) {                                                                     \
    DOCHECK_CONTEXT(rep->xcode.context, rep->current_target == -1,             \
                    "You must set a target first!");                           \
    mech = btech_context_get_mech(rep->xcode.context, rep->current_target);    \
    DOCHECK_CONTEXT(rep->xcode.context, mech == nullptr,                       \
                    "The target's BTech data is not allocated.");              \
  }

/*--------------------------------------------------------------------------*/

/* Code Begins                                                              */

/*--------------------------------------------------------------------------*/

/* Alloc free function */

/* Alloc/free routine */

void invalid_section(DbRef player, Mech *mech) {
  int mechtype = mech_class(mech);
  int movetype = mech_movement_type(mech);

  notify(btech_context_evaluation(mech_context(mech)), player,
         "Not a legal armor location, must be one of:");

  switch (mechtype) {
  case CLASS_MW:
  case CLASS_MECH:
    notify(btech_context_evaluation(mech_context(mech)), player,
           "HEAD (H), CTORSO (CT), LTORSO (LT), RTORSO (RT)");

    if (movetype == MOVE_QUAD)
      notify(btech_context_evaluation(mech_context(mech)), player,
             "LARM (LA), RARM (RA), LLEG (LL), RLEG (RL)");
    else
      notify(btech_context_evaluation(mech_context(mech)), player,
             "FLLEG (FLL), FRLEG (FRL), RLLEG (RLL), RRLEG (RRL)");

    break;
  case CLASS_VEH_NAVAL:
  case CLASS_VEH_GROUND:
    notify(btech_context_evaluation(mech_context(mech)), player,
           "FSIDE (FS), RSIDE (RS), LSIDE (LS), ASIDE (AS), TURRET (TU)");
    break;
  case CLASS_VTOL:
    notify(btech_context_evaluation(mech_context(mech)), player,
           "FSIDE (FS), RSIDE (RS), LSIDE (LS), ASIDE (AS), ROTOR (RO)");
    break;
  case CLASS_AERO:
    notify(btech_context_evaluation(mech_context(mech)), player,
           "NOSE (N), LWING (LW), RWING (RW), ASIDE (AS)");
    break;
  case CLASS_DS:
    notify(btech_context_evaluation(mech_context(mech)), player,
           "NOSE (N), LWING (LW), RWING (RW), LRWING (LR), RRWING "
           "(RR), ASIDE (AS)");
    break;
  case CLASS_SPHEROID_DS:
    notify(btech_context_evaluation(mech_context(mech)), player,
           "NOSE (N), FRSIDE (FR), FLSIDE (FL), RLSIDE (RL), RRSIDE "
           "(RR), ASIDE (AS)");
    break;
  case CLASS_BSUIT:
    notify(btech_context_evaluation(mech_context(mech)), player,
           "S1, S2, S3, S4, S5, S6, S7, S8");
    break;
  default:
    notify(btech_context_evaluation(mech_context(mech)), player,
           "Invalid or unknown unit type!");
  }
}

/*
 * Logic for the 'setarmor' mechrep command.
 */
void mechrep_Rsetarmor(DbRef player, void *data, char *buffer) {
  char *args[4];
  int argc;
  int index;
  int temp;

  MECHREP_COMMON(1);
  argc = mech_parseattributes(buffer, args, 4);
  DOCHECK_CONTEXT(rep->xcode.context, !argc, "Invalid number of arguments!");
  index = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                 args[0]);

  if (index == -1) {
    // Invalid section, emit error and valid choices for unit type.
    invalid_section(player, mech);
    return;
  }

  argc--;

  if (argc) {
    // One Argument Given.
    temp = atoi(args[1]);
    if (temp < 0)
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Invalid armor value!");
    else {
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "Front armor set to    : %d", temp);
      mech_section_armor_set(mech, index, temp);
      mech_section_original_armor_set(mech, index, temp);
    }
    argc--;
  }
  if (argc) {
    // Two Arguments Given.
    temp = atoi(args[2]);
    if (temp < 0)
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Invalid Internal armor value!");
    else {
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "Internal armor set to : %d", temp);
      mech_section_internal_set(mech, index, temp);
      mech_section_original_internal_set(mech, index, temp);
    }
    argc--;
  }
  if (argc) {
    // Three Arguments Given.
    temp = atoi(args[3]);
    if (index == CTORSO || index == RTORSO || index == LTORSO) {
      if (temp < 0)
        notify(btech_context_evaluation(rep->xcode.context), player,
               "Invalid Rear armor value!");
      else {
        notify_printf(btech_context_evaluation(rep->xcode.context), player,
                      "Rear armor set to     : %d", temp);
        mech_section_rear_armor_set(mech, index, temp);
        mech_section_original_rear_armor_set(mech, index, temp);
      }
    } else
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Only the torso can have rear armor.");
  }
}

/*
 * Handles the adding of weapons via the 'addweap' command in the form of:
 * addweap <weap> <loc> <crits> [<flags>]
 * Current Flags: O = OS, T = TC, R = Rear
 */
void mechrep_Raddweap(DbRef player, void *data, char *buffer) {
  char *args[20];    /* The argument array */
  int argc;          /* Count of arguments */
  int index;         /* Used to determine section validity */
  int weapindex;     /* Weapon index number */
  int weapnumcrits;  /* Number of crits the desired weapon occupies. */
  int loop, temp;    /* Loop Counters */
  int isrear = 0;    /* Rear mounted? */
  int istc = 0;      /* Is the weap TC'd? */
  int isoneshot = 0; /* If 1, weapon is a One-Shot (OS) Weap */
  int argstoiter;    /* Holder for figuring out how many args to scan */
  char flagholder;   /* Holder for flag comparisons */

  MECHREP_COMMON(1);

  argc = mech_parseattributes(buffer, args, 20);
  DOCHECK_CONTEXT(rep->xcode.context, argc < 3, "Invalid number of arguments!")

  index = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                 args[1]);

  if (index == -1) {
    // Invalid section entered. Emit error and valid sections.
    invalid_section(player, mech);
    return;
  }

  weapindex = WeaponIndexFromString(rep->xcode.context, args[0]);

  if (weapindex == -1) {
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "That is not a valid weapon!");
    DumpWeapons(mech_context(mech), player);
    return;
  }

  /*
   * There are always 3 arguments that preceed flags.
   * addweap <weap> <loc> <crit>, 0, 1, and 2 respectively in args[][].
   * By subtracting 3, we figure out how many of our arguments are actually
   * flags.
   */
  argstoiter = argc - 3;

  /*
   * Now we take those additional flags and look for matches. argc is
   * decremented to keep track of how many of our arguments are crit
   * locations.
   */
  for (loop = 0; loop < argstoiter; loop++) {
    flagholder = toupper(args[3 + loop][0]);

    if (flagholder == 'T') {
      /* Targeting Computer */
      istc = 1;
    } else if (flagholder == 'R') {
      /* Rear Mounted */
      isrear = 1;
    } else if (flagholder == 'O') {
      /* One-Shot */
      isoneshot = 1;
    }

    /*
     * If it's a letter, it's not a crit location. If a
     * player throws numbers in with the crit flags, then
     * they'll see error messages about crit counts. Need
     * to find a better way to fool-proof this.
     */
    if (isalpha(flagholder))
      argc--;

  } /* end for */

  /* Chop off the first the first two redundant args. */
  argc -= 2;

  weapnumcrits = GetWeaponCrits(mech, weapindex);

  // Add < 9 for split weap help
  /* Check to see if player gives enough crits and start adding if so. */
  if (argc < weapnumcrits && weapnumcrits < 9) {
    notify_printf(
        btech_context_evaluation(rep->xcode.context), player,
        "Not enough critical slots specified! (Given: %i, Needed: %i)", argc,
        weapnumcrits);
  } else if (argc > weapnumcrits) {
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "Too many critical slots specified! (Given: %i, Needed: %i)",
                  argc, weapnumcrits);
  } else {
    if (argc < weapnumcrits) // notify player of split crit
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "Weapon will be split! %d additional crits needed.",
                    weapnumcrits - argc);
    for (loop = 0; loop < argc; loop++) {
      temp = atoi(args[2 + loop]);
      temp--; /* From 1 based to 0 based */
      DOCHECK_CONTEXT(rep->xcode.context, temp < 0 || temp > NUM_CRITICALS,
                      "Bad critical location!");
      int fire_mode = 0;

      /* If this is a Rocket Launcher, use isrocket to set the OS flag */
      //                      if(MechWeapons[weapindex].special & ROCKET)
      //                              isrocket = 1;

      if (isrear)
        fire_mode |= REAR_MOUNT;
      if (istc)
        fire_mode |= ON_TC;
      /* Rockets are OS too */ // NOT! -=RST
      if (isoneshot)
        fire_mode |= OS_MODE;
      mech_critical_configure(mech, index, temp, I2Weapon(weapindex), 0,
                              fire_mode, 0);
    }
    if (MechWeapons[weapindex].special & AMS) {
      if (MechWeapons[weapindex].special & CLAT)
        mech_technology_flags_add(mech, CL_ANTI_MISSILE_TECH);
      else
        mech_technology_flags_add(mech, IS_ANTI_MISSILE_TECH);
    }
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "Weapon added.");
  }
} /* end mechrep_Raddweap() */

void mechrep_Rfiremode(DbRef player, void *data, char *buffer) {
  char *args[4];
  int argc;
  int section, critical, weaptype;

  MECHREP_COMMON(1);
  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(rep->xcode.context, argc < 2,
                  "MECHREP: Invalid Syntax. Try FireMode <Weapon#> <Mode>");

  weaptype = FindWeaponNumberOnMech_Advanced(mech, atoi(args[0]), &section,
                                             &critical, 0);

  if (weaptype < 0) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Invalid Weapon #!");
    return;
  }

  if (MechWeapons[weaptype].ammoperton == 0) {
    notify(btech_context_evaluation(mech_context(mech)), player,
           "That weapon doesn't require ammo!");
    return;
  }

  if (mech_critical_fire_mode(mech, section, critical) & OS_MODE) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Keeping One Shot Mode!");
    mech_critical_ammo_mode_set(mech, section, critical, 0);
  } else if (!(mech_critical_fire_mode(mech, section, critical) &
               HALFTON_MODE)) {

    mech_critical_fire_mode_set(mech, section, critical, 0);
    mech_critical_ammo_mode_set(mech, section, critical, 0);
  }

  switch (toupper(args[1][0])) {
  case 'W':
    mech_critical_ammo_mode_add(mech, section, critical, SWARM_MODE);
    break;
  case '#':
    mech_critical_ammo_mode_add(mech, section, critical, MML_LRM_MODE);
    break;
  case '1':
    mech_critical_ammo_mode_add(mech, section, critical, SWARM1_MODE);
    break;
  case 'I':
    mech_critical_ammo_mode_add(mech, section, critical, INFERNO_MODE);
    break;
  case 'L':
    mech_critical_ammo_mode_add(mech, section, critical, LBX_MODE);
    break;
  case 'A':
    mech_critical_ammo_mode_add(mech, section, critical, ARTEMIS_MODE);
    break;
  case 'N':
    mech_critical_ammo_mode_add(mech, section, critical, NARC_MODE);
    break;
  case 'C':
    mech_critical_ammo_mode_add(mech, section, critical, CLUSTER_MODE);
    break;
  case 'M':
    mech_critical_ammo_mode_add(mech, section, critical, MINE_MODE);
    break;
  case 'S':
    mech_critical_ammo_mode_add(mech, section, critical, SMOKE_MODE);
    break;
  case 'X':
    mech_critical_ammo_mode_add(mech, section, critical, INARC_EXPLO_MODE);
    break;
  case 'Y':
    mech_critical_ammo_mode_add(mech, section, critical, INARC_HAYWIRE_MODE);
    break;
  case 'E':
    mech_critical_ammo_mode_add(mech, section, critical, INARC_ECM_MODE);
    break;
  case 'R':
    mech_critical_ammo_mode_add(mech, section, critical, AC_AP_MODE);
    break;
  case 'F':
    mech_critical_ammo_mode_add(mech, section, critical, AC_FLECHETTE_MODE);
    break;
  case 'D':
    mech_critical_ammo_mode_add(mech, section, critical, AC_INCENDIARY_MODE);
    break;
  case 'P':
    mech_critical_ammo_mode_add(mech, section, critical, AC_PRECISION_MODE);
    break;
  case 'T':
    mech_critical_ammo_mode_add(mech, section, critical, STINGER_MODE);
    break;
  case 'U':
    mech_critical_ammo_mode_add(mech, section, critical, AC_CASELESS_MODE);
    break;
  case 'J':
    mech_critical_fire_mode_add(mech, section, critical, WILL_JETTISON_MODE);
    break;
  case 'G':
    mech_critical_ammo_mode_add(mech, section, critical, SGUIDED_MODE);
    break;
  case 'H':
    mech_critical_ammo_mode_add(mech, section, critical, ATM_HE_MODE);
    break;
  case 'V':
    mech_critical_ammo_mode_add(mech, section, critical, ATM_ER_MODE);
    break;
  case '-':
    mech_critical_ammo_mode_set(mech, section, critical, 0);
    mech_critical_fire_mode_set(mech, section, critical, 0);
  }

  notify(btech_context_evaluation(rep->xcode.context), player,
         "Firemode changed!");
}
/*
 * Logic for the 'reload' mechrep command.
 */
void mechrep_Rreload(DbRef player, void *data, char *buffer) {
  char *args[4];
  int argc;
  int index;
  int weapindex;
  int subsect;

  MECHREP_COMMON(1);
  argc = mech_parseattributes(buffer, args, 4);
  DOCHECK_CONTEXT(rep->xcode.context, argc <= 2,
                  "Invalid number of arguments!");
  weapindex = WeaponIndexFromString(rep->xcode.context, args[0]);

  if (weapindex == -1) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "That is not a valid weapon!");
    DumpWeapons(mech_context(mech), player);
    return;
  }

  index = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                 args[1]);

  if (index == -1) {
    // Invalid section entered. Emit error and valid sections.
    invalid_section(player, mech);
    return;
  }

  subsect = atoi(args[2]);
  subsect--; /* from 1 based to 0 based */
  DOCHECK_CONTEXT(rep->xcode.context,
                  subsect < 0 || subsect >= CritsInLoc(mech, index),
                  "Critslot out of range!");
  if (MechWeapons[weapindex].ammoperton == 0)
    notify(btech_context_evaluation(mech_context(mech)), player,
           "That weapon doesn't require ammo!");
  else {
    mech_critical_part_type_set(mech, index, subsect, I2Ammo(weapindex));
    if (!(mech_critical_fire_mode(mech, index, subsect) & HALFTON_MODE)) {
      mech_critical_fire_mode_set(mech, index, subsect, 0);
      mech_critical_ammo_mode_set(mech, index, subsect, 0);
    }

    if (argc > 3)
      switch (toupper(args[3][0])) {
      case '+':
        mech_critical_fire_mode_add(mech, index, subsect, HALFTON_MODE);
        break;
      case '#':
        mech_critical_ammo_mode_add(mech, index, subsect, MML_LRM_MODE);
        break;
      case 'W':
        mech_critical_ammo_mode_add(mech, index, subsect, SWARM_MODE);
        break;
      case '1':
        mech_critical_ammo_mode_add(mech, index, subsect, SWARM1_MODE);
        break;
      case 'I':
        mech_critical_ammo_mode_add(mech, index, subsect, INFERNO_MODE);
        break;
      case 'L':
        mech_critical_ammo_mode_add(mech, index, subsect, LBX_MODE);
        break;
      case 'A':
        mech_critical_ammo_mode_add(mech, index, subsect, ARTEMIS_MODE);
        break;
      case 'N':
        mech_critical_ammo_mode_add(mech, index, subsect, NARC_MODE);
        break;
      case 'C':
        mech_critical_ammo_mode_add(mech, index, subsect, CLUSTER_MODE);
        break;
      case 'M':
        mech_critical_ammo_mode_add(mech, index, subsect, MINE_MODE);
        break;
      case 'S':
        mech_critical_ammo_mode_add(mech, index, subsect, SMOKE_MODE);
        break;
      case 'Z':
        mech_critical_ammo_mode_add(mech, index, subsect, INARC_NEMESIS_MODE);
        break;
      case 'X':
        mech_critical_ammo_mode_add(mech, index, subsect, INARC_EXPLO_MODE);
        break;
      case 'Y':
        mech_critical_ammo_mode_add(mech, index, subsect, INARC_HAYWIRE_MODE);
        break;
      case 'E':
        mech_critical_ammo_mode_add(mech, index, subsect, INARC_ECM_MODE);
        break;
      case 'R':
        mech_critical_ammo_mode_add(mech, index, subsect, AC_AP_MODE);
        break;
      case 'F':
        mech_critical_ammo_mode_add(mech, index, subsect, AC_FLECHETTE_MODE);
        break;
      case 'D':
        mech_critical_ammo_mode_add(mech, index, subsect, AC_INCENDIARY_MODE);
        break;
      case 'P':
        mech_critical_ammo_mode_add(mech, index, subsect, AC_PRECISION_MODE);
        break;
      case 'T':
        mech_critical_ammo_mode_add(mech, index, subsect, STINGER_MODE);
        break;
      case 'U':
        mech_critical_ammo_mode_add(mech, index, subsect, AC_CASELESS_MODE);
        break;
      case 'G':
        mech_critical_ammo_mode_add(mech, index, subsect, SGUIDED_MODE);
        break;
      case 'H':
        mech_critical_ammo_mode_add(mech, index, subsect, ATM_HE_MODE);
        break;
      case 'V':
        mech_critical_ammo_mode_add(mech, index, subsect, ATM_ER_MODE);
        break;
      case '-':
        mech_critical_ammo_mode_set(mech, index, subsect, 0);
        mech_critical_fire_mode_set(mech, index, subsect, 0);
      }

    mech_critical_data_set(mech, index, subsect,
                           FullAmmo(mech, index, subsect));
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Weapon loaded!");
  }
}

/*
 * Logic for the 'restock' mechrep command.
 */
void mechrep_Rrestock(DbRef player, void *data, char *buffer) {
  char *args[2];
  int argc;
  int index;
  int subsect;

  MECHREP_COMMON(1);
  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(rep->xcode.context, argc < 2, "Invalid number of arguments!");

  index = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                 args[0]);

  if (index == -1) {
    // Invalid section entered. Emit error and valid sections.
    invalid_section(player, mech);
    return;
  }

  subsect = atoi(args[1]);
  subsect--; /* from 1 based to 0 based */
  DOCHECK_CONTEXT(rep->xcode.context,
                  subsect < 0 || subsect >= CritsInLoc(mech, index),
                  "Critslot out of range!");
  if (MechWeapons[Ammo2I(mech_critical_part_type(mech, index, subsect))]
          .ammoperton == 0)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "That weapon doesn't require ammo!");
  else {
    mech_critical_data_set(mech, index, subsect,
                           FullAmmo(mech, index, subsect));
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Weapon restocked!");
  }
}

/*
 * Logic for the 'repair' mechrep command.
 */
void mechrep_Rrepair(DbRef player, void *data, char *buffer) {
  char *args[4];
  int argc;
  int index;
  int temp = 0;

  MECHREP_COMMON(1);
  argc = mech_parseattributes(buffer, args, 4);
  DOCHECK_CONTEXT(rep->xcode.context, argc < 2, "Invalid number of arguments!");
  index = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                 args[0]);

  if (index == -1) {
    // Invalid section entered. Emit error and valid sections.
    invalid_section(player, mech);
    return;
  }
  if (argc > 2) {
    temp = atoi(args[2]);
    DOCHECK_CONTEXT(rep->xcode.context, temp < 0, "Illegal value for armor!");
  }

  switch (args[1][0]) {
  case 'A':
  case 'a':
    /* armor */
    mech_section_armor_set(mech, index, temp);
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Armor repaired!");
    break;
  case 'I':
  case 'i':
    /* internal */
    mech_section_internal_set(mech, index, temp);
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Internal structure repaired!");
    break;
  case 'C':
  case 'c':
    /* criticals */
    temp--;
    if (temp >= 0 && temp < NUM_CRITICALS) {
      mech_RepairPart(mech, index, temp);
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Critical location repaired!");
    } else {
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Critical Location out of range!");
    }
    break;
  case 'R':
  case 'r':
    /* rear */
    if (index == CTORSO || index == LTORSO || index == RTORSO) {
      mech_section_rear_armor_set(mech, index, temp);
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Rear armor repaired!");
    } else {
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Only the center, rear and left torso have rear armor!");
    }
    break;
  case 'S':
  case 's':
    /* reattach */
    mech_ReAttach(mech, index);
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Section reattached.");
    break;
  default:
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Illegal Type-> must be ARMOR, INTERNAL, CRIT, REAR");
    return;
  }
}

/*
   ADDSP <ITEM> <LOCATION> <SUBSECT> [<DATA>]
 */
