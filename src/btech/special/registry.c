#include "btech_event.h" // IWYU pragma: keep
#include "map.h"         // IWYU pragma: keep
#include "map_api.h"
#include "map_terrain.h"
#include "mech_parts.h"               // IWYU pragma: keep
#include "mech_scan_api.h"            // IWYU pragma: keep
#include "mech_status_api.h"          // IWYU pragma: keep
#include "mux/server/runtime_clock.h" // IWYU pragma: keep

/*
 * $Id: registry.c,v 1.4 2005/08/08 09:43:09 murrayma Exp $
 *
 * Original author: unknown
 *
 * Copyright (c) 1996-2002 Markus Stenberg
 * Copyright (c) 1998-2002 Thomas Wouters
 * Copyright (c) 2000-2002 Cord Awtry
 *
 * Last modified: Thu Jul  9 02:40:16 1998 fingon
 *
 * This includes the basic code to allow objects to have hardcoded
 * commands / properties.
 *
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "ds_turret_api.h"
#include "legacy_macros.h"
#include "map_dynamic_api.h"
#include "mech_lifecycle.h"
#include "mech_restrict_api.h"
#include "mech_template_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/network/mux_event.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"
#include "part_cost_api.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_settings.h"

#define FAST_WHICHSPECIAL

#define _GLUE_C

/*** #include all the prototype here! ****/
#include "autopilot.h"
#include "btech/persistence.h"
#include "coolmenu.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_partnames_api.h"
#include "mech_stat_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/objects/powers.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "mycool.h"
#include "registry_internal.h"
#include "section_types.h"
#include "turret.h"

/* Special object parameters.  */
const BtechSpecialObjectDefinition SpecialObjects[BTECH_SPECIAL_OBJECT_COUNT] =
    {{"MECH", mechcommands, 0, mech_storage_size, newfreemech, HEAT_TICK,
      mech_update, POWER_NONE},
     {"DEBUG", debugcommands, sizeof(BtechSpecialObject), nullptr, nullptr, 0,
      nullptr, POWER_NONE},
     {"MECHREP", mechrepcommands, sizeof(RepairFacility), nullptr,
      newfreemechrep, 0, nullptr, POWER_NONE},
     {"MAP", mapcommands, sizeof(BattleMap), nullptr, newfreemap, LOS_TICK,
      map_update, POWER_NONE},
     {"AUTOPILOT", autopilotcommands, sizeof(Autopilot), nullptr,
      auto_newautopilot, 0, nullptr, POWER_NONE},
     {"TURRET", turretcommands, sizeof(Turret), nullptr,
      turret_lifecycle_update, 0, nullptr, POWER_NONE}};

int btech_special_object_type_count(void) { return BTECH_SPECIAL_OBJECT_COUNT; }

const char *btech_special_object_type_name(int type) {
  if (type < 0 || type >= BTECH_SPECIAL_OBJECT_COUNT)
    return "Unknown";
  return SpecialObjects[type].type;
}

size_t btech_special_object_storage_size(int type) {
  if (type < 0 || type >= BTECH_SPECIAL_OBJECT_COUNT)
    return 0;
  return btech_special_object_data_size(&SpecialObjects[type]);
}

#define NUM_SPECIAL_OBJECTS BTECH_SPECIAL_OBJECT_COUNT

/* Prototypes */

/*************CALLABLE PROTOS*****************/

/* Main entry point */
bool btech_command_try_execute(BtechContext *context, DbRef player, DbRef loc,
                               char *command);

/* Called when a user creates or removes the XCODE flag. */
void CreateNewSpecialObject(BtechContext *context, DbRef player, DbRef key);
void btech_special_object_dispose(BtechContext *context, DbRef player,
                                  DbRef key);
void list_hashstat(DbRef player, const char *tab_name, HashTable *htab);
void raw_notify(EvaluationContext *evaluation, DbRef player, const char *msg);

/*************PERSONAL PROTOS*****************/
void *NewSpecialObject(BtechContext *context, long id, int type);
void *btech_context_find_object(BtechContext *context, DbRef key);
int btech_context_which_special(BtechContext *context, DbRef key);
int btech_context_which_special_attribute(BtechContext *context, DbRef key);

static int compare_dbrefs(void *key1, void *key2, void *token) {
  const DbRef key1_val = (DbRef)key1;
  const DbRef key2_val = (DbRef)key2;

  return key1_val - key2_val;
}

void btech_registry_tree_initialize(BtechContext *context) {
  context->special_objects = red_black_tree_init(compare_dbrefs, nullptr);
  if (!context->special_objects) {
    /* TODO: We could handle this more gracefully... */
    exit(EXIT_FAILURE);
  }
}

/*********************************************/

static int mech_class_command_flag(int unit_class) {
  switch (unit_class) {
  case CLASS_MECH:
    return GFLAG_MECH;
  case CLASS_VEH_GROUND:
    return GFLAG_GROUNDVEH;
  case CLASS_AERO:
    return GFLAG_AERO;
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    return GFLAG_DS;
  case CLASS_VTOL:
    return GFLAG_VTOL;
  case CLASS_VEH_NAVAL:
    return GFLAG_NAVAL;
  case CLASS_BSUIT:
    return GFLAG_BSUIT;
  case CLASS_MW:
    return GFLAG_MW;
  default:
    return 0;
  }
}

int btech_command_allowed_for_mech(Mech *mech, int cmdflag) {
  int i;

  if (!cmdflag)
    return 1;
  if (!mech || !(i = mech_class_command_flag(mech_class(mech))))
    return 0;
  if (cmdflag > 0) {
    if (cmdflag & i)
      return 1;
  } else if (!((0 - cmdflag) & i))
    return 1;
  return 0;
}

bool btech_special_command_access(BtechContext *context, DbRef object,
                                  PowerId power) {
  return is_god(context->database, object) ||
         is_wizard(context->database, object) ||
         (power != POWER_NONE &&
          game_object_has_power(context->database, object, power));
}

int HandledCommand_sub(BtechContext *context, DbRef player, DbRef location,
                       char *command) {
  BtechSpecialObject *xcode_obj = NULL;

  const BtechSpecialObjectDefinition *typeOfObject;
  int type;
  BtechCommandDefinition *cmd;
  char *tmpc, *tmpchar;
  int ishelp;

  type = btech_context_which_special(context, location);
  if (type < 0 || (btech_special_object_data_size(&SpecialObjects[type]) > 0 &&
                   !(xcode_obj = red_black_tree_find(context->special_objects,
                                                     (void *)location)))) {
    if (type >= 0 || !is_xcode(context->database, location) ||
        is_zombie(context->database, location))
      return 0;
    if ((type = btech_context_which_special_attribute(context, location)) >=
        0) {
      if (btech_special_object_data_size(&SpecialObjects[type]) > 0)
        return 0;
    } else
      return 0;
  }
#ifdef FAST_WHICHSPECIAL
  if (type > (int)(NUM_SPECIAL_OBJECTS))
    return 0;
#endif
  typeOfObject = &SpecialObjects[type];
  tmpc = strstr(command, " ");
  if (tmpc)
    *tmpc = 0;
  ishelp = !strcmp(command, "HELP");
  for (tmpchar = command; *tmpchar; tmpchar++)
    *tmpchar = ascii_to_lower(*tmpchar);
  cmd = (BtechCommandDefinition *)hash_table_find(
      command, &context->special_commands[type]);
  if (tmpc)
    *tmpc = ' ';
  if (cmd && (type != GTYPE_MECH ||
              (type == GTYPE_MECH && btech_command_allowed_for_mech(
                                         ((Mech *)xcode_obj), cmd->flag)))) {
#define SKIPSTUFF(a)                                                           \
  while (*a && *a != ' ')                                                      \
    a++;                                                                       \
  while (*a == ' ')                                                            \
  a++
    if (cmd->helpmsg[0] != '@' ||
        btech_special_command_access(context, player,
                                     typeOfObject->power_needed)) {
      SKIPSTUFF(command);
      cmd->handler(player, xcode_obj, command);
    } else
      notify(btech_context_evaluation(context), player,
             "Sorry, that command is restricted!");
    return 1;
  } else if (ishelp) {
    SKIPSTUFF(command);
    btech_special_object_help(context, player, typeOfObject->type, type,
                              location, typeOfObject->power_needed, location,
                              command);
    return 1;
  }
  return 0;
}

static bool okay_hcode(BtechContext *context, DbRef object) {
  return object >= 0 && is_xcode(context->database, object) &&
         !is_zombie(context->database, object);
}

/* Main entry point */
bool btech_command_try_execute(BtechContext *context, DbRef player, DbRef loc,
                               char *command) {
  DbRef curr, temp;

  if (strlen(command) > (LBUF_SIZE - MBUF_SIZE))
    return 0;
  if (okay_hcode(context, player) &&
      HandledCommand_sub(context, player, player, command))
    return 1;
  if (okay_hcode(context, loc) &&
      HandledCommand_sub(context, player, loc, command))
    return 1;
  SAFE_DOLIST(context->database, curr, temp,
              game_object_contents(context->database, player)) {
    if (okay_hcode(context, curr))
      if (HandledCommand_sub(context, player, curr, command))
        return 1;
#if 0 /* Recursion is evil ; let's not do that, this time */
		if(has_contents(context->database, curr))
			if(HandledCommand_contents(player, curr, command))
				return 1;
#endif
  }
  return 0;
}

void InitSpecialHash(BtechContext *context, int which);
const int global_specials = NUM_SPECIAL_OBJECTS;

void *NewSpecialObject(BtechContext *context, long id, int type) {
  BtechSpecialObject *xcode_obj = NULL;
  size_t data_size = btech_special_object_data_size(&SpecialObjects[type]);

  if (data_size) {
    xcode_obj = (BtechSpecialObject *)calloc(1, data_size);
    if (!xcode_obj) {
      printf("Unable to calloc\n");
      exit(1);
    }
    xcode_obj->type = type;
    xcode_obj->size = data_size;
    xcode_obj->context = context;

    if (SpecialObjects[type].lifecycle)
      SpecialObjects[type].lifecycle(id, (void **)&xcode_obj, SPECIAL_ALLOC);

    red_black_tree_insert(context->special_objects, (void *)id, xcode_obj);
  }

  return xcode_obj;
}

void CreateNewSpecialObject(BtechContext *context, DbRef player, DbRef key) {
  void *new;
  const BtechSpecialObjectDefinition *typeOfObject;
  int type;
  char *str;

  str = btech_attribute_read(context->database, key, A_XTYPE,
                             (char[LBUF_SIZE]){0});
  if (!(str && *str)) {
    notify(btech_context_evaluation(context), player,
           "You must first set the XTYPE using @xtype <object>=<type>");
    notify(btech_context_evaluation(context), player,
           "Valid XTYPEs include: MECH, MECHREP, MAP, DEBUG, "
           "AUTOPILOT, TURRET.");
    notify(btech_context_evaluation(context), player, "Resetting XCODE flag.");
    c_xcode(context->database, key); /* Reset the flag */
    return;
  }

  /* Find the special objects */
  type = btech_context_which_special_attribute(context, key);
  if (type > -1) {
    /* We found the proper special object */
    typeOfObject = &SpecialObjects[type];
    if (btech_special_object_data_size(typeOfObject)) {
      new = NewSpecialObject(context, key, type);
      if (!new)
        notify(btech_context_evaluation(context), player,
               "Memory allocation failure!");
    }
  } else {
    notify(btech_context_evaluation(context), player,
           "That is not a valid XTYPE!");
    notify(btech_context_evaluation(context), player,
           "Valid XTYPEs include: MECH, MECHREP, MAP, DEBUG, "
           "AUTOPILOT, TURRET.");
    notify(btech_context_evaluation(context), player, "Resetting XCODE flag.");
    c_xcode(context->database, key);
  }
}

void btech_special_object_dispose(BtechContext *context, DbRef player,
                                  DbRef key) {
  BtechSpecialObject *xcode_obj;

  int i;
  const BtechSpecialObjectDefinition *typeOfObject;

  xcode_obj = red_black_tree_find(context->special_objects, (void *)key);

  i = btech_context_which_special_attribute(context, key);
  if (i < 0) {
    notify(btech_context_evaluation(context), player,
           "CRITICAL: Unable to free data, inconsistency somewhere. Please");
    notify(btech_context_evaluation(context), player,
           "contact a wizard about this _NOW_!");
    return;
  }
  typeOfObject = &SpecialObjects[i];

  if (btech_special_object_data_size(typeOfObject) > 0 &&
      btech_context_which_special(context, key) != i) {
    notify(btech_context_evaluation(context), player,
           "Semi-critical error has occured. For some reason the "
           "object's data differs\nfrom the data on the object. Please "
           "contact a wizard about this.");
    i = btech_context_which_special(context, key);
  }
  if (xcode_obj) {
    if (typeOfObject->lifecycle)
      typeOfObject->lifecycle(key, (void **)&xcode_obj, SPECIAL_FREE);
    red_black_tree_delete(context->special_objects, (void *)key);
    mux_event_remove_data(context->events, xcode_obj);
    free(xcode_obj);
  } else if (btech_special_object_data_size(typeOfObject) > 0) {
    notify(btech_context_evaluation(context), player,
           "This object is not in the special object DBASE.");
    notify(btech_context_evaluation(context), player,
           "Please contact a wizard about this bug. ");
  }
}

static void destroy_special_object(void *key, void *data, void *arg) {
  BtechContext *context = arg;
  BtechSpecialObject *xcode_obj = data;
  const BtechSpecialObjectDefinition *type = &SpecialObjects[xcode_obj->type];

  mux_event_remove_data(context->events, xcode_obj);
  if (type->lifecycle)
    type->lifecycle((DbRef)key, (void **)&xcode_obj, SPECIAL_FREE);
  free(xcode_obj);
}

void btech_context_release_owned_state(BtechContext *context) {
  if (context == nullptr)
    return;

  if (context->special_objects != nullptr) {
    red_black_tree_release(context->special_objects, destroy_special_object,
                           context);
    context->special_objects = nullptr;
  }
  for (size_t i = 0; i < context->special_command_count; i++)
    hash_table_destroy(&context->special_commands[i]);
  free(context->special_commands);
  context->special_commands = nullptr;
  context->special_command_count = 0;
  btech_stats_destroy(context);
#ifdef BT_ADVANCED_ECON
  btech_part_costs_destroy(context);
#endif
  destroy_partname_tables(context);
  missile_hit_registry_destroy(&context->missile_hits);
  btech_weapon_settings_destroy(&context->weapon_settings);
  mech_template_registry_destroy(context);
  mech_reference_cache_destroy(context);
  *context = (BtechContext){0};
}

void Dump_Mech(BtechContext *context, DbRef player, int type, char *typestr) {
  notify(btech_context_evaluation(context), player,
         "Support discontinued. Bother a wiz if this bothers you.");
}

void DumpMechs(BtechContext *context, DbRef player) {
  Dump_Mech(context, player, GTYPE_MECH, "mech");
}

void DumpMaps(BtechContext *context, DbRef player) {
  notify(btech_context_evaluation(context), player,
         "Support discontinued. Bother a wiz if this bothers you.");
}

/***************** INTERNAL ROUTINES *************/
#ifdef FAST_WHICHSPECIAL
int btech_context_which_special(BtechContext *context, DbRef key) {
  BtechSpecialObject *xcode_obj;

  if (!is_good_obj(context->database, key))
    return -1;
  if (!is_xcode(context->database, key))
    return -1;
  if (!(xcode_obj = red_black_tree_find(context->special_objects, (void *)key)))
    return -1;
  return xcode_obj->type;
}
#else
int btech_context_which_special(BtechContext *context, DbRef key) {
  return btech_context_which_special_attribute(context, key);
}
#endif

int btech_context_which_special_attribute(BtechContext *context, DbRef key) {
  int i;
  int returnValue = -1;
  char *str;

  if (!is_xcode(context->database, key))
    return -1;
  str = btech_attribute_read(context->database, key, A_XTYPE,
                             (char[LBUF_SIZE]){0});
  if (str && *str) {
    for (i = 0; i < (int)(NUM_SPECIAL_OBJECTS); i++) {
      if (!strcmp(SpecialObjects[i].type, str)) {
        returnValue = i;
        break;
      }
    }
  }
  return (returnValue);
}

bool btech_context_is_mech(BtechContext *context, DbRef key) {
  return btech_context_which_special(context, key) == GTYPE_MECH;
}

bool btech_context_is_auto(BtechContext *context, DbRef key) {
  return btech_context_which_special(context, key) == GTYPE_AUTO;
}

bool btech_context_is_map(BtechContext *context, DbRef key) {
  return btech_context_which_special(context, key) == GTYPE_MAP;
}

void *btech_context_find_object(BtechContext *context, DbRef key) {
  return red_black_tree_find(context->special_objects, (void *)key);
}

void InitSpecialHash(BtechContext *context, int which) {
  char *tmp, *tmpc;
  int i;
  char buf[MBUF_SIZE];

  hash_table_initialize(&context->special_commands[which], 20 * HASH_FACTOR);
  for (i = 0; (tmp = SpecialObjects[which].commands[i].name); i++) {
    if (!btech_command_definition_has_handler(
            &SpecialObjects[which].commands[i]))
      continue;
    tmpc = buf;
    for (; *tmp && *tmp != ' '; tmp++)
      *(tmpc++) = ascii_to_lower(*tmp);
    *tmpc = 0;
    if ((tmpc = strstr(buf, " ")))
      *tmpc = 0;
    hash_table_add(buf, (int *)&SpecialObjects[which].commands[i],
                   &context->special_commands[which]);
  }
}

void btech_special_object_flag_changed(BtechContext *context, DbRef player,
                                       DbRef obj, bool from, bool to) {
  if (from == to)
    return;
  if (!to) {
    s_xcode(context->database, obj);
    btech_special_object_dispose(context, player, obj);
    c_xcode(context->database, obj);
  } else
    CreateNewSpecialObject(context, player, obj);
}

#undef notify
void mecha_notify(EvaluationContext *evaluation, DbRef player,
                  const char *msg) {
  raw_notify(evaluation, player, msg);
}

void mecha_notify_except(EvaluationContext *evaluation, DbRef loc, DbRef player,
                         DbRef exception, const char *msg) {
  DbRef first;

  if (loc != exception)
    notify_checked(evaluation, loc, player, msg,
                   MSG_ME_ALL | MSG_F_UP | MSG_S_INSIDE | MSG_NBR_EXITS_A);
  DOLIST(evaluation->world->database, first,
         game_object_contents(evaluation->world->database, loc)) {
    if (first != exception) {
      notify_checked(evaluation, first, player, msg,
                     (MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE));
    }
  }
}

void btech_special_objects_reset(BtechContext *context) {
  mux_event_run_by_type(context->events, EVENT_HIDE);
  mux_event_run_by_type(context->events, EVENT_BLINDREC);
}

BattleMap *btech_context_get_map(BtechContext *context, DbRef d) {
  BtechSpecialObject *xcode_obj;

  if (!(xcode_obj = red_black_tree_find(context->special_objects, (void *)d)))
    return NULL;
  if (xcode_obj->type != GTYPE_MAP)
    return NULL;
  return (BattleMap *)xcode_obj;
}

Mech *btech_context_get_mech(BtechContext *context, DbRef d) {
  BtechSpecialObject *xcode_obj;

  if (!(is_good_obj(context->database, d)))
    return NULL;
  if (!(is_xcode(context->database, d)))
    return NULL;
  if (!(xcode_obj = red_black_tree_find(context->special_objects, (void *)d)))
    return NULL;
  if (xcode_obj->type != GTYPE_MECH)
    return NULL;
  return (Mech *)xcode_obj;
}
