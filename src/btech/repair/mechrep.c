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

#include "mech.h"
#include "mech_build_api.h"
#include "mech_consistency_api.h"
#include "mech_restrict_api.h"
#include "mech_status_api.h"
#include "mech_utils_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/network/mux_event_alloc.h"
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

void newfreemechrep(DbRef key, void **data, int selector) {
  struct RepairFacility *new = *data;

  switch (selector) {
  case SPECIAL_ALLOC:
    new->mynum = key;
    new->current_target = -1;
    break;
  }
}

/* With cap R means restricted command */

void mechrep_Rresetcrits(DbRef player, void *data, char *buffer) {
  int i;

  MECHREP_COMMON(1);
  notify(btech_context_evaluation(rep->xcode.context), player,
         "Default criticals set!");
  for (i = 0; i < NUM_SECTIONS; i++)
    FillDefaultCriticals(mech, i);
}

void mechrep_Rdisplaysection(DbRef player, void *data, char *buffer) {
  char *args[1];
  int index;

  MECHREP_COMMON(1);
  DOCHECK_CONTEXT(rep->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "You must specify a section to list the criticals for!");
  index = ArmorSectionFromString(MechType(mech), MechMove(mech), args[0]);
  DOCHECK_CONTEXT(rep->xcode.context, index == -1, "Invalid section!");
  CriticalStatus(btech_context_evaluation(rep->xcode.context), player, mech,
                 index);
}

#define MechComputersRadioRange(mech)                                          \
  (DEFAULT_RADIORANGE * generic_radio_multiplier(mech))

void mechrep_Rsetradio(DbRef player, void *data, char *buffer) {
  char *args[2];
  int i;

  MECHREP_COMMON(1);
  switch (mech_parseattributes(buffer, args, 2)) {
  case 0:
    notify(btech_context_evaluation(rep->xcode.context), player,
           "This remains to be done [showing of stuff when no args]");
    return;
  case 2:
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Too many args, unable to cope().");
    return;
  }
  i = BOUNDED(1, atoi(args[0]), 5);
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Radio level set to %d.", i);
  MechRadio(mech) = i;
  MechRadioType(mech) = generic_radio_type(MechRadio(mech), 0);
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Number of freqs: %d  Extra stuff: %d",
                MechRadioType(mech) % 16, (MechRadioType(mech) / 16) * 16);
  MechRadioRange(mech) = MechComputersRadioRange(mech);
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Radio range set to %d.", (int)MechRadioRange(mech));
}

void mechrep_Rsettarget(DbRef player, void *data, char *buffer) {
  char *args[2];
  int newmech;

  MECHREP_COMMON(0);
  switch (mech_parseattributes(buffer, args, 2)) {
  case 1:
    newmech = match_thing(&btech_context_command(rep->xcode.context)->match,
                          player, args[0]);
    DOCHECK_CONTEXT(rep->xcode.context,
                    !(is_good_obj(rep->xcode.context->database, newmech) &&
                      is_xcode(rep->xcode.context->database, newmech)),
                    "That is not a BattleMech or Vehicle!");
    rep->current_target = newmech;
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "Mech to repair changed to #%d", newmech);
    break;
  default:
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Too many arguments!");
  }
}

void mechrep_Rsettype(DbRef player, void *data, char *buffer) {
  char *args[1];

  MECHREP_COMMON(1);
  DOCHECK_CONTEXT(rep->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "Invalid number of arguments!");
  switch (toupper(args[0][0])) {
  case 'M':
    MechType(mech) = CLASS_MECH;
    MechMove(mech) = MOVE_BIPED;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Type set to MECH");
    break;
  case 'Q':
    MechType(mech) = CLASS_MECH;
    MechMove(mech) = MOVE_QUAD;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Type set to QUAD");
    break;
  case 'G':
    MechType(mech) = CLASS_VEH_GROUND;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Type set to VEHICLE");
    break;
  case 'V':
    MechType(mech) = CLASS_VTOL;
    MechMove(mech) = MOVE_VTOL;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Type set to VTOL");
    break;
  case 'N':
    MechType(mech) = CLASS_VEH_NAVAL;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Type set to NAVAL");
    break;
  case 'A':
    MechType(mech) = CLASS_AERO;
    MechMove(mech) = MOVE_FLY;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Type set to AeroSpace");
    break;
  case 'D':
    MechType(mech) = CLASS_DS;
    MechMove(mech) = MOVE_FLY;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Type set to DropShip");
    break;
  case 'S':
    MechType(mech) = CLASS_SPHEROID_DS;
    MechMove(mech) = MOVE_FLY;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Type set to SpheroidDropship");
    break;
  case 'B':
    MechType(mech) = CLASS_BSUIT;
    MechMove(mech) = MOVE_BIPED;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Type set to BattleSuit");
    break;
  default:
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Types are: MECH, GROUND, VTOL, NAVAL, AERO, DROPSHIP and "
           "SPHEROIDDROPSHIP");
    break;
  }
}

#define SETVALUE_FUNCTION_FLOAT(funcname, valname, valstring, modifier)        \
  void funcname(DbRef player, void *data, char *buffer) {                      \
    char *args[1];                                                             \
    float f;                                                                   \
    MECHREP_COMMON(1);                                                         \
    DOCHECK_CONTEXT(                                                           \
        rep->xcode.context, mech_parseattributes(buffer, args, 1) != 1,        \
        tprintf("Invalid number of arguments to Set%s!", valstring));          \
    f = atof(args[0]);                                                         \
    valname = f * modifier;                                                    \
    notify(btech_context_evaluation(rep->xcode.context), player,               \
           tprintf("%s changed to %.2f.", valstring, valname));                \
  }

#define SETVALUE_FUNCTION_INT(funcname, valname, valstring, modifier)          \
  void funcname(DbRef player, void *data, char *buffer) {                      \
    char *args[1];                                                             \
    int f;                                                                     \
    MECHREP_COMMON(1);                                                         \
    DOCHECK_CONTEXT(                                                           \
        rep->xcode.context, mech_parseattributes(buffer, args, 1) != 1,        \
        tprintf("Invalid number of arguments to Set%s!", valstring));          \
    f = atoi(args[0]);                                                         \
    valname = f * modifier;                                                    \
    notify(btech_context_evaluation(rep->xcode.context), player,               \
           tprintf("%s changed to %d.", valstring, valname));                  \
  }

SETVALUE_FUNCTION_FLOAT(mechrep_Rsetspeed, MechMaxSpeed(mech), "Maxspeed",
                        KPH_PER_MP);
SETVALUE_FUNCTION_FLOAT(mechrep_Rsetjumpspeed, MechJumpSpeed(mech), "Jumpspeed",
                        KPH_PER_MP);
SETVALUE_FUNCTION_INT(mechrep_Rsetheatsinks, MechRealNumsinks(mech),
                      "Heatsinks", 1);
SETVALUE_FUNCTION_INT(mechrep_Rsetlrsrange, MechLRSRange(mech), "LRSrange", 1);
SETVALUE_FUNCTION_INT(mechrep_Rsettacrange, MechTacRange(mech), "TACrange", 1);
SETVALUE_FUNCTION_INT(mechrep_Rsetscanrange, MechScanRange(mech), "SCANrange",
                      1);
SETVALUE_FUNCTION_INT(mechrep_Rsetradiorange, MechRadioRange(mech),
                      "RADIOrange", 1);
SETVALUE_FUNCTION_INT(mechrep_Rsettons, MechTons(mech), "Tons", 1);

void mechrep_Rsetmove(DbRef player, void *data, char *buffer) {
  char *args[1];

  MECHREP_COMMON(1);
  DOCHECK_CONTEXT(rep->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "Invalid number of arguments!");
  switch (toupper(args[0][0])) {
  case 'T':
    MechMove(mech) = MOVE_TRACK;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Movement set to TRACKED");
    break;
  case 'W':
    MechMove(mech) = MOVE_WHEEL;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Movement set to WHEELED");
    break;
  case 'H':
    switch (toupper(args[0][1])) {
    case 'O':
      MechMove(mech) = MOVE_HOVER;
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Movement set to HOVER");
      break;
    case 'U':
      MechMove(mech) = MOVE_HULL;
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Movement set to HULL");
      break;
    }
    break;
  case 'V':
    MechMove(mech) = MOVE_VTOL;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Movement set to VTOL");
    break;
  case 'Q':
    MechMove(mech) = MOVE_QUAD;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Movement set to QUAD");
    break;
  case 'B':
    MechMove(mech) = MOVE_BIPED;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Movement set to BIPED");
    break;
  case 'S':
    MechMove(mech) = MOVE_SUB;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Movement set to SUB");
    break;
  case 'F':
    switch (toupper(args[0][1])) {
    case 'O':
      MechMove(mech) = MOVE_FOIL;
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Movement set to FOIL");
      break;
    case 'L':
      MechMove(mech) = MOVE_FLY;
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Movement set to FLY");
      break;
    }
    break;
  case 'N':
    MechMove(mech) = MOVE_NONE;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Movement set to NONE");
    break;
  default:
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Types are: TRACK, WHEEL, VTOL, QUAD, BIPED, HOVER, HULL, "
           "FLY, SUB, FOIL and NONE");
    break;
  }
}
