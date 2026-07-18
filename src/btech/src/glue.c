
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
#include "persistence/btech_persistence.h"
#include "turret.h"

/* Special object parameters.  */
SpecialObjectStruct SpecialObjects[] = {
    {"MECH", mechcommands, sizeof(MECH), newfreemech, HEAT_TICK, mech_update,
     POW_MECH},
    {"DEBUG", debugcommands, 0, NULL, 0, NULL, POW_SECURITY},
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
int HandledCommand(DbRef player, DbRef loc, char *command);

/* called when user creates/removes hardcode flag */
void CreateNewSpecialObject(DbRef player, DbRef key);
void DisposeSpecialObject(DbRef player, DbRef key);
void list_hashstat(DbRef player, const char *tab_name, HashTable *htab);
void raw_notify(EvaluationContext *evaluation, DbRef player, const char *msg);

/*************PERSONAL PROTOS*****************/
void *NewSpecialObject(long id, int type);
void *FindObjectsData(DbRef key);
static void DoSpecialObjectHelp(DbRef player, char *type, int id, int loc,
                                int powerneeded, int objid, char *arg);
void initialize_colorize();

#ifndef FAST_WHICHSPECIAL

#define WhichSpecialS WhichSpecial
int WhichSpecial(DbRef key);

#else

int WhichSpecial(DbRef key);
static int WhichSpecialS(DbRef key);

#endif

RedBlackTree xcode_tree = NULL;

static int compare_dbrefs(void *key1, void *key2, void *token) {
  const DbRef key1_val = (DbRef)key1;
  const DbRef key2_val = (DbRef)key2;

  return key1_val - key2_val;
}

static void init_xcode_tree(void) {
  xcode_tree = red_black_tree_init(compare_dbrefs, NULL);
  if (!xcode_tree) {
    /* TODO: We could handle this more gracefully... */
    exit(EXIT_FAILURE);
  }
}

/*********************************************/

HashTable SpecialCommandHash[NUM_SPECIAL_OBJECTS];

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

int HandledCommand_sub(DbRef player, DbRef location, char *command) {
  XCODE *xcode_obj = NULL;

  struct SpecialObjectStruct *typeOfObject;
  int type;
  CommandsStruct *cmd;
  char *tmpc, *tmpchar;
  int ishelp;

  type = WhichSpecial(location);
  if (type < 0 ||
      (SpecialObjects[type].datasize > 0 &&
       !(xcode_obj = red_black_tree_find(xcode_tree, (void *)location)))) {
    if (type >= 0 || !is_hardcode(btech_context_active()->database, location) ||
        is_zombie(btech_context_active()->database, location))
      return 0;
    if ((type = WhichSpecialS(location)) >= 0) {
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
  cmd = (CommandsStruct *)hash_table_find(command, &SpecialCommandHash[type]);
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
        Have_MechPower(
            game_object_owner(btech_context_active()->database, player),
            typeOfObject->power_needed)) {
      SKIPSTUFF(command);
      ((void (*)(DbRef, void *, char *))cmd->func)(player, xcode_obj, command);
    } else
      notify(BTECH_EVALUATION_CONTEXT, player,
             "Sorry, that command is restricted!");
    return 1;
  } else if (ishelp) {
    SKIPSTUFF(command);
    DoSpecialObjectHelp(player, typeOfObject->type, type, location,
                        typeOfObject->power_needed, location, command);
    return 1;
  }
  return 0;
}

#define OkayHcode(a)                                                           \
  (a >= 0 && is_hardcode(btech_context_active()->database, a) &&               \
   !is_zombie(btech_context_active()->database, a))

/* Main entry point */
int HandledCommand(DbRef player, DbRef loc, char *command) {
  DbRef curr, temp;

  if (strlen(command) > (LBUF_SIZE - MBUF_SIZE))
    return 0;
  if (OkayHcode(player) && HandledCommand_sub(player, player, command))
    return 1;
  if (OkayHcode(loc) && HandledCommand_sub(player, loc, command))
    return 1;
  SAFE_DOLIST(btech_context_active()->database, curr, temp,
              game_object_contents(btech_context_active()->database, player)) {
    if (OkayHcode(curr))
      if (HandledCommand_sub(player, curr, command))
        return 1;
#if 0 /* Recursion is evil ; let's not do that, this time */
		if(has_contents(btech_context_active()->database, curr))
			if(HandledCommand_contents(player, curr, command))
				return 1;
#endif
  }
  return 0;
}

void InitSpecialHash(int which);
void initialize_partname_tables();

int global_specials = NUM_SPECIAL_OBJECTS;

static int remove_from_all_maps_func(void *key, void *data, int depth,
                                     void *arg) {
  XCODE *const xcode_obj = data;
  MECH *const mech = arg;

  if (xcode_obj->type == GTYPE_MAP) {
    MAP *map;
    int i;

    if (!(map = getMap((DbRef)key)))
      return 1;
    for (i = 0; i < map->first_free; i++)
      if (map->mechsOnMap[i] == mech->mynum)
        map->mechsOnMap[i] = -1;
  }
  return 1;
}

void mech_remove_from_all_maps(MECH *mech) {
  red_black_tree_walk(xcode_tree, WALK_INORDER, remove_from_all_maps_func,
                      mech);
}

static DbRef except_map = -1;

static int remove_from_all_maps_except_func(void *key, void *data, int depth,
                                            void *arg) {
  DbRef key_val = (DbRef)key;
  XCODE *const xcode_obj = data;
  MECH *const mech = arg;

  if (xcode_obj->type == GTYPE_MAP) {
    int i;
    MAP *map;

    if (key_val == except_map)
      return 1;
    if (!(map = getMap(key_val)))
      return 1;
    for (i = 0; i < map->first_free; i++)
      if (map->mechsOnMap[i] == mech->mynum)
        map->mechsOnMap[i] = -1;
  }
  return 1;
}

void mech_remove_from_all_maps_except(MECH *mech, int num) {
  /* TODO: Put the mech and the except_map into a structure for arg.  */
  except_map = num;
  red_black_tree_walk(xcode_tree, WALK_INORDER,
                      remove_from_all_maps_except_func, mech);
  except_map = -1;
}

static int load_update2(void *key, void *data, int depth, void *arg) {
  XCODE *const xcode_obj = data;

  if (xcode_obj->type == GTYPE_MECH)
    mech_map_consistency_check((void *)xcode_obj);
  return 1;
}

static int load_update4(void *key, void *data, int depth, void *arg) {
  XCODE *const xcode_obj = data;

  if (xcode_obj->type == GTYPE_MECH) {
    MECH *const mech = (MECH *)xcode_obj;
    MAP *map;

    if (!(map = getMap(mech->mapindex))) {
      /* Ugly kludge */
      if ((map = getMap(game_object_location(btech_context_active()->database,
                                             mech->mynum))))
        mech_Rsetmapindex(
            GOD, mech,
            tprintf("%ld", game_object_location(
                               btech_context_active()->database, mech->mynum)));
      if (!(map = getMap(mech->mapindex)))
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

  if (xcode_obj->type == GTYPE_AUTO) {
    AUTO *const autopilot = (AUTO *)xcode_obj;

    int i;

    /* Commands and A* paths are restored before these derived caches. */
    autopilot->weaplist = NULL;
    for (i = 0; i < AUTO_PROFILE_MAX_SIZE; i++) {
      autopilot->profile[i] = NULL;
    }

    if (!autopilot->mymechnum ||
        !(autopilot->mymech = getMech(autopilot->mymechnum))) {
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
          game_object_location(btech_context_active()->database,
                               autopilot->mynum) == autopilot->mymechnum &&
          autopilot->commands &&
          doubly_linked_list_size(autopilot->commands) > 0 &&
          !mux_event_count_type_data(btech_context_active()->events,
                                     EVENT_AUTOCOM, autopilot))
        AUTO_COM(autopilot, AUTOPILOT_NC_DELAY);
      if (Gunning(autopilot))
        DoStartGun(autopilot);
    }
  }

  return 1;
}

void heartbeat_init();

void LoadSpecialObjects(void) {
  DbRef i;
  int id, brand;
  int type;

  init_xcode_tree();

  mux_event_initialize(btech_context_active()->events);
  mux_event_count_initialize();
  init_stat();
  initialize_partname_tables();
  /* The SQLite startup path still needs ANSI parser state for BTech output. */
  initialize_colorize();
  for (i = 0; MissileHitTable[i].key != -1; i++) {
    if (find_matching_vlong_part(MissileHitTable[i].name, NULL, &id, &brand))
      MissileHitTable[i].key = Weapon2I(id);
    else
      MissileHitTable[i].key = -2;
  }
  /* Loop through the entire database, and if it has the special */
  /* object flag, add it to our linked list. */
  DO_WHOLE_DB(btech_context_active()->database, i)
  if (is_hardcode(btech_context_active()->database, i) &&
      !is_going(btech_context_active()->database, i) &&
      !is_halted(btech_context_active()->database, i)) {
    type = WhichSpecialS(i);
    if (type >= 0) {
      if (SpecialObjects[type].datasize > 0)
        NewSpecialObject(i, type);
    } else
      c_hardcode(btech_context_active()->database, i); /* Reset the flag */
  }
  for (i = 0; i < (int)(NUM_SPECIAL_OBJECTS); i++) {
    InitSpecialHash(i);
    if (!SpecialObjects[i].updatefunc)
      SpecialObjects[i].updateTime = 0;
  }
  init_btechstats();
#ifdef BTMUX_PERSISTENCE_TESTING
  /* The integration fixture creates its initial SQLite special-state rows. */
  if (getenv("BTMUX_TEST_BTECH_BOOTSTRAP")) {
    heartbeat_init();
    return;
  }
#endif
  if (btech_persistence_load_special_state_path(
          btech_context_active()->configuration->database.gamedb) < 0) {
    exit(EXIT_FAILURE);
  }
  red_black_tree_walk(xcode_tree, WALK_INORDER, load_update2, NULL);
  red_black_tree_walk(xcode_tree, WALK_INORDER, load_update3, NULL);
  red_black_tree_walk(xcode_tree, WALK_INORDER, load_update4, NULL);
  red_black_tree_walk(xcode_tree, WALK_INORDER, load_autopilot_data, NULL);
  heartbeat_init();
}

static int UpdateSpecialObject_func(void *key, void *data, int depth,
                                    void *arg) {
  XCODE *const xcode_obj = data;

  if (!SpecialObjects[xcode_obj->type].updateTime)
    return 1;
  if ((btech_context_active()->clock->now %
       SpecialObjects[xcode_obj->type].updateTime))
    return 1;
  ((void (*)(DbRef, void *))SpecialObjects[xcode_obj->type].updatefunc)(
      (DbRef)key, xcode_obj);
  return 1;
}

/* This is called once a second for each special object */

/* Note the new handling for calls being done at <1second intervals,
   or possibly at >1second intervals */

void UpdateSpecialObjects(void) {
  static time_t lastrun = 0;

  const char *cmdsave;
  int i;
  int times = lastrun ? (btech_context_active()->clock->now - lastrun) : 1;

  if (times > 20)
    times = 20; /* Machine's hopelessly lagged,
                           we don't want to make it [much] worse */
  cmdsave = btech_context_active()->command_context->debug_command;
  for (i = 0; i < times; i++) {
    mux_event_run(btech_context_active()->events);
    btech_context_active()->command_context->debug_command =
        (char *)"< Generic hcode update handler>";
    red_black_tree_walk(xcode_tree, WALK_INORDER, UpdateSpecialObject_func,
                        NULL);
  }
  lastrun = btech_context_active()->clock->now;
  btech_context_active()->command_context->debug_command = cmdsave;
}

void *NewSpecialObject(long id, int type) {
  XCODE *xcode_obj = NULL;

  if (SpecialObjects[type].datasize) {
    xcode_obj = (XCODE *)calloc(1, SpecialObjects[type].datasize);
    if (!xcode_obj) {
      printf("Unable to calloc\n");
      exit(1);
    }
    xcode_obj->type = type;
    xcode_obj->size = SpecialObjects[type].datasize;

    if (SpecialObjects[type].allocfreefunc)
      ((void (*)(DbRef, void **, int))SpecialObjects[type].allocfreefunc)(
          id, (void **)&xcode_obj, SPECIAL_ALLOC);

    red_black_tree_insert(xcode_tree, (void *)id, xcode_obj);
  }

  return xcode_obj;
}

void CreateNewSpecialObject(DbRef player, DbRef key) {
  void *new;
  struct SpecialObjectStruct *typeOfObject;
  int type;
  char *str;

  str = silly_atr_get(key, A_XTYPE);
  if (!(str && *str)) {
    notify(BTECH_EVALUATION_CONTEXT, player,
           "You must first set the XTYPE using @xtype <object>=<type>");
    notify(BTECH_EVALUATION_CONTEXT, player,
           "Valid XTYPEs include: MECH, MECHREP, MAP, DEBUG, "
           "AUTOPILOT, TURRET.");
    notify(BTECH_EVALUATION_CONTEXT, player, "Resetting hardcode flag.");
    c_hardcode(btech_context_active()->database, key); /* Reset the flag */
    return;
  }

  /* Find the special objects */
  type = WhichSpecialS(key);
  if (type > -1) {
    /* We found the proper special object */
    typeOfObject = &SpecialObjects[type];
    if (typeOfObject->datasize) {
      new = NewSpecialObject(key, type);
      if (!new)
        notify(BTECH_EVALUATION_CONTEXT, player, "Memory allocation failure!");
    }
  } else {
    notify(BTECH_EVALUATION_CONTEXT, player, "That is not a valid XTYPE!");
    notify(BTECH_EVALUATION_CONTEXT, player,
           "Valid XTYPEs include: MECH, MECHREP, MAP, DEBUG, "
           "AUTOPILOT, TURRET.");
    notify(BTECH_EVALUATION_CONTEXT, player, "Resetting HARDCODE flag.");
    c_hardcode(btech_context_active()->database, key);
  }
}

void DisposeSpecialObject(DbRef player, DbRef key) {
  XCODE *xcode_obj;

  int i;
  struct SpecialObjectStruct *typeOfObject;

  xcode_obj = red_black_tree_find(xcode_tree, (void *)key);

  i = WhichSpecialS(key);
  if (i < 0) {
    notify(BTECH_EVALUATION_CONTEXT, player,
           "CRITICAL: Unable to free data, inconsistency somewhere. Please");
    notify(BTECH_EVALUATION_CONTEXT, player,
           "contact a wizard about this _NOW_!");
    return;
  }
  typeOfObject = &SpecialObjects[i];

  if (typeOfObject->datasize > 0 && WhichSpecial(key) != i) {
    notify(BTECH_EVALUATION_CONTEXT, player,
           "Semi-critical error has occured. For some reason the "
           "object's data differs\nfrom the data on the object. Please "
           "contact a wizard about this.");
    i = WhichSpecial(key);
  }
  if (xcode_obj) {
    if (typeOfObject->allocfreefunc)
      ((void (*)(DbRef, void **, int))typeOfObject->allocfreefunc)(
          key, (void **)&xcode_obj, SPECIAL_FREE);
    red_black_tree_delete(xcode_tree, (void *)key);
    mux_event_remove_data(btech_context_active()->events, xcode_obj);
    free(xcode_obj);
  } else if (typeOfObject->datasize > 0) {
    notify(BTECH_EVALUATION_CONTEXT, player,
           "This object is not in the special object DBASE.");
    notify(BTECH_EVALUATION_CONTEXT, player,
           "Please contact a wizard about this bug. ");
  }
}

void Dump_Mech(DbRef player, int type, char *typestr) {
  notify(BTECH_EVALUATION_CONTEXT, player,
         "Support discontinued. Bother a wiz if this bothers you.");
#if 0
	MECH *mech;
	char buff[100];
	int i, running = 0, count = 0;
	Node *temp;

	notify(BTECH_EVALUATION_CONTEXT, player, "ID    # STATUS      MAP #      PILOT #");
	notify(BTECH_EVALUATION_CONTEXT, player, "----------------------------------------");
	for(temp = TreeTop(xcode_tree); temp; temp = TreeNext(temp))
		if(WhichSpecial((i = NodeKey(temp))) == type) {
			mech = (MECH *) NodeData(temp);
			sprintf(buff, "#%5d %-8s    #%5d    #%5d", mech->mynum,
					!Started(mech) ? "SHUTDOWN" : "RUNNING", mech->mapindex,
					MechPilot(mech));
			notify(BTECH_EVALUATION_CONTEXT, player, buff);
			if(MechStatus(mech) & STARTED)
				running++;
			count++;
		}
	sprintf(buff, "%d %ss running out of %d %ss allocated.", running,
			typestr, count, typestr);
	notify(BTECH_EVALUATION_CONTEXT, player, buff);
	notify(BTECH_EVALUATION_CONTEXT, player, "Done listing");
#endif
}

void DumpMechs(DbRef player) { Dump_Mech(player, GTYPE_MECH, "mech"); }

void DumpMaps(DbRef player) {
  notify(BTECH_EVALUATION_CONTEXT, player,
         "Support discontinued. Bother a wiz if this bothers you.");
#if 0
	MAP *map;
	char buff[100];
	int j, count;
	Node *temp;

	notify(BTECH_EVALUATION_CONTEXT, player, "MAP #       NAME              X x Y   MECHS");
	notify(BTECH_EVALUATION_CONTEXT, player, "-------------------------------------------");
	for(temp = TreeTop(xcode_tree); temp; temp = TreeNext(temp))
		if(WhichSpecial(NodeKey(temp)) == GTYPE_MAP) {
			count = 0;
			map = (MAP *) NodeData(temp);
			for(j = 0; j < map->first_free; j++)
				if(map->mechsOnMap[j] != -1)
					count++;
			sprintf(buff, "#%5d    %-17.17s %3d x%3d       %d", map->mynum,
					map->mapname, map->map_width, map->map_height, count);
			notify(BTECH_EVALUATION_CONTEXT, player, buff);
		}
	notify(BTECH_EVALUATION_CONTEXT, player, "Done listing");
#endif
}

/***************** INTERNAL ROUTINES *************/
#ifdef FAST_WHICHSPECIAL
int WhichSpecial(DbRef key) {
  XCODE *xcode_obj;

  if (!is_good_obj(btech_context_active()->database, key))
    return -1;
  if (!is_hardcode(btech_context_active()->database, key))
    return -1;
  if (!(xcode_obj = red_black_tree_find(xcode_tree, (void *)key)))
    return -1;
  return xcode_obj->type;
}

static int WhichSpecialS(DbRef key)
#else
int WhichSpecial(DbRef key)
#endif
{
  int i;
  int returnValue = -1;
  char *str;

  if (!is_hardcode(btech_context_active()->database, key))
    return -1;
  str = silly_atr_get(key, A_XTYPE);
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

int IsMech(DbRef num) { return WhichSpecial(num) == GTYPE_MECH; }

int IsAuto(DbRef num) { return WhichSpecial(num) == GTYPE_AUTO; }

int IsMap(DbRef num) { return WhichSpecial(num) == GTYPE_MAP; }

/*** Support routines ***/
void *FindObjectsData(DbRef key) {
  return red_black_tree_find(xcode_tree, (void *)key);
}

char *center_string(char *c, int len) {
  static char buf[LBUF_SIZE];
  int l = strlen(c);
  int p, i;

  p = MAX(0, (len - l) / 2);
  for (i = 0; i < p; i++)
    buf[i] = ' ';
  strcpy(buf + p, c);
  return buf;
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

static void DoSpecialObjectHelp(DbRef player, char *type, int id, int loc,
                                int powerneeded, int objid, char *arg) {
  int i, j;
  MECH *mech = NULL;
  int pos[100][2];
  int count = 0, csho = 0;
  coolmenu *c = NULL;
  char buf[LBUF_SIZE];
  char *d;
  int dc;

  if (id == GTYPE_MECH)
    mech = getMech(loc);
  bzero(pos, sizeof(pos));
  for (i = 0; SpecialObjects[id].commands[i].name; i++) {
    if (!SpecialObjects[id].commands[i].func &&
        (SpecialObjects[id].commands[i].helpmsg[0] != '@' ||
         Have_MechPower(
             game_object_owner(btech_context_active()->database, player),
             powerneeded)))
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
        d = center_string(HELPMSG(pos[i][0]), 70);
        sim(tprintf("%s%s%s", "%cg", d, "%c"), CM_ONE);
      } else
        sim(tprintf("%s command listing: ", type), CM_ONE | CM_CENTER);
      for (j = pos[i][0] + (count == 1 ? 0 : 1); j < pos[i][0] + pos[i][1]; j++)
        if (SpecialObjects[id].commands[j].helpmsg[0] != '@' ||
            Have_MechPower(
                game_object_owner(btech_context_active()->database, player),
                powerneeded))
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
          if (count > 1)
            vsi(tprintf("%s%s%s", "%cg", center_string(HELPMSG(pos[i][0]), 70),
                        "%c"));
          for (j = pos[i][0] + (count == 1 ? 0 : 1); j < pos[i][0] + pos[i][1];
               j++)
            if (SpecialObjects[id].commands[j].helpmsg[0] != '@' ||
                Have_MechPower(
                    game_object_owner(btech_context_active()->database, player),
                    powerneeded))
              if (id != GTYPE_MECH ||
                  Can_Use_Command(mech, SpecialObjects[id].commands[j].flag))
                cut_apart_helpmsgs(&c, SpecialObjects[id].commands[j].name,
                                   HELPMSG(j), 37, 1);
        }
    }
  }
  sim(NULL, CM_ONE | CM_LINE);
  ShowCoolMenu(player, c);
  KillCoolMenu(c);
}

void InitSpecialHash(int which) {
  char *tmp, *tmpc;
  int i;
  char buf[MBUF_SIZE];

  hash_table_initialize(&SpecialCommandHash[which], 20 * HASH_FACTOR);
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
                   &SpecialCommandHash[which]);
  }
}

void handle_xcode(DbRef player, DbRef obj, int from, int to) {
  if (from == to)
    return;
  if (!to) {
    s_hardcode(btech_context_active()->database, obj);
    DisposeSpecialObject(player, obj);
    c_hardcode(btech_context_active()->database, obj);
  } else
    CreateNewSpecialObject(player, obj);
}

#define DEFAULT 0 /* Normal */
#define ANSI_START "\033["
#define ANSI_START_LEN 2
#define ANSI_END "m"
#define ANSI_END_LEN 1

struct color_entry {
  int bit;
  int negbit;
  char ltr;
  const char *string;
  char *sstring;
} color_table[] = {
    {0x0008, 7, 'n', ANSI_NORMAL, NULL},  {0x0001, 0, 'h', ANSI_HILITE, NULL},
    {0x0002, 0, 'i', ANSI_INVERSE, NULL}, {0x0004, 0, 'f', ANSI_BLINK, NULL},
    {0x0010, 0, 'x', ANSI_BLACK, NULL},   {0x0010, 0x10, 'l', ANSI_BLACK, NULL},
    {0x0020, 0, 'r', ANSI_RED, NULL},     {0x0040, 0, 'g', ANSI_GREEN, NULL},
    {0x0080, 0, 'y', ANSI_YELLOW, NULL},  {0x0100, 0, 'b', ANSI_BLUE, NULL},
    {0x0200, 0, 'm', ANSI_MAGENTA, NULL}, {0x0400, 0, 'c', ANSI_CYAN, NULL},
    {0x0800, 0, 'w', ANSI_WHITE, NULL},   {0, 0, 0, NULL, NULL}};

#define CHARS 256

char colorc_reverse[CHARS];

void initialize_colorize() {
  int i;
  char buf[20];
  char *c;

  c = buf + ANSI_START_LEN;
  for (i = 0; i < CHARS; i++)
    colorc_reverse[i] = DEFAULT;
  for (i = 0; color_table[i].string; i++) {
    colorc_reverse[(short)color_table[i].ltr] = i;
    strcpy(buf, color_table[i].string);
    buf[strlen(buf) - ANSI_END_LEN] = 0;
    color_table[i].sstring = strdup(c);
  }
}

#undef notify
char *colorize(DbRef player, char *from) {
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
        i = colorc_reverse[(short)*p];
      if (i == DEFAULT && *p != 'n')
        p--;
      color_wanted &= ~color_table[i].negbit;
      color_wanted |= color_table[i].bit;
    } else {
      if (color_wanted && is_ansi(btech_context_active()->database, player)) {
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
            strcpy(q, color_table[i].sstring);
            q += strlen(color_table[i].sstring);
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
  if (color_wanted && is_ansi(btech_context_active()->database, player)) {
    /* Generate efficient color string */
    strcpy(q, ANSI_START);
    q += ANSI_START_LEN;
    cnt = 0;
    for (i = 0; color_table[i].string; i++)
      if (color_wanted & color_table[i].bit &&
          color_table[i].bit != color_table[i].negbit) {
        if (cnt)
          *q++ = ';';
        strcpy(q, color_table[i].sstring);
        q += strlen(color_table[i].sstring);
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

void mecha_notify(DbRef player, char *msg) {
  char *tmp;

  tmp = colorize(player, msg);
  raw_notify(BTECH_EVALUATION_CONTEXT, player, tmp);
  free_lbuf(tmp);
}

void mecha_notify_except(DbRef loc, DbRef player, DbRef exception, char *msg) {
  DbRef first;

  if (loc != exception)
    notify_checked(BTECH_EVALUATION_CONTEXT, loc, player, msg,
                   (MSG_ME_ALL | MSG_F_UP | MSG_S_INSIDE | MSG_NBR_EXITS_A |
                    MSG_COLORIZE));
  DOLIST(btech_context_active()->database, first,
         game_object_contents(btech_context_active()->database, loc)) {
    if (first != exception) {
      notify_checked(BTECH_EVALUATION_CONTEXT, first, player, msg,
                     (MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE | MSG_COLORIZE));
    }
  }
}

void ResetSpecialObjects() {
  mux_event_run_by_type(btech_context_active()->events, EVENT_HIDE);
  mux_event_run_by_type(btech_context_active()->events, EVENT_BLINDREC);
}

MAP *getMap(DbRef d) {
  XCODE *xcode_obj;

  if (!(xcode_obj = red_black_tree_find(xcode_tree, (void *)d)))
    return NULL;
  if (xcode_obj->type != GTYPE_MAP)
    return NULL;
  return (MAP *)xcode_obj;
}

MECH *getMech(DbRef d) {
  XCODE *xcode_obj;

  if (!(is_good_obj(btech_context_active()->database, d)))
    return NULL;
  if (!(is_hardcode(btech_context_active()->database, d)))
    return NULL;
  if (!(xcode_obj = red_black_tree_find(xcode_tree, (void *)d)))
    return NULL;
  if (xcode_obj->type != GTYPE_MECH)
    return NULL;
  return (MECH *)xcode_obj;
}
