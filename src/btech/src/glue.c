
/*
 * $Id: glue.c,v 1.4 2005/08/08 09:43:09 murrayma Exp $
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

#include "mux/server/platform.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define FAST_WHICHSPECIAL

#include "mux/network/mux_event_alloc.h"

#define _GLUE_C

/*** #include all the prototype here! ****/
#include "autopilot.h"
#include "coolmenu.h"
#include "debug.h"
#include "glue.h"
#include "mech.events.h"
#include "mech.h"
#include "mech.tech.h"
#include "mechrep.h"
#include "mux/database/powers.h"
#include "mux/support/ansi.h"
#include "mux/support/red_black_tree.h"
#include "mycool.h"
#include "p.bsuit.h"
#include "p.ds.turret.h"
#include "p.mech.partnames.h"
#include "p.mech.stat.h"
#include "p.mechfile.h"
#include "p.mechrep.h"
#include "persistence/btech_persistence.h"
#include "turret.h"

/* Special object parameters.  */
const SpecialObjectStruct SpecialObjects[] = {
    {"MECH", mechcommands, sizeof(MECH), newfreemech, HEAT_TICK, mech_update,
     POW_MECH},
    {"DEBUG", debugcommands, sizeof(XCODE), NULL, 0, NULL, POW_SECURITY},
    {"MECHREP", mechrepcommands, sizeof(MECHREP), newfreemechrep, 0, NULL,
     POW_MECHREP},
    {"MAP", mapcommands, sizeof(MAP), newfreemap, LOS_TICK, map_update,
     POW_MAP},
    {"AUTOPILOT", autopilotcommands, sizeof(AUTO), auto_newautopilot, 0, NULL,
     POW_SECURITY},
    {"TURRET", turretcommands, sizeof(TURRET_T), newturret, 0, NULL,
     POW_SECURITY}};

#define NUM_SPECIAL_OBJECTS                                                    \
  (sizeof(SpecialObjects) / sizeof(SpecialObjectStruct))

/* Prototypes */

/*************CALLABLE PROTOS*****************/

/* Main entry point */
int HandledCommand(BtechContext *context, DbRef player, DbRef loc,
                   char *command);

/* called when user creates/removes hardcode flag */
void CreateNewSpecialObject(BtechContext *context, DbRef player, DbRef key);
void DisposeSpecialObject(BtechContext *context, DbRef player, DbRef key);
void list_hashstat(DbRef player, const char *tab_name, HashTable *htab);
void raw_notify(EvaluationContext *evaluation, DbRef player, const char *msg);

/*************PERSONAL PROTOS*****************/
void *NewSpecialObject(BtechContext *context, long id, int type);
void *btech_context_find_object(BtechContext *context, DbRef key);
static void DoSpecialObjectHelp(BtechContext *context, DbRef player, char *type,
                                int id, int loc, int powerneeded, int objid,
                                char *arg);
void initialize_colorize(BtechContext *context);
void destroy_colorize(BtechContext *context);

int btech_context_which_special(BtechContext *context, DbRef key);
static int btech_context_which_special_attribute(BtechContext *context,
                                                 DbRef key);

static int compare_dbrefs(void *key1, void *key2, void *token) {
  const DbRef key1_val = (DbRef)key1;
  const DbRef key2_val = (DbRef)key2;

  return key1_val - key2_val;
}

static void init_xcode_tree(BtechContext *context) {
  context->special_objects = red_black_tree_init(compare_dbrefs, nullptr);
  if (!context->special_objects) {
    /* TODO: We could handle this more gracefully... */
    exit(EXIT_FAILURE);
  }
}

/*********************************************/

static int Can_Use_Command(MECH *mech, int cmdflag) {
#define TYPE2FLAG(a)                                                           \
  ((a) == CLASS_MECH         ? GFLAG_MECH                                      \
   : (a) == CLASS_VEH_GROUND ? GFLAG_GROUNDVEH                                 \
   : (a) == CLASS_AERO       ? GFLAG_AERO                                      \
   : DropShip(a)             ? GFLAG_DS                                        \
   : (a) == CLASS_VTOL       ? GFLAG_VTOL                                      \
   : (a) == CLASS_VEH_NAVAL  ? GFLAG_NAVAL                                     \
   : (a) == CLASS_BSUIT      ? GFLAG_BSUIT                                     \
   : (a) == CLASS_MW         ? GFLAG_MW                                        \
                             : 0)
  int i;

  if (!cmdflag)
    return 1;
  if (!mech || !(i = TYPE2FLAG(MechType(mech))))
    return 0;
  if (cmdflag > 0) {
    if (cmdflag & i)
      return 1;
  } else if (!((0 - cmdflag) & i))
    return 1;
  return 0;
}

static bool have_mech_power(BtechContext *context, DbRef object, int power) {
  return (game_object_powers2(context->database, object) & power) ||
         is_wizard(context->database, object);
}

int HandledCommand_sub(BtechContext *context, DbRef player, DbRef location,
                       char *command) {
  XCODE *xcode_obj = NULL;

  const SpecialObjectStruct *typeOfObject;
  int type;
  CommandsStruct *cmd;
  char *tmpc, *tmpchar;
  int ishelp;

  type = btech_context_which_special(context, location);
  if (type < 0 || (SpecialObjects[type].datasize > 0 &&
                   !(xcode_obj = red_black_tree_find(context->special_objects,
                                                     (void *)location)))) {
    if (type >= 0 || !is_hardcode(context->database, location) ||
        is_zombie(context->database, location))
      return 0;
    if ((type = btech_context_which_special_attribute(context, location)) >=
        0) {
      if (SpecialObjects[type].datasize > 0)
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
    *tmpchar = ToLower(*tmpchar);
  cmd = (CommandsStruct *)hash_table_find(command,
                                          &context->special_commands[type]);
  if (tmpc)
    *tmpc = ' ';
  if (cmd && (type != GTYPE_MECH ||
              (type == GTYPE_MECH &&
               Can_Use_Command(((MECH *)xcode_obj), cmd->flag)))) {
#define SKIPSTUFF(a)                                                           \
  while (*a && *a != ' ')                                                      \
    a++;                                                                       \
  while (*a == ' ')                                                            \
  a++
    if (cmd->helpmsg[0] != '@' ||
        have_mech_power(context, player, typeOfObject->power_needed)) {
      SKIPSTUFF(command);
      ((void (*)(DbRef, void *, char *))cmd->func)(player, xcode_obj, command);
    } else
      notify(btech_context_evaluation(context), player,
             "Sorry, that command is restricted!");
    return 1;
  } else if (ishelp) {
    SKIPSTUFF(command);
    DoSpecialObjectHelp(context, player, typeOfObject->type, type, location,
                        typeOfObject->power_needed, location, command);
    return 1;
  }
  return 0;
}

static bool okay_hcode(BtechContext *context, DbRef object) {
  return object >= 0 && is_hardcode(context->database, object) &&
         !is_zombie(context->database, object);
}

/* Main entry point */
int HandledCommand(BtechContext *context, DbRef player, DbRef loc,
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

static int remove_from_all_maps_func(void *key, void *data, int depth,
                                     void *arg) {
  XCODE *const xcode_obj = data;
  MECH *const mech = arg;

  if (xcode_obj->type == GTYPE_MAP) {
    MAP *map;
    int i;

    if (!(map = btech_context_get_map(mech->xcode.context, (DbRef)key)))
      return 1;
    for (i = 0; i < map->first_free; i++)
      if (map->mechsOnMap[i] == mech->mynum)
        map->mechsOnMap[i] = -1;
  }
  return 1;
}

void mech_remove_from_all_maps(MECH *mech) {
  red_black_tree_walk(mech->xcode.context->special_objects, WALK_INORDER,
                      remove_from_all_maps_func, mech);
}

typedef struct RemoveFromAllMapsContext {
  MECH *mech;
  DbRef except_map;
} RemoveFromAllMapsContext;

static int remove_from_all_maps_except_func(void *key, void *data, int depth,
                                            void *arg) {
  DbRef key_val = (DbRef)key;
  XCODE *const xcode_obj = data;
  RemoveFromAllMapsContext *context = arg;
  MECH *const mech = context->mech;

  if (xcode_obj->type == GTYPE_MAP) {
    int i;
    MAP *map;

    if (key_val == context->except_map)
      return 1;
    if (!(map = btech_context_get_map(mech->xcode.context, key_val)))
      return 1;
    for (i = 0; i < map->first_free; i++)
      if (map->mechsOnMap[i] == mech->mynum)
        map->mechsOnMap[i] = -1;
  }
  return 1;
}

void mech_remove_from_all_maps_except(MECH *mech, int num) {
  RemoveFromAllMapsContext context = {
      .mech = mech,
      .except_map = num,
  };

  red_black_tree_walk(mech->xcode.context->special_objects, WALK_INORDER,
                      remove_from_all_maps_except_func, &context);
}

static int load_update2(void *key, void *data, int depth, void *arg) {
  XCODE *const xcode_obj = data;

  if (xcode_obj->type == GTYPE_MECH)
    mech_map_consistency_check((void *)xcode_obj);
  return 1;
}

static int load_update4(void *key, void *data, int depth, void *arg) {
  XCODE *const xcode_obj = data;
  BtechContext *const context = arg;

  if (xcode_obj->type == GTYPE_MECH) {
    MECH *const mech = (MECH *)xcode_obj;
    MAP *map;

    if (!(map = btech_context_get_map(context, mech->mapindex))) {
      /* Ugly kludge */
      if ((map = btech_context_get_map(
               context, game_object_location(context->database, mech->mynum))))
        mech_Rsetmapindex(GOD, mech,
                          tprintf("%ld", game_object_location(context->database,
                                                              mech->mynum)));
      if (!(map = btech_context_get_map(context, mech->mapindex)))
        return 1;
    }

    if (!Started(mech))
      return 1;
    StartSeeing(mech);
    UpdateRecycling(mech);
    MaybeMove(mech);
  }
  return 1;
}

static int load_update3(void *key, void *data, int depth, void *arg) {
  XCODE *const xcode_obj = data;

  if (xcode_obj->type == GTYPE_MAP) {
    eliminate_empties((MAP *)xcode_obj);
    recalculate_minefields((MAP *)xcode_obj);
  }
  return 1;
}

/*
 * Read in autopilot data
 */
static int load_autopilot_data(void *key, void *data, int depth, void *arg) {
  XCODE *const xcode_obj = data;
  BtechContext *const context = arg;

  if (xcode_obj->type == GTYPE_AUTO) {
    AUTO *const autopilot = (AUTO *)xcode_obj;

    int i;

    /* Commands and A* paths are restored before these derived caches. */
    autopilot->weaplist = NULL;
    for (i = 0; i < AUTO_PROFILE_MAX_SIZE; i++) {
      autopilot->profile[i] = NULL;
    }

    if (!autopilot->mymechnum || !(autopilot->mymech = btech_context_get_mech(
                                       context, autopilot->mymechnum))) {
      DoStopGun(autopilot);
    } else {
      /*
       * Weapon lists and range profiles are caches derived from the restored
       * MECH definition. Rebuild them instead of persisting cache trees.
       */
      auto_update_profile_event(autopilot);

      /*
       * MUX event nodes are runtime-only. An autopilot that was engaged at
       * checkpoint time is identified by the durable MECH->AUTO link and by
       * the AUTO object being inside that MECH. Requeue its dispatcher from
       * the durable command list; it recreates goal-specific events itself.
       */
      if (MechAuto(autopilot->mymech) == autopilot->mynum &&
          game_object_location(context->database, autopilot->mynum) ==
              autopilot->mymechnum &&
          autopilot->commands &&
          doubly_linked_list_size(autopilot->commands) > 0 &&
          !mux_event_count_type_data(context->events, EVENT_AUTOCOM, autopilot))
        AUTO_COM(autopilot, AUTOPILOT_NC_DELAY);
      if (Gunning(autopilot))
        DoStartGun(autopilot);
    }
  }

  return 1;
}

void LoadSpecialObjects(BtechContext *context) {
  DbRef i;
  int type;

  init_xcode_tree(context);
  context->special_commands =
      calloc(NUM_SPECIAL_OBJECTS, sizeof(*context->special_commands));
  if (context->special_commands == nullptr)
    exit(EXIT_FAILURE);
  context->special_command_count = NUM_SPECIAL_OBJECTS;

  mux_event_initialize(context->events);
  init_stat(context);
  initialize_partname_tables(context);
  /* The SQLite startup path still needs ANSI parser state for BTech output. */
  initialize_colorize(context);
  if (!btech_weapon_settings_initialize(&context->weapon_settings))
    exit(EXIT_FAILURE);
  if (!missile_hit_registry_initialize(&context->missile_hits, context))
    exit(EXIT_FAILURE);
  /* Loop through the entire database, and if it has the special */
  /* object flag, add it to our linked list. */
  DO_WHOLE_DB(context->database, i)
  if (is_hardcode(context->database, i) && !is_going(context->database, i) &&
      !is_halted(context->database, i)) {
    type = btech_context_which_special_attribute(context, i);
    if (type >= 0) {
      if (SpecialObjects[type].datasize > 0)
        NewSpecialObject(context, i, type);
    } else
      c_hardcode(context->database, i); /* Reset the flag */
  }
  for (i = 0; i < (int)(NUM_SPECIAL_OBJECTS); i++) {
    InitSpecialHash(context, i);
  }
  init_btechstats(context);
#ifdef BTMUX_PERSISTENCE_TESTING
  /* The integration fixture creates its initial SQLite special-state rows. */
  if (getenv("BTMUX_TEST_BTECH_BOOTSTRAP")) {
    heartbeat_init(context);
    return;
  }
#endif
  if (btech_persistence_load_special_state_path(
          context, context->configuration->database.gamedb) < 0) {
    exit(EXIT_FAILURE);
  }
  red_black_tree_walk(context->special_objects, WALK_INORDER, load_update2,
                      context);
  red_black_tree_walk(context->special_objects, WALK_INORDER, load_update3,
                      context);
  red_black_tree_walk(context->special_objects, WALK_INORDER, load_update4,
                      context);
  red_black_tree_walk(context->special_objects, WALK_INORDER,
                      load_autopilot_data, context);
  heartbeat_init(context);
}

static int UpdateSpecialObject_func(void *key, void *data, int depth,
                                    void *arg) {
  XCODE *const xcode_obj = data;
  BtechContext *const context = arg;

  if (!SpecialObjects[xcode_obj->type].updateTime)
    return 1;
  if ((context->clock->now % SpecialObjects[xcode_obj->type].updateTime))
    return 1;
  ((void (*)(DbRef, void *))SpecialObjects[xcode_obj->type].updatefunc)(
      (DbRef)key, xcode_obj);
  return 1;
}

/* This is called once a second for each special object */

/* Note the new handling for calls being done at <1second intervals,
   or possibly at >1second intervals */

void UpdateSpecialObjects(BtechContext *context) {
  const char *cmdsave;
  int i;
  int times = context->last_special_update
                  ? (context->clock->now - context->last_special_update)
                  : 1;

  if (times > 20)
    times = 20; /* Machine's hopelessly lagged,
                           we don't want to make it [much] worse */
  cmdsave = btech_context_command(context)->debug_command;
  for (i = 0; i < times; i++) {
    mux_event_run(context->events);
    btech_context_command(context)->debug_command =
        (char *)"< Generic hcode update handler>";
    red_black_tree_walk(context->special_objects, WALK_INORDER,
                        UpdateSpecialObject_func, context);
  }
  context->last_special_update = context->clock->now;
  btech_context_command(context)->debug_command = cmdsave;
}

void *NewSpecialObject(BtechContext *context, long id, int type) {
  XCODE *xcode_obj = NULL;

  if (SpecialObjects[type].datasize) {
    xcode_obj = (XCODE *)calloc(1, SpecialObjects[type].datasize);
    if (!xcode_obj) {
      printf("Unable to calloc\n");
      exit(1);
    }
    xcode_obj->type = type;
    xcode_obj->size = SpecialObjects[type].datasize;
    xcode_obj->context = context;

    if (SpecialObjects[type].allocfreefunc)
      ((void (*)(DbRef, void **, int))SpecialObjects[type].allocfreefunc)(
          id, (void **)&xcode_obj, SPECIAL_ALLOC);

    red_black_tree_insert(context->special_objects, (void *)id, xcode_obj);
  }

  return xcode_obj;
}

void CreateNewSpecialObject(BtechContext *context, DbRef player, DbRef key) {
  void *new;
  const SpecialObjectStruct *typeOfObject;
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
    notify(btech_context_evaluation(context), player,
           "Resetting hardcode flag.");
    c_hardcode(context->database, key); /* Reset the flag */
    return;
  }

  /* Find the special objects */
  type = btech_context_which_special_attribute(context, key);
  if (type > -1) {
    /* We found the proper special object */
    typeOfObject = &SpecialObjects[type];
    if (typeOfObject->datasize) {
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
    notify(btech_context_evaluation(context), player,
           "Resetting HARDCODE flag.");
    c_hardcode(context->database, key);
  }
}

void DisposeSpecialObject(BtechContext *context, DbRef player, DbRef key) {
  XCODE *xcode_obj;

  int i;
  const SpecialObjectStruct *typeOfObject;

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

  if (typeOfObject->datasize > 0 &&
      btech_context_which_special(context, key) != i) {
    notify(btech_context_evaluation(context), player,
           "Semi-critical error has occured. For some reason the "
           "object's data differs\nfrom the data on the object. Please "
           "contact a wizard about this.");
    i = btech_context_which_special(context, key);
  }
  if (xcode_obj) {
    if (typeOfObject->allocfreefunc)
      ((void (*)(DbRef, void **, int))typeOfObject->allocfreefunc)(
          key, (void **)&xcode_obj, SPECIAL_FREE);
    red_black_tree_delete(context->special_objects, (void *)key);
    mux_event_remove_data(context->events, xcode_obj);
    free(xcode_obj);
  } else if (typeOfObject->datasize > 0) {
    notify(btech_context_evaluation(context), player,
           "This object is not in the special object DBASE.");
    notify(btech_context_evaluation(context), player,
           "Please contact a wizard about this bug. ");
  }
}

static void destroy_special_object(void *key, void *data, void *arg) {
  BtechContext *context = arg;
  XCODE *xcode_obj = data;
  const SpecialObjectStruct *type = &SpecialObjects[xcode_obj->type];

  mux_event_remove_data(context->events, xcode_obj);
  if (type->allocfreefunc)
    ((void (*)(DbRef, void **, int))type->allocfreefunc)(
        (DbRef)key, (void **)&xcode_obj, SPECIAL_FREE);
  free(xcode_obj);
}

void btech_context_destroy(BtechContext *context) {
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
  destroy_colorize(context);
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
  XCODE *xcode_obj;

  if (!is_good_obj(context->database, key))
    return -1;
  if (!is_hardcode(context->database, key))
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

static int btech_context_which_special_attribute(BtechContext *context,
                                                 DbRef key) {
  int i;
  int returnValue = -1;
  char *str;

  if (!is_hardcode(context->database, key))
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

void center_string(char *destination, size_t destination_size,
                   const char *source, int width) {
  if (destination == nullptr || destination_size == 0)
    return;

  size_t source_length = strlen(source);
  size_t padding = 0;
  if (width > 0 && (size_t)width > source_length)
    padding = ((size_t)width - source_length) / 2;
  padding = MIN(padding, destination_size - 1);
  memset(destination, ' ', padding);
  snprintf(destination + padding, destination_size - padding, "%s", source);
}

static void help_color_initialize(const char *from, char *to) {
  int i;
  char buf[LBUF_SIZE];
  char *tp = to;

  for (i = 0; from[i] && from[i] != ' '; i++)
    ;
  if (from[i]) {

    /*      from[i]=0; */
    strncpy(buf, from, i);
    buf[i] = 0;
    safe_str("%ch%cb", to, &tp);
    safe_str(buf, to, &tp);
    safe_str("%cn ", to, &tp);
    safe_str((char *)&from[i + 1], to, &tp);

    /*      from[i]=' '; */
  } else {
    safe_str("%cc", to, &tp);
    safe_str((char *)from, to, &tp);
    safe_str("%cn", to, &tp);
  }
  *tp = '\0';
}

#define ONE_LINE_TEXTS

#ifdef ONE_LINE_TEXTS
#define MLen CM_ONE
#else
#define MLen CM_TWO
#endif

static char *do_ugly_things(coolmenu **d, char *msg, int len, int initial) {
  coolmenu *c = *d;
  size_t msg_len;
  char *e;
  char buf[LBUF_SIZE];

  /* XXX: Not entirely sure what this is for.  */
#ifndef ONE_LINE_TEXTS
  if (!msg) {
    sim(" ", MLen);
    *d = c;
    return NULL;
  }
#endif

  /*
   * Split off at last space on a line, taking into account initial
   * indentation, etc.  Help messages are strings of words, separated by
   * at most one space, with no word longer than len.
   *
   * All of these assumptions are necessary for this code to be safe.
   * Basically, the code needs to find the breaking space.
   *
   * FIXME: All of this code really needs more cleanup and fixing.
   */
  msg_len = strlen(msg);

  if (msg_len <= (size_t)len) {
    /* Line fits, don't split anything.  */
    e = msg + msg_len;
  } else {
    /* Split at last space on line.  */
    for (e = msg + len - 1; *e != ' '; e--)
      ;
  }

  if (initial > 0) {
    /* Colorize header line.  */
    help_color_initialize(msg, buf);
  } else if (initial < 0) {
    /* Write indented line.  */
    memset(buf, ' ', -initial);
    memcpy(buf - initial, msg, e - msg);
    buf[(e - msg) - initial] = '\0';
  } else {
    /* Write unindented line.  */
    memcpy(buf, msg, e - msg);
    buf[e - msg] = '\0';
  }

  sim(buf, MLen);

  /* Move pointer to start of next line.  */
  if (*e == ' ')
    e++;

  *d = c;
  return *e ? e : NULL;
}

#define Len(s) ((!s || !*s) ? 0 : strlen(s))

#define TAB 3

static void cut_apart_helpmsgs(coolmenu **d, char *msg1, char *msg2, int len,
                               int initial) {
  int l1 = Len(msg1);
  int l2 = Len(msg2);
  int nl1, nl2;

#ifndef ONE_LINE_TEXTS

  msg1 = do_ugly_things(d, msg1, len, initial);
  msg2 =
      do_ugly_things(d, msg2, initial ? len : len - TAB, initial ? 0 : 0 - TAB);
  if (!msg1 && !msg2)
    return;
  nl1 = Len(msg1);
  nl2 = Len(msg2);
  if (nl1 != l1 || nl2 != l2) /* To prevent infinite loops */
    cut_apart_helpmsgs(d, msg1, msg2, len, 0);
#else
  int first = 1;

  while (msg1 && *msg1) {
    msg1 = do_ugly_things(d, msg1, len * 2 - 1, first);
    nl1 = Len(msg1);
    if (nl1 == l1)
      break;
    l1 = nl1;
    first = 0;
  }
  while (msg2 && *msg2) {
    msg2 = do_ugly_things(d, msg2, len * 2 - TAB, 0 - TAB);
    nl2 = Len(msg2);
    if (nl2 == l2)
      break;
    l2 = nl2;
  }

#endif
}

static void DoSpecialObjectHelp(BtechContext *context, DbRef player, char *type,
                                int id, int loc, int powerneeded, int objid,
                                char *arg) {
  int i, j;
  MECH *mech = NULL;
  int pos[100][2];
  int count = 0, csho = 0;
  coolmenu *c = NULL;
  char buf[LBUF_SIZE];
  char *d;
  int dc;

  if (id == GTYPE_MECH)
    mech = btech_context_get_mech(context, loc);
  bzero(pos, sizeof(pos));
  for (i = 0; SpecialObjects[id].commands[i].name; i++) {
    if (!SpecialObjects[id].commands[i].func &&
        (SpecialObjects[id].commands[i].helpmsg[0] != '@' ||
         have_mech_power(context, player, powerneeded)))
      if (id != GTYPE_MECH ||
          Can_Use_Command(mech, SpecialObjects[id].commands[i].flag)) {
        if (count)
          pos[count - 1][1] = i - pos[count - 1][0];
        pos[count][0] = i;
        count++;
      }
  }
  if (count)
    pos[count - 1][1] = i - pos[count - 1][0];
  else {
    pos[0][0] = 0;
    pos[0][1] = i;
    count = 1;
  }
  sim(NULL, CM_ONE | CM_LINE);
  if (!arg || !*arg) {
#define HELPMSG(a)                                                             \
  &SpecialObjects[id]                                                          \
       .commands[a]                                                            \
       .helpmsg[SpecialObjects[id].commands[a].helpmsg[0] == '@']
    for (i = 0; i < count; i++) {
      if (count > 1) {
        center_string(buf, sizeof(buf), HELPMSG(pos[i][0]), 70);
        d = buf;
        sim(tprintf("%s%s%s", "%cg", d, "%c"), CM_ONE);
      } else
        sim(tprintf("%s command listing: ", type), CM_ONE | CM_CENTER);
      for (j = pos[i][0] + (count == 1 ? 0 : 1); j < pos[i][0] + pos[i][1]; j++)
        if (SpecialObjects[id].commands[j].helpmsg[0] != '@' ||
            have_mech_power(context, player, powerneeded))
          if (id != GTYPE_MECH ||
              Can_Use_Command(mech, SpecialObjects[id].commands[j].flag)) {
            strcpy(buf, SpecialObjects[id].commands[j].name);
            d = buf;
            while (*d && *d != ' ')
              d++;
            if (*d == ' ')
              *d = 0;
            sim(buf, CM_FOUR);
            csho++;
          }
    }
    if (!csho)
      vsi(tprintf("There are no commands you are authorized to use here."));
    else {
      sim(NULL, CM_ONE | CM_LINE);
      if (count > 1)
        vsi("Additional info available with 'HELP SUBTOPIC'");
      else
        vsi("Additional info available with 'HELP ALL'");
    }
  } else {
    /* Try to find matching subtopic, or ALL */
    if (!strcasecmp(arg, "all")) {
      if (count > 1) {
        vsi("ALL not available for objects with subcategories.");
        dc = -2;
      } else
        dc = -1;
    } else {
      if (count == 1) {
        vsi("This object doesn't have any other detailed help than 'HELP ALL'");
        dc = -2;
      } else {
        for (i = 0; i < count; i++)
          if (!strcasecmp(arg, HELPMSG(pos[i][0])))
            break;
        if (i == count) {
          vsi("Subcategory not found.");
          dc = -2;
        } else
          dc = i;
      }
    }
    if (dc > -2) {
      for (i = 0; i < count; i++)
        if (dc == -1 || i == dc) {
          if (count > 1) {
            center_string(buf, sizeof(buf), HELPMSG(pos[i][0]), 70);
            vsi(tprintf("%s%s%s", "%cg", buf, "%c"));
          }
          for (j = pos[i][0] + (count == 1 ? 0 : 1); j < pos[i][0] + pos[i][1];
               j++)
            if (SpecialObjects[id].commands[j].helpmsg[0] != '@' ||
                have_mech_power(context, player, powerneeded))
              if (id != GTYPE_MECH ||
                  Can_Use_Command(mech, SpecialObjects[id].commands[j].flag))
                cut_apart_helpmsgs(&c, SpecialObjects[id].commands[j].name,
                                   HELPMSG(j), 37, 1);
        }
    }
  }
  sim(NULL, CM_ONE | CM_LINE);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}

void InitSpecialHash(BtechContext *context, int which) {
  char *tmp, *tmpc;
  int i;
  char buf[MBUF_SIZE];

  hash_table_initialize(&context->special_commands[which], 20 * HASH_FACTOR);
  for (i = 0; (tmp = SpecialObjects[which].commands[i].name); i++) {
    if (!SpecialObjects[which].commands[i].func)
      continue;
    tmpc = buf;
    for (; *tmp && *tmp != ' '; tmp++)
      *(tmpc++) = ToLower(*tmp);
    *tmpc = 0;
    if ((tmpc = strstr(buf, " ")))
      *tmpc = 0;
    hash_table_add(buf, (int *)&SpecialObjects[which].commands[i],
                   &context->special_commands[which]);
  }
}

void handle_xcode(BtechContext *context, DbRef player, DbRef obj, int from,
                  int to) {
  if (from == to)
    return;
  if (!to) {
    s_hardcode(context->database, obj);
    DisposeSpecialObject(context, player, obj);
    c_hardcode(context->database, obj);
  } else
    CreateNewSpecialObject(context, player, obj);
}

#define DEFAULT 0 /* Normal */
#define ANSI_START "\033["
#define ANSI_START_LEN 2
#define ANSI_END "m"
#define ANSI_END_LEN 1

typedef struct ColorTableEntry {
  int bit;
  int negbit;
  char ltr;
  const char *string;
} ColorTableEntry;

static const ColorTableEntry color_table[] = {
    {0x0008, 7, 'n', ANSI_NORMAL},  {0x0001, 0, 'h', ANSI_HILITE},
    {0x0002, 0, 'i', ANSI_INVERSE}, {0x0004, 0, 'f', ANSI_BLINK},
    {0x0010, 0, 'x', ANSI_BLACK},   {0x0010, 0x10, 'l', ANSI_BLACK},
    {0x0020, 0, 'r', ANSI_RED},     {0x0040, 0, 'g', ANSI_GREEN},
    {0x0080, 0, 'y', ANSI_YELLOW},  {0x0100, 0, 'b', ANSI_BLUE},
    {0x0200, 0, 'm', ANSI_MAGENTA}, {0x0400, 0, 'c', ANSI_CYAN},
    {0x0800, 0, 'w', ANSI_WHITE},   {0, 0, 0, nullptr}};

#define CHARS 256
enum { COLOR_ENTRY_COUNT = 13 };

struct BtechColorizeState {
  int reverse[CHARS];
  char *short_sequences[COLOR_ENTRY_COUNT];
};

void initialize_colorize(BtechContext *context) {
  BtechColorizeState *state = calloc(1, sizeof(*state));
  int i;
  char buf[20];
  char *c;

  if (state == nullptr)
    exit(EXIT_FAILURE);
  context->colorize = state;
  c = buf + ANSI_START_LEN;
  for (i = 0; i < CHARS; i++)
    state->reverse[i] = DEFAULT;
  for (i = 0; color_table[i].string; i++) {
    state->reverse[(unsigned char)color_table[i].ltr] = i;
    strcpy(buf, color_table[i].string);
    buf[strlen(buf) - ANSI_END_LEN] = 0;
    state->short_sequences[i] = strdup(c);
    if (state->short_sequences[i] == nullptr)
      exit(EXIT_FAILURE);
  }
}

void destroy_colorize(BtechContext *context) {
  BtechColorizeState *state = context->colorize;

  if (state == nullptr)
    return;
  for (size_t i = 0; i < COLOR_ENTRY_COUNT; i++)
    free(state->short_sequences[i]);
  free(state);
  context->colorize = nullptr;
}

#undef notify
char *colorize(EvaluationContext *evaluation, DbRef player, char *from) {
  const BtechColorizeState *state = evaluation->btech->colorize;
  char *to;
  char *p, *q;
  int color_wanted = 0;
  int i;
  int cnt;

  q = to = alloc_lbuf("colorize");
#if 1
  for (p = from; *p; p++) {
    if (*p == '%' && *(p + 1) == 'c') {
      p += 2;
      if (*p <= 0)
        i = DEFAULT;
      else
        i = state->reverse[(unsigned char)*p];
      if (i == DEFAULT && *p != 'n')
        p--;
      color_wanted &= ~color_table[i].negbit;
      color_wanted |= color_table[i].bit;
    } else {
      if (color_wanted && is_ansi(evaluation->world->database, player)) {
        *q = 0;
        /* Generate efficient color string */
        strcpy(q, ANSI_START);
        q += ANSI_START_LEN;
        cnt = 0;
        for (i = 0; color_table[i].string; i++)
          if (color_wanted & color_table[i].bit &&
              color_table[i].bit != color_table[i].negbit) {
            if (cnt)
              *q++ = ';';
            strcpy(q, state->short_sequences[i]);
            q += strlen(state->short_sequences[i]);
            cnt++;
          }
        strcpy(q, ANSI_END);
        q += ANSI_END_LEN;
        color_wanted = 0;
      }
      *q++ = *p;
    }
  }
  *q = 0;
  if (color_wanted && is_ansi(evaluation->world->database, player)) {
    /* Generate efficient color string */
    strcpy(q, ANSI_START);
    q += ANSI_START_LEN;
    cnt = 0;
    for (i = 0; color_table[i].string; i++)
      if (color_wanted & color_table[i].bit &&
          color_table[i].bit != color_table[i].negbit) {
        if (cnt)
          *q++ = ';';
        strcpy(q, state->short_sequences[i]);
        q += strlen(state->short_sequences[i]);
        cnt++;
      }
    strcpy(q, ANSI_END);
    q += ANSI_END_LEN;
    color_wanted = 0;
  }
#else
  strcpy(to, p);
#endif
  return to;
}

void mecha_notify(EvaluationContext *evaluation, DbRef player, char *msg) {
  char *tmp;

  tmp = colorize(evaluation, player, msg);
  raw_notify(evaluation, player, tmp);
  free_lbuf(tmp);
}

void mecha_notify_except(EvaluationContext *evaluation, DbRef loc, DbRef player,
                         DbRef exception, char *msg) {
  DbRef first;

  if (loc != exception)
    notify_checked(evaluation, loc, player, msg,
                   (MSG_ME_ALL | MSG_F_UP | MSG_S_INSIDE | MSG_NBR_EXITS_A |
                    MSG_COLORIZE));
  DOLIST(evaluation->world->database, first,
         game_object_contents(evaluation->world->database, loc)) {
    if (first != exception) {
      notify_checked(evaluation, first, player, msg,
                     (MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE | MSG_COLORIZE));
    }
  }
}

void ResetSpecialObjects(BtechContext *context) {
  mux_event_run_by_type(context->events, EVENT_HIDE);
  mux_event_run_by_type(context->events, EVENT_BLINDREC);
}

MAP *btech_context_get_map(BtechContext *context, DbRef d) {
  XCODE *xcode_obj;

  if (!(xcode_obj = red_black_tree_find(context->special_objects, (void *)d)))
    return NULL;
  if (xcode_obj->type != GTYPE_MAP)
    return NULL;
  return (MAP *)xcode_obj;
}

MECH *btech_context_get_mech(BtechContext *context, DbRef d) {
  XCODE *xcode_obj;

  if (!(is_good_obj(context->database, d)))
    return NULL;
  if (!(is_hardcode(context->database, d)))
    return NULL;
  if (!(xcode_obj = red_black_tree_find(context->special_objects, (void *)d)))
    return NULL;
  if (xcode_obj->type != GTYPE_MECH)
    return NULL;
  return (MECH *)xcode_obj;
}
