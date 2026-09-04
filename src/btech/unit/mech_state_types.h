/* Defines BattleTech unit state types. */

#pragma once

#include <time.h>

#include "mech_status_types.h"
#include "mux/objects/db.h"
#include "section_types.h"
typedef struct {
  char mech_name[31]; /* Holds the 30 char ID for the mech */
  char mech_type[25]; /* Holds the mechref for the mech */
  char unit_era[25];  /* Era Per 'Catalyst Master Unit list' of Unit */
  char unit_tro[25];  /* TRO/RecordSheet where mech is from */
  char type;          /* The type of this unit */
  char move;          /* The movement type of this unit */
  char tac_range;     /* Tactical range for sensors */
  char lrs_range;     /* Long range for sensors */
  char scan_range;    /* Range for scanners */
  char numsinks;      /* number of heatsinks (also engine */
  int hsengoverride;  /* # of Heatsinks in Engine Override. Default to zero */
  char computer;      /* Partially replaces tac/lrs/scan/radiorange */
  char radio;
  unsigned char radioinfo;
  /* crits ( - from heatsinks) ) */
  char si;      /* Structural integrity of a craft */
  char si_orig; /* maximum struct. int */

  short radio_range; /* Can read/write comfortably at that distance */

  struct MechSection sections[NUM_SECTIONS]; /* armor */
  int fuel;                                  /* Fuel left */
  int fuel_orig;                             /* Fuel tank capacity */

  int tons;      /* How much I weigh */
  int walkspeed; /* Future expansion to do speed correctly */
  int runspeed;
  float maxspeed;          /* Maxspeed (running) in KPH */
  float template_maxspeed; /* we should read this in */

  int cargospace; /* Assigned cargo space * 100 for half and quarter tons */
  char targcomp;  /* Targeting comp mode. */
  char unused_char[3];
  char carmaxton; /* Max Tonnage variable for carrier sizing */
} MechDefinitionState;

typedef struct MechDamageRecord {
  int amount;
  time_t occured_at;
  struct MechDamageRecord *next;
  DbRef attacker_num;
  int counted;
} MechDamageRecord;

typedef struct {
  char jumptop;        /* How many MPs we've left for vertical stuff? */
  char aim;            /* section of target aimed at */
  char basetohit;      /* total to hit modifiers from critical hits */
  char pilotskillbase; /* holds constant skills mods */
  char engineheat;     /* +5 per critical hit there */
  char masc_value;     /* MASC roll .. updated up/down as needed */
  char aim_type;       /* Type we aim at */

  char sensor[2];       /* Primary mode, secondary mode */
  Byte fire_adjustment; /* For artillery mostly */
  char vis_mod;         /* Should be in range of 0 to 100 ; basically, this
                           is used as _base_ of random element in each sensor type,
                           altered         once every heat update (and when mech's
                           sensor mode         changes) */
  char chargetimer;     /* # of movement ticks since 'charge' command */
  float chargedist;     /* # of hexes moved since 'charge' command */
  char staggerstamp;    /* When in last turn this 'mech staggered */

  int mech_prefs;            /* Mech preferences */
  short jumplength;          /* in real coords (for jump and goto) */
  short goingx, goingy;      /* in map coords (for jump and goto) */
  short desiredfacing;       /* You are turning if this != facing */
  short angle;               /* For DS / Aeros */
  short jumpheading;         /* Jumping head */
  short targx, targy, targz; /* in map coords, target squares */
  short turretfacing;        /* Jumping head */
  short turndamage;          /* holds damge taken in 5 sec interval */
  short lateral;             /* Quad lateral move mode */
  short num_seen;            /* Number of enemies seen */
  short lx, ly;

  DbRef chgtarget; /* My CHARGE target */
  DbRef dfatarget; /* My DFA target */
  DbRef target;    /* My default target */
  DbRef swarming;  /* Swarm target */
  DbRef swarmedby; /* Who's swarming/mounting us */
  DbRef carrying;  /* Who are we lugging about? */
  DbRef spotter;   /* Who's spotting for us? */

  float heat;       /* Heat index */
  float weapheat;   /* Weapon heat factor-> see manifesto */
  float plus_heat;  /* how much heat I am producing */
  float minus_heat; /* how much heat I can dissipate */

  float startfx, startfy; /* in real coords (for jump and goto) */
  float startfz, endfz;   /* startstuff's also aeros' speed */
  float verticalspeed;    /* VTOL vertical speed in KPH */
  float speed;            /* Speed in KPH */
  float desired_speed;    /* Desired speed in KPH */
  float jumpspeed;        /* Jumping distance or current height in km */

  MechCritStatus critstatus; /* see mech_status_types.h */
  MechStatus status;         /* see mech_status_types.h */
  MechStatus2 status2;       /* see mech_status_types.h */
  int specials;              /* see key below */
  int specials2;             /* More tech specials */
  MechSpecialsStatus
      specialsstatus; /* status element specials, like ECM, etc... */
  MechTankCritStatus
      tankcritstatus; /* status element for crits that are specific to vehicles.
                         see mech_status_types.h */

  time_t last_weapon_recycle; /* This updated only on 'as needed' basis ;
                                 basically, all weapon recycling events
                                 compare the current time to the
                                 last_weapon_recycle, and send recycled-messages
                                 for all recycled weapons. */
  int cargo_weight;           /* How much stuff do we have? */

  /* BTHRandomization stuff (rok) ;) */
  int lastrndu;
  int rnd;

  int last_ds_msg; /* Used for DS-spam */
  int boom_start;  /* Used for Stackpole-effect */

  int maxfuel;       /* How much fuel fits to this thing anyway? */
  int lastused;      /* Idle timeout thing */
  int cocoon;        /* OOD cocoon */
  int commconv;      /* Evil magic related to commconv, p1 */
  int commconv_last; /* Evil magic related to commconv, p2 */
  int onumsinks;     /* Original HS (?) */
  int disabled_hs;   /* Disabled (on purpose, not destroyed) HS */
  DbRef autopilot_num;
  int heatboom_last;
  time_t sspin; /* Start of aero spin */
  int can_see;
  int row; /* _Own_ weight */
  int rcw; /* _Carried_ weight */
  float rspd;
  int erat;
  int per;
  int wxf;
  int last_startup;        /* timestamp of last 'startup' */
  int maxsuits;            /* Maximum number of bsuits in this unit */
  int infantry_specials;   /* Infantry related specials */
  char scharge_value;      /* Supercharger roll .. updated up/down as needed */
  int stagger_damage;      /* Damage for Stagger MkII */
  int last_stagger_notify; /* The level that we were last notified of a stagger
                            */
  MechCritStatus2 critstatus2; /* Starting to fill up. More CritStatus */
  float
      xpmod; /* Used to modify XP values per unit. Will default loading to 1 */
  int shots_fired;      /* Record how many shots we fired */
  int shots_hit;        /* Record how many shots we hit */
  int shots_missed;     /* Record how many shots we missed */
  int damage_taken;     /* Record how much damage we took */
  int damage_inflicted; /* Record how much damage we inflicted */
  int units_killed;     /* Record how many units we killed */
  struct MechDamageRecord
      *stagger_damage_list; /* The damage we've taken, in linked list form, so
                             we can calc damages - JF */
  time_t last_stagger_check;

} MechRuntimeState;

typedef struct {
  char pilotstatus;     /* damage pilot has taken */
  float hexes_walked;   /* Hexes walked counter */
  short facing;         /* 0-359.. */
  short x, y, z;        /* hex quantized x,y,z on the map in MP (hexes) */
  short last_x, last_y; /* last hex entered */
  float fx, fy, fz;     /* exact x, y and z on the map */
  int team;             /* Only for internal use */
  int unusable_arcs; /* Horrid kludge for disallowing use of some arcs' guns */
  int stall;         /* is this mech in a repair stall? */
  DbRef pilot;       /* My pilot */
  DbRef bay[NUM_BAYS];
  DbRef turret[NUM_TURRETS];
} MechPositionState;

typedef struct {
  char c3_chan_title[CHTITLELEN + 1];  /* applies to C3 and C3i */
  DbRef c3i_network[C3I_NETWORK_SIZE]; /* other mechs in the C3i network */
  int w_c3i_network_size;              /* Current size of our network */
  DbRef c3_network[C3_NETWORK_SIZE];   /* The whole network. We're sacrificing
                                         memory for speed. */
  int w_c3_network_size;               /* Current size of the C3Network */
  int w_total_c3_masters;              /* How many masters are on this mech? */
  int w_working_c3_masters; /* How many working masters are on this mech? */
  int c3_freq_mode;         /* applies to C3 and C3i */
  DbRef tag_target;         /* dbref of the target we're tagging */
  DbRef tagged_by;          /* dbref of the person tagging us */
} MechNetworkState;
