/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#pragma once

#include "equipment_types.h"
struct CriticalSlot {
  unsigned char brand;   /* Hold brand number, and damage (upper 4 bits) */
  unsigned char data;    /* Holds information like ammo remaining, etc */
  unsigned short type;   /* Type of item that this is a critical for */
  unsigned int firemode; /* Holds info like rear mount, ultra mode... */
  unsigned int ammomode; /* Holds info for the special ammo type in use */
  unsigned int weapDamageFlags; /* Holds the enhanced critical damage flags */
  short desiredAmmoLoc;         /* Location of the desired ammo bin */
  //    unsigned int recycle;   /* time when it will finish recycling */
};

/* Fire modes */
#define DESTROYED_MODE 0x00000001 /* the part is destroyed */
#define DISABLED_MODE 0x00000002  /* the part is disabled */
#define BROKEN_MODE                                                            \
  0x00000004 /* the part is part of a destroyed weapon/item                    \
              */
#define DAMAGED_MODE                                                           \
  0x00000008             /* the part is damaged from an enhanced critical */
#define ON_TC 0x00000010 /* (T) Set if the wepons mounted with TC */
#define REAR_MOUNT 0x00000020    /* (R) set if weapon is rear mounted */
#define HOTLOAD_MODE 0x00000040  /* (H) Weapon's being hotloaded */
#define HALFTON_MODE 0x00000080  /* Weapon is in halfton mode */
#define OS_MODE 0x00000100       /* (O) In weapon itself : Weapon's one-shot */
#define OS_USED 0x00000200       /* One-shot ammo _has_ been already used */
#define ULTRA_MODE 0x00000400    /* (U) set if weapon is in Ultra firing mode */
#define RFAC_MODE 0x00000800     /* (F) the weapon is set as a rapid fire AC */
#define GATTLING_MODE 0x00001000 /* (G) For Gattling MGs */
#define RAC_TWOSHOT_MODE 0x00002000  /* (2) RAC in two shot mode */
#define RAC_FOURSHOT_MODE 0x00004000 /* (4) RAC in four shot mode */
#define RAC_SIXSHOT_MODE 0x00008000  /* (6) RAC in six shot mode */
#define HEAT_MODE 0x00010000         /* (H) Toggle a flamer into heat mode */
#define WILL_JETTISON_MODE                                                     \
  0x00020000 /* Set if the slot will get destroyed during a backpack jettison  \
                (BSuits) */
#define IS_JETTISONED_MODE                                                     \
  0x00040000 /* Set if the slot has been jettisoned (BSuits) */
#define OMNI_BASE_MODE                                                         \
  0x00080000 /* Set if the slot part of the base config of an omni mech */
#define ROCKET_FIRED                                                           \
  0x00100000 /* Set if the slot's rocket launcher has been fired */

#define RAC_MODES (RAC_TWOSHOT_MODE | RAC_FOURSHOT_MODE | RAC_SIXSHOT_MODE)
#define FIRE_MODES                                                             \
  (HOTLOAD_MODE | ULTRA_MODE | RFAC_MODE | GATTLING_MODE | RAC_MODES |         \
   HEAT_MODE)

/* Ammo modes */
#define LBX_MODE 0x00000001     /* (L) set if weapon is firing LBX ammo */
#define ARTEMIS_MODE 0x00000002 /* (A) artemis compatible missiles/laucher */
#define NARC_MODE 0x00000004    /* (N) narc compatible missiles/launcher */
#define CLUSTER_MODE 0x00000008 /* (C) Set if weapon is firing cluster ammo */
#define MINE_MODE 0x00000010    /* (M) Set if weapon's firing mines */
#define SMOKE_MODE 0x00000020   /* (S) Set if weapon's firing smoke rounds */
#define INFERNO_MODE                                                           \
  0x00000040 /* (I) SRM's loaded with Inferno rounds (cause heat) */
#define SWARM_MODE 0x00000080  /* (W) LRM's loaded with Swarm rounds */
#define SWARM1_MODE 0x00000100 /* (1) LRM's loaded with Swarm1 rounds (FoF) */
#define INARC_EXPLO_MODE                                                       \
  0x00000200 /* (X) inarc launcher firing explosive pods */
#define INARC_HAYWIRE_MODE                                                     \
  0x00000400                      /* (Y) inarc launcher firing haywire pods */
#define INARC_ECM_MODE 0x00000800 /* (E) inarc launcher firing ecm pods */
#define INARC_NEMESIS_MODE                                                     \
  0x00001000 /* (Z) inarc launcher firing nemesis pods */
#define AC_AP_MODE                                                             \
  0x00002000 /* (R) autocannon firing armor piercing rounds                    \
              */
#define AC_FLECHETTE_MODE                                                      \
  0x00004000 /* (F) autocannon firing flechette rounds */
#define AC_INCENDIARY_MODE                                                     \
  0x00008000 /* (D) autocannon firing incendiary rounds */
#define AC_PRECISION_MODE                                                      \
  0x00010000                    /* (P) autocannon firing precision rounds */
#define STINGER_MODE 0x00020000 /* (T) AntiAir LRM */
#define AC_CASELESS_MODE                                                       \
  0x00040000 /* (U) autocannon firing caseless rounds                          \
              */
#define SGUIDED_MODE                                                           \
  0x00080000 /* (G) LRM's loaded with Semi-Guided rounds (benefits only if     \
                unit is lit by 'TAG' */
#define ATM_ER_MODE 0x00100000  /* (R) ATM's in Extended Range mode */
#define ATM_HE_MODE 0x00200000  /* (X) ATM's in High Explosive Mode */
#define MML_LRM_MODE 0x00400000 /* (#) MML in LRM Mode */

#define ARTILLERY_MODES (CLUSTER_MODE | MINE_MODE | SMOKE_MODE)
#define INARC_MODES                                                            \
  (INARC_EXPLO_MODE | INARC_HAYWIRE_MODE | INARC_ECM_MODE | INARC_NEMESIS_MODE)
#define MISSILE_MODES                                                          \
  (ARTEMIS_MODE | NARC_MODE | INFERNO_MODE | SWARM_MODE | SWARM1_MODE |        \
   STINGER_MODE | SGUIDED_MODE)
#define AC_MODES                                                               \
  (AC_AP_MODE | AC_FLECHETTE_MODE | AC_INCENDIARY_MODE | AC_PRECISION_MODE |   \
   AC_CASELESS_MODE)
#define ATM_MODES (ATM_ER_MODE | ATM_HE_MODE)
#define AMMO_MODES                                                             \
  (LBX_MODE | AC_MODES | MISSILE_MODES | INARC_MODES | ARTILLERY_MODES |       \
   ATM_MODES | MML_LRM_MODE)

/* Enhanced critical damage flags */
#define WEAP_DAM_MODERATE 0x00000001 /* +1 to hit */
#define WEAP_DAM_EN_FOCUS                                                      \
  0x00000002 /* Energy weapons: Focus misaligned. -1 damage, +1 BTH at med and \
                long range */
#define WEAP_DAM_EN_CRYSTAL                                                    \
  0x00000004 /* Energy weapons: Crystal damaged. +1 heat. Roll of 2 results in \
                ammo like explosion */
#define WEAP_DAM_BALL_BARREL                                                   \
  0x00000008 /* Ballistic weapons: Barrel damaged. Roll of 2 results in weapon \
                jam */
#define WEAP_DAM_BALL_AMMO                                                     \
  0x00000010 /* Ballistic weapons: Ammo feed damaged. Can not switch ammo      \
                type. Roll of 2 results in ammo explosion */
#define WEAP_DAM_MSL_RANGING                                                   \
  0x00000020 /* Missile weapons: Ranging system hit. +1 BTH at med and long    \
                ranges */
#define WEAP_DAM_MSL_AMMO                                                      \
  0x00000040 /* Missile weapons: Ammo feed damaged. Can not switch ammo type.  \
                Roll of 2 results in ammo explosion */

/* Structure for each of the 8 sections */
struct MechSection {
  unsigned char armor;    /* External armor value */
  unsigned char internal; /* Internal armor value */
  unsigned char rear;     /* Rear armor value */
  unsigned char armor_orig;
  unsigned char internal_orig;
  unsigned char rear_orig;
  char basetohit; /* Holds to hit modifiers for weapons in section */
  char config;    /* flags for CASE, etc. */
  char recycle;   /* after physical attack, set counter */
  unsigned short
      specials; /* specials for this section, like attached NARC pods, etc... */
  struct CriticalSlot criticals[NUM_CRITICALS]; /* Criticals */
};

/* Section configurations */
#define CASE_TECH 0x01         /* section has CASE technology */
#define SECTION_DESTROYED 0x02 /* section has been destroyed */
#define SECTION_BREACHED 0x04  /* section has been exposed to vacuum */
#define SECTION_FLOODED                                                        \
  0x08            /* section has been flooded with water - Kipsta. 8/3/99 */
#define AXED 0x10 /* arm was used to axe/sword someone */
#define STABILIZERS_DESTROYED                                                  \
  0x20 /* vehicle only. Double attacker mod for weapons from the section */
#define CASEII_TECH 0x40 /* section has CASE II technology */

/* Section specials */
#define NARC_ATTACHED 0x00000001 /* set if mech has a NARC beacon attached. */
#define INARC_HOMING_ATTACHED                                                  \
  0x00000002 /* set if mech has an iNARC homing beacon attached. */
#define INARC_HAYWIRE_ATTACHED                                                 \
  0x00000004 /* set if mech has an iNARC haywire beacon attached. */
#define INARC_ECM_ATTACHED                                                     \
  0x00000008 /* set if mech has an iNARC ecm beacon attached. */
#define INARC_NEMESIS_ATTACHED                                                 \
  0x00000010 /* set if mech has an iNARC nemesis beacon attached. */
#define CARRYING_CLUB 0x00000020 /* carrying a club in this location */

/* ground combat types */
#define CLASS_MECH 0
#define CLASS_VEH_GROUND 1
#define CLASS_VEH_NAVAL 3

/* Air types */
#define CLASS_VTOL 2
#define CLASS_SPHEROID_DS 4 /* Spheroid DropShip */
#define CLASS_AERO 5
#define CLASS_MW 6 /* Ejected MechWarrior */
#define CLASS_DS 7 /* AeroDyne DropShip */
#define CLASS_BSUIT 8
#define CLASS_LAST 8

#define DropShip(a) ((a) == CLASS_DS || (a) == CLASS_SPHEROID_DS)
#define IsDS(m) (DropShip(MechType(m)))

/* ground movement types */
#define MOVE_BIPED 0
#define MOVE_QUAD 8
#define MOVE_TRACK 1
#define MOVE_WHEEL 2
#define MOVE_HOVER 3
#define MOVE_HULL 5
#define MOVE_FOIL 6
#define MOVE_SUB 9

/* Air movenement types */
#define MOVE_VTOL 4
#define MOVE_FLY 7

#define MOVE_NONE 10 /* Stationary, for one reason or another */

#define MOVENEMENT_LAST 10

/* Mech Preferences list */
#define MECHPREF_PKILL 0x00000001  /* Kill MWs anyway */
#define MECHPREF_SLWARN 0x00000002 /* Warn when lit by slite */
#define MECHPREF_AUTOFALL                                                      \
  0x00000004 /* Jump off cliffs (don't try to avoid)                           \
              */
#define MECHPREF_NOARMORWARN                                                   \
  0x00000008 /* Don't warn when armor is getting low */
#define MECHPREF_NOAMMOWARN                                                    \
  0x00000010 /* Don't warn when ammo is getting low                            \
              */
#define MECHPREF_STANDANYWAY                                                   \
  0x00000020                           /* Try to stand even when BTH too high */
#define MECHPREF_AUTOCON_SD 0x00000040 /* Autocon on non-started units */
#define MECHPREF_NOFRIENDLYFIRE 0x00000080 /* Disallow firing on teammates */
#define MECHPREF_TURNMODE 0x00000100 /* Tight or Normal for Maneuvering Ace */
#define MECHPREF_BTHDEBUG                                                      \
  0x00000200 /* Show BTH Debug or not (Can Get Spammy) */
