
/*
 * $Id: autopilot.h,v 1.4 2005/08/03 21:40:54 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Wed Oct 30 20:43:42 1996 fingon
 * Last modified: Sat Jun  6 19:56:42 1998 fingon
 *
 */

#pragma once

#include <time.h>

#include "btech_event.h"
#include "mech_events.h"
#include "mux/server/platform.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/red_black_tree.h"

#include "special_object.h"

typedef struct MuxEvent MuxEvent;

constexpr int AUTOPILOT_MEMORY =
    100; /* Number of command slots available to AI */
#define AUTOPILOT_MAX_ARGS                                                     \
  5 /* Max number of arguments for a given AI Command                          \
       Includes the command as the first argument */

/* The various flags for the AI */
#define AUTOPILOT_AUTOGUN                                                      \
  1 /* Is autogun enabled, ie: shoot what AI wants to                          \
     */
constexpr int AUTOPILOT_GUNZOMBIE = 2;
constexpr int AUTOPILOT_PILZOMBIE = 4;
constexpr int AUTOPILOT_ROAM = 8;   /* Are we roaming around */
constexpr int AUTOPILOT_LSENS = 16; /* Should change sensors or user set them */
constexpr int AUTOPILOT_CHASETARG = 32; /* Should chase the target */
#define AUTOPILOT_WAS_CHASE_ON                                                 \
  64 /* Was chasetarg on, for use with movement stuff */
constexpr int AUTOPILOT_SWARMCHARGE = 128;
constexpr int AUTOPILOT_ASSIGNED_TARGET =
    256; /* We given a specific target ? */

/* Various delays for the autopilot */
constexpr int AUTOPILOT_NC_DELAY =
    1; /* Generic command wait time before executing */

constexpr int AUTOPILOT_GOTO_TICK = 4; /* How often to check any GOTO event */
#define AUTOPILOT_LEAVE_TICK                                                   \
  6 /* How often to check if we've left                                        \
       the bay/hangar */
constexpr int AUTOPILOT_WAITFOE_TICK = 4;
constexpr int AUTOPILOT_PURSUE_TICK = 4;

constexpr int AUTOPILOT_FOLLOW_TICK = 4;
#define AUTOPILOT_FOLLOW_UPDATE_TICK                                           \
  10 /* When should we update the target hex */

#define AUTOPILOT_CHASETARG_UPDATE_TICK                                        \
  30 /* When should we update chasetarg                                        \
      */

#define AUTOPILOT_STARTUP_TICK                                                 \
  STARTUP_TIME + AUTOPILOT_NC_DELAY /* Delay for startup */

/* Defines for the autogun/autosensor stuff */
constexpr int AUTO_GUN_TICK = 1;          /* Every second */
constexpr float AUTO_GUN_MAX_HEAT = 6.0F; /* Last heat we let heat go to */
constexpr int AUTO_GUN_MAX_TARGETS = 100; /* Don't really use this one */
constexpr int AUTO_GUN_MAX_RANGE = 30;    /* Max range to look for targets */
constexpr int AUTO_GUN_UPDATE_TICK = 30;  /* When to look for a new target */
#define AUTO_GUN_IDLE_TICK                                                     \
  10 /* How often to call autogun when in idle mode                            \
      */
constexpr float AUTO_GUN_PHYSICAL_RANGE_MIN =
    3.0F; /* Min range at which to physically attack other targets if our main
             target is beyond this distance */
#define AUTO_PROFILE_TICK                                                      \
  180 /* How often to update the weapon profile                                \
         of the AI */
constexpr int AUTO_PROFILE_MAX_SIZE = 30; /* Size of the profile array */
constexpr int AUTO_SENSOR_TICK = 30;      /* Every 30 seconds or so */

/* Chase Target stuff for use with auto_set_chasetarget_mode */
typedef enum AutopilotChaseTargetMode : int {
  AUTO_CHASETARGET_ON = 1,       /* Turns it on and resets the values */
  AUTO_CHASETARGET_OFF = 2,      /* Turns it off */
  AUTO_CHASETARGET_REMEMBER = 3, /* Only if the AI remembers it being on */
  AUTO_CHASETARGET_SAVE = 4,     /* Turns it off and remembers it was on */
} AutopilotChaseTargetMode;

/* Roam Stuff */
/* Types of ROAMing */
constexpr int AUTO_ROAM_MAP = 1;  /* Roaming the whole map */
constexpr int AUTO_ROAM_SPOT = 2; /* Roaming a single area */

/* Tick values etc.. */
constexpr int AUTO_ROAM_TICK = 3;           /* How often to update */
constexpr int AUTO_ROAM_NEW_HEX_TICK = 100; /* How often to pick a new hex */
#define AUTO_ROAM_MAX_RADIUS                                                   \
  30 /* Max distance a person can make AI                                      \
        radius roam */
#define AUTO_ROAM_MAX_MAP_DISTANCE                                             \
  50 /* Max distance an AI will try to roam                                    \
        at a given time if its roaming the                                     \
        whole map */
#define AUTO_ROAM_MAX_ITERATIONS                                               \
  3 /* Max number of times AI will look                                        \
       for a new roam hex */

/*! \todo {Not sure what these are look into it} */
constexpr int AUTO_GOET = 15;
constexpr int AUTO_GOTT = 240;

void autopilot_resume_for_mech(Mech *mech);

void autopilot_autogun_log(const Autopilot *autopilot, const char *format, ...)
    __attribute__((format(printf, 2, 3)));

/*
 * Profile node structure
 */
typedef struct AutopilotWeaponProfile {
  int damage;
  int heat;
  RedBlackTree weaplist;
} AutopilotWeaponProfile;

/*
 * The Autopilot Structure
 */
typedef struct Autopilot {
  BtechSpecialObject xcode; /* XCODE base class field */

  DbRef mynum;          /* The AI's dbref number */
  Mech *mymech;         /* The AI's unit */
  DbRef mapindex;       /* The map the AI is currently on */
  DbRef mymechnum;      /* the dbref of the AI's mech */
  unsigned short speed; /* % of speed (1-100) that the AI should drive at */
  int ofsx, ofsy;       /* ? */

  unsigned char verbose_level; /* How talkative should the AI be */

  DbRef target;           /* The AI's current target */
  int target_score;       /* Current score of the AI's target */
  int target_threshold;   /* Threshold at which to change to another target */
  int target_update_tick; /* What autogun tick we currently at and should we
                             update */

  DbRef chase_target;        /* Current target we are chasing */
  int chasetarg_update_tick; /* When should we update chasetarg */

  int follow_update_tick; /* When should we update follow */

  /* Special AI flags */
  unsigned short flags;

  /* The autopilot's command list */
  DoublyLinkedList *commands;

  /* AI A* pathfinding stuff */
  DoublyLinkedList *astar_path;

  /* The AI's Weaplist for use with autogun */
  DoublyLinkedList *weaplist;

  /* Range Profile Array - for use with autogun */
  RedBlackTree profile[AUTO_PROFILE_MAX_SIZE];

  /* Max Range of AI's mech's weapons */
  int mech_max_range;

  /* Roam Stuff */
  unsigned char roam_type;  /* What type of ROAM are we doing */
  int roam_update_tick;     /* When should we update roam info */
  short roam_target_hex_x;  /* Target Hex - X Coord */
  short roam_target_hex_y;  /* Target Hex - Y Coord */
  int roam_anchor_hex_x;    /* Anchor Hex - X Coord */
  int roam_anchor_hex_y;    /* Anchor Hex - Y Coord */
  int roam_anchor_distance; /* Distance (in hexes) allowed from anchor hex */

  /*! \todo {Figure out if we need to keep these variables} */
  /* Temporary AI-pathfind data variables */
  int ahead_ok;
  int auto_cmode; /* 0 = Flee ; 1 = Close ; 2 = Maintain distances if possible
                   */
  int auto_cdist; /* Distance we're trying to maintain */
  int auto_goweight;
  int auto_fweight;
  int auto_nervous;

  int b_msc, w_msc, b_bsc, w_bsc, b_dan, w_dan;
  time_t last_upd;

} Autopilot;

static inline bool autopilot_is_gunning(const Autopilot *autopilot) {
  return autopilot->flags & AUTOPILOT_AUTOGUN;
}

static inline void autopilot_gunning_flag_set(Autopilot *autopilot) {
  autopilot->flags |= AUTOPILOT_AUTOGUN;
}

static inline void autopilot_gunning_stop(Autopilot *autopilot) {
  autopilot->flags &=
      (unsigned short)~(AUTOPILOT_AUTOGUN | AUTOPILOT_GUNZOMBIE);
}

static inline void autopilot_gunning_start(Autopilot *autopilot) {
  autopilot->flags |= AUTOPILOT_AUTOGUN;
  autopilot->flags &= (unsigned short)~AUTOPILOT_GUNZOMBIE;
}

static inline void autopilot_gunning_suspend(Autopilot *autopilot) {
  autopilot->flags &= (unsigned short)~AUTOPILOT_AUTOGUN;
  autopilot->flags |= AUTOPILOT_GUNZOMBIE;
}

static inline void autopilot_pilot_suspend(Autopilot *autopilot) {
  autopilot->flags |= AUTOPILOT_PILZOMBIE;
}

static inline void autopilot_gunning_resume(Autopilot *autopilot) {
  if (autopilot->flags & AUTOPILOT_GUNZOMBIE)
    autopilot_gunning_start(autopilot);
}

void autopilot_resume(Autopilot *autopilot);

static inline bool autopilot_has_assigned_target(const Autopilot *autopilot) {
  return autopilot->flags & AUTOPILOT_ASSIGNED_TARGET;
}

static inline void autopilot_assigned_target_set(Autopilot *autopilot,
                                                 bool assigned) {
  if (assigned)
    autopilot->flags |= AUTOPILOT_ASSIGNED_TARGET;
  else
    autopilot->flags &= (unsigned short)~AUTOPILOT_ASSIGNED_TARGET;
}

static inline bool autopilot_is_chasing_target(const Autopilot *autopilot) {
  return autopilot->flags & AUTOPILOT_CHASETARG;
}

static inline void autopilot_chasing_target_set(Autopilot *autopilot,
                                                bool chasing) {
  if (chasing)
    autopilot->flags |= AUTOPILOT_CHASETARG;
  else
    autopilot->flags &= (unsigned short)~AUTOPILOT_CHASETARG;
}

static inline bool autopilot_was_chasing_target(const Autopilot *autopilot) {
  return autopilot->flags & AUTOPILOT_WAS_CHASE_ON;
}

static inline void autopilot_chasing_target_memory_set(Autopilot *autopilot,
                                                       bool remembered) {
  if (remembered)
    autopilot->flags |= AUTOPILOT_WAS_CHASE_ON;
  else
    autopilot->flags &= (unsigned short)~AUTOPILOT_WAS_CHASE_ON;
}

/* command_node struct to store AI
 * commands for the AI command list */
typedef struct AutopilotArgumentList {
  char *values[AUTOPILOT_MAX_ARGS];
  size_t capacity;
} AutopilotArgumentList;

typedef struct AutopilotCommand {
  AutopilotArgumentList arguments;
  unsigned char argcount;                  /* Number of arguments */
  int command_enum;                        /* The ENUM value for the command */
  int (*ai_command_function)(Autopilot *); /* Function Pointer */
} AutopilotCommand;

/* A structure to store info about the various AI commands */
typedef struct AutopilotCommandDefinition {
  const char *name;
  int argcount;
  int command_enum;
  int (*ai_command_function)(Autopilot *);
} AutopilotCommandDefinition;

/* astar node Structure for the A star pathfinding */
typedef struct AutopilotPathNode {
  short x;
  short y;
  short x_parent;
  short y_parent;
  int g_score;
  int h_score;
  int f_score;
  int hexoffset;
} AutopilotPathNode;

/* Weaplist node for storing info about weapons on the mech */
typedef struct AutopilotWeapon {
  int weapon_number;
  int weapon_db_number;
  int section;
  int critical;
  int range_scores[AUTO_PROFILE_MAX_SIZE];
} AutopilotWeapon;

/* Target node for storing target data */
typedef struct AutopilotTarget {
  int target_score;
  DbRef target_dbref;
} AutopilotTarget;

/* Quick flags for use with the various autopilot
 * commands.  Check the ACOM array in autopilot_commands.c */
enum {
  GOAL_CHASETARGET, /* An extension of follow for chasetarget */
  GOAL_DUMBFOLLOW,
  GOAL_DUMBGOTO,
  GOAL_ENTERBASE, /* Revamp of enterbase so it keeps trying */
  GOAL_FOLLOW,    /* Uses the new Astar system */
  GOAL_GOTO,      /* Uses the new Astar system */
  GOAL_LEAVEBASE,
  GOAL_OLDGOTO, /* Old implementation of goto */
  GOAL_ROAM,    /* New version using Astar */
  GOAL_WAIT,    /* unimplemented */

  COMMAND_ATTACKLEG, /* unimplemented */
  COMMAND_AUTOGUN,   /* New version that has 3 options 'on', 'off' and 'target
                        dbref' */
  COMMAND_CHASEMODE, /* unimplemented */
  COMMAND_CMODE,     /* unimplemented */
  COMMAND_DROPOFF,
  COMMAND_EMBARK,
  COMMAND_ENTERBAY, /* unimplemented */
  COMMAND_JUMP,     /* unimplemented */
  COMMAND_LOAD,     /* unimplemented */
  COMMAND_PICKUP,
  COMMAND_REPORT,   /* unimplemented */
  COMMAND_ROAMMODE, /* unimplemented */
  COMMAND_SHUTDOWN,
  COMMAND_SPEED,
  COMMAND_STARTUP,
  COMMAND_STOPGUN,   /* unimplemented */
  COMMAND_SWARM,     /* unimplemented */
  COMMAND_SWARMMODE, /* unimplemented */
  COMMAND_UDISEMBARK,
  COMMAND_UNLOAD, /* unimplemented */
  AUTO_NUM_COMMANDS
};

/* Function Prototypes will go here */

/* From autopilot_core.c */
void auto_destroy_command_node(AutopilotCommand *node);
void auto_delcommand(DbRef player, void *data, const char *buffer);
void auto_addcommand(DbRef player, void *data, char *buffer);
void auto_listcommands(DbRef player, void *data, char *buffer);
void auto_eventstats(DbRef player, void *data, char *buffer);
void auto_set_comtitle(Autopilot *autopilot, Mech *mech);
void auto_init(Autopilot *autopilot, Mech *mech);
void auto_engage(DbRef player, void *data, const char *buffer);
void auto_disengage(DbRef player, void *data, const char *buffer);
void auto_goto_next_command(Autopilot *autopilot, int time);
char *auto_get_command_arg(Autopilot *autopilot, int command_number,
                           int arg_number);
int auto_get_command_enum(Autopilot *autopilot, int command_number);
void auto_newautopilot(DbRef key, void **data,
                       BtechSpecialLifecycleOperation operation);

/* From autopilot_commands.c */
void auto_cal_mapindex(BtechContext *context, Mech *mech);
void auto_set_chasetarget_mode(Autopilot *autopilot,
                               AutopilotChaseTargetMode mode);
void auto_command_startup(Autopilot *autopilot, Mech *mech);
void auto_command_shutdown(Autopilot *autopilot, Mech *mech);
void auto_command_pickup(Autopilot *autopilot, Mech *mech);
void auto_command_dropoff(Mech *mech);
void auto_command_speed(Autopilot *autopilot);
void auto_command_autogun(Autopilot *autopilot, Mech *mech);
void auto_command_chasetarget(Autopilot *autopilot);
void auto_command_embark(Autopilot *autopilot, Mech *mech);
void auto_command_udisembark(Mech *mech);
void auto_com_event(MuxEvent *muxevent);
void auto_astar_goto_event(MuxEvent *muxevent);
void auto_astar_follow_event(MuxEvent *muxevent);
void auto_dumbgoto_event(MuxEvent *muxevent);
void auto_dumbfollow_event(MuxEvent *muxevent);
void auto_leave_event(MuxEvent *muxevent);
void auto_enter_event(MuxEvent *muxevent);
void auto_command_roam(Autopilot *autopilot, Mech *mech);
void auto_astar_roam_event(MuxEvent *muxevent);
void speed_up_if_neccessary(Autopilot *autopilot, Mech *mech, int target_x,
                            int target_y, int bearing);
int slow_down_if_neccessary(Autopilot *autopilot, Mech *mech, float range,
                            int bearing, int target_x, int target_y);
void update_wanted_heading(Autopilot *autopilot, Mech *mech, int bearing);

/* From autopilot_ai.c */
int auto_astar_generate_path(Autopilot *autopilot, Mech *mech, int end_x,
                             int end_y);
void auto_destroy_astar_path(Autopilot *autopilot);
void auto_stop_pilot(Autopilot *autopilot);
void auto_heartbeat(Autopilot *autopilot);

/* From autopilot_autogun.c */
int SearchLightInRange(Mech *mech, BattleMap *map);
int PrefVisSens(Mech *mech, BattleMap *map, int slite, Mech *target);
void auto_sensor_event(Autopilot *muxevent);
void auto_gun_event(Autopilot *AUTOPILOT);
void auto_destroy_weaplist(Autopilot *autopilot);
void auto_update_profile_event(Autopilot *autopilot);

/* From autopilot_radio.c */
void auto_reply_event(MuxEvent *muxevent);
void auto_reply(Mech *mech, const char *buf);
void auto_parse_command(Autopilot *autopilot, Mech *mech, int chn,
                        char *buffer);

void auto_radio_command_autogun(Autopilot *autopilot, Mech *mech,
                                AutopilotArgumentList *args, int argc,
                                char *mesg);
void auto_radio_command_chasetarg(Autopilot *autopilot, Mech *mech,
                                  AutopilotArgumentList *args, int argc,
                                  char *mesg);
void auto_radio_command_dfollow(Autopilot *autopilot, Mech *mech,
                                AutopilotArgumentList *args, int argc,
                                char *mesg);
void auto_radio_command_dgoto(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg);
void auto_radio_command_dropoff(Autopilot *autopilot, Mech *mech,
                                AutopilotArgumentList *args, int argc,
                                char *mesg);
void auto_radio_command_embark(Autopilot *autopilot, Mech *mech,
                               AutopilotArgumentList *args, int argc,
                               char *mesg);
void auto_radio_command_enterbase(Autopilot *autopilot, Mech *mech,
                                  AutopilotArgumentList *args, int argc,
                                  char *mesg);
void auto_radio_command_follow(Autopilot *autopilot, Mech *mech,
                               AutopilotArgumentList *args, int argc,
                               char *mesg);
void auto_radio_command_goto(Autopilot *autopilot, Mech *mech,
                             AutopilotArgumentList *args, int argc, char *mesg);
void auto_radio_command_heading(Autopilot *autopilot, Mech *mech,
                                AutopilotArgumentList *args, int argc,
                                char *mesg);
void auto_radio_command_help(Autopilot *autopilot, Mech *mech,
                             AutopilotArgumentList *args, int argc, char *mesg);
void auto_radio_command_hide(Autopilot *autopilot, Mech *mech,
                             AutopilotArgumentList *args, int argc, char *mesg);
void auto_radio_command_jumpjet(Autopilot *autopilot, Mech *mech,
                                AutopilotArgumentList *args, int argc,
                                char *mesg);
void auto_radio_command_leavebase(Autopilot *autopilot, Mech *mech,
                                  AutopilotArgumentList *args, int argc,
                                  char *mesg);
void auto_radio_command_ogoto(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg);
void auto_radio_command_pickup(Autopilot *autopilot, Mech *mech,
                               AutopilotArgumentList *args, int argc,
                               char *mesg);
void auto_radio_command_position(Autopilot *autopilot, Mech *mech,
                                 AutopilotArgumentList *args, int argc,
                                 char *mesg);
void auto_radio_command_prone(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg);
void auto_radio_command_report(Autopilot *autopilot, Mech *mech,
                               AutopilotArgumentList *args, int argc,
                               char *mesg);
void auto_radio_command_reset(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg);
void auto_radio_command_sensor(Autopilot *autopilot, Mech *mech,
                               AutopilotArgumentList *args, int argc,
                               char *mesg);
void auto_radio_command_shutdown(Autopilot *autopilot, Mech *mech,
                                 AutopilotArgumentList *args, int argc,
                                 char *mesg);
void auto_radio_command_speed(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg);
void auto_radio_command_stand(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg);
void auto_radio_command_startup(Autopilot *autopilot, Mech *mech,
                                AutopilotArgumentList *args, int argc,
                                char *mesg);
void auto_radio_command_stop(Autopilot *autopilot, Mech *mech,
                             AutopilotArgumentList *args, int argc, char *mesg);
void auto_radio_command_sweight(Autopilot *autopilot, Mech *mech,
                                AutopilotArgumentList *args, int argc,
                                char *mesg);
void auto_radio_command_target(Autopilot *autopilot, Mech *mech,
                               AutopilotArgumentList *args, int argc,
                               char *mesg);

#include "ai_api.h"
#include "autogun_api.h"
#include "autopilot_api.h"
#include "autopilot_command_api.h"
#include "autopilot_commands_api.h"
