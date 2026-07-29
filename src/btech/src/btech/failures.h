
/* Brand level modifiers and failure resultant data here after included.
   Failure.h
   Created By: Nim
   Dated:      9 - 21 - 96

   Parts copyright (c) 2002 Thomas Wouters

   $Id: failures.h,v 1.1.1.1 2005/01/11 21:18:07 kstevens Exp $
   Last modified: Sat Jun  6 20:27:26 1998 fingon
 */

#pragma once

#include "p.failures.h"

#define IsAutocannon(a) (MechWeapons[a].type == TAMMO)
#define IsEnergy(a) (MechWeapons[a].type == TBEAM)
/*#define IsFlamer(a) (MechWeapons[a].type==TBEAM && \
        strstr(MechWeapons[a].name, "Flamer")) */

/* these are types of modifiers */
#define HEAT 1
#define RANGE 2
#define DAMAGE 3
#define POWER_SPIKE 4
#define WEAPON_JAMMED 5
#define WEAPON_DUD 6
#define CRAZY_MISSILES 7

#define FAIL_STATIC 1

/* these are catagories of damage */
#define FAIL_NONE 0
#define FAIL_JAMMED 1
#define FAIL_SHORTED 2
#define FAIL_DUD 3
#define FAIL_EMPTY 4
#define FAIL_DESTROYED 5
#define FAIL_AMMOJAMMED 6
#define FAIL_AMMOCRITJAMMED 7

struct brand_data {
  char *name;
  short level;
  int success;
  int modifier;
};

struct failure_data {
  char *message;
  int data; /* things like percent to alter */
  void (*func)(MECH *, int, int, int, int, int, int *, int *);
  int type;
  int flag;
};

/*  Brand keys
   1 - This is absolute crap.
   2 - This is low end.
   3 - This is average.
   4 - These are supieror parts.
   5 - These are EXTREMELY RARE and EXTREMELY reliable
 */

#ifndef _FAILURES_C

extern struct brand_data brands[];
extern struct failure_data failures[];
#else
struct brand_data brands[] = {
    {"Lords", 1, 80, -40}, /* Energy weapons */
    {"Hesperus", 2, 90, -20},
    {"Martell", 3, 95, 0},
    {"Magna", 4, 100, 20},
    {"Agra", 5, 101, 40},

    {"Luxor", 1, 80, -40}, /* Autocannons */
    {"SperryBrowning", 2, 90, -20},
    {"Oriente", 3, 95, 0},
    {"Deprus", 4, 100, 20},
    {"Armstrong", 5, 101, 40},

    {"Coventry", 1, 80, -40}, /* Missiles */
    {"Shannon", 2, 90, -20},
    {"Bical", 3, 95, 0},
    {"Holly", 4, 100, 20},
    {"Telos", 5, 101, 40},

    {"Pynes", 1, 80, -40}, /* Flamers */
    {"Hotshot", 2, 90, -20},
    {"Firestorm", 3, 95, 0},
    {"Purity", 4, 100, 20},
    {"Ventra", 5, 101, 40},

    {"Dalban", 1, 80, -40}, /* Computers */
    {"Hartford", 2, 90, -20},
    {"Garet", 3, 95, 0},
    {"Ares", 4, 100, 20},
    {"Tek", 5, 101, 40},

    {"Duoteck", 1, 80, -40}, /* Radios */
    {"CeresCom", 2, 90, -20},
    {"Achernar", 3, 95, 0},
    {"Tek", 4, 100, 20},
    {"Iriad", 5, 101, 40},
};

#define REQ_HEAT 1
#define REQ_TARGET 2
#define REQ_TAC 3
#define REQ_LRS 4
#define REQ_SCANNERS 5
#define REQ_COMPUTER 6
#define REQ_RADIO 7

struct failure_data failures[] = {
#define ENERGY_INDEX 0
    /* Energy Weapons - 0 */

    {"[fg=red bold]Your weapon fails to charge properly![reset]", 15,
     FailureWeaponDamage, FAIL_NONE, 0},
    {"[fg=red bold]Your weapon fails to charge properly![reset]", 30,
     FailureWeaponDamage, FAIL_NONE, 0},
    {"[fg=red bold]Your weapon fails to charge properly![reset]", 45,
     FailureWeaponDamage, FAIL_NONE, 0},
    {"[fg=red bold]Failure in the weapon's cooling system ; too much heat "
     "produced![reset]",
     30, FailureWeaponHeat, FAIL_NONE, REQ_HEAT},
    {"[fg=red bold]Odd energy reading from the weapon ; It seems to have gone "
     "offline![reset]",
     0, FailureWeaponSpike, FAIL_SHORTED, 0},
    {"[fg=red bold]Weapon melts down![reset]", 0, FailureWeaponSpike,
     FAIL_SHORTED, 0},

/* Autocannons - 6 */
#define AC_INDEX 6

    {"[fg=red bold]Round misfires! .. and spirals off![reset]", 0,
     FailureWeaponDud, FAIL_NONE, 0},
    {"[fg=red bold]Round not fired!  Dud![reset]", 0, FailureWeaponDud,
     FAIL_DUD, 0},
    {"[fg=red bold]Weapon JAMS... clearing![reset]", 0, FailureWeaponJammed,
     FAIL_JAMMED, 0},
    {"[fg=red bold]Failure in the weapon's cooling system, too much heat "
     "produced![reset]",
     20, FailureWeaponHeat, FAIL_NONE, REQ_HEAT},
    {"[fg=red bold]Failure in the weapon's cooling system, too much heat "
     "produced![reset]",
     40, FailureWeaponHeat, FAIL_NONE, REQ_HEAT},
    {"[fg=red bold]Round not fired!  STUCK in chamber![reset]", 0,
     FailureWeaponDud, FAIL_DUD, 0},

/* Missiles - 12 */
#define MISSILE_INDEX 12

    {"[fg=red bold]Rack jams, attemping to clear![reset]", 0,
     FailureWeaponJammed, FAIL_JAMMED, 0},
    {"[fg=red bold]Some of your missiles veer off course![reset]", 20,
     FailureWeaponMissiles, FAIL_NONE, 0},
    {"[fg=red bold]Some of your missiles veer off course![reset]", 40,
     FailureWeaponMissiles, FAIL_NONE, 0},
    {"[fg=red bold]Guidance Failure!  All missile veer off course![reset]", 100,
     FailureWeaponMissiles, FAIL_NONE, 0},
    {"[fg=red bold]Weapon power spikes.. attempting to restart![reset]", 0,
     FailureWeaponSpike, FAIL_SHORTED, 0},
    {"[fg=red bold]Weapon power spikes.. Electronics fused!![reset]", 0,
     FailureWeaponSpike, FAIL_SHORTED, 0},

/* Flamer - 18 */
#define FLAMMER_INDEX 18

    {"[fg=red bold]Gel line clogs, sending pressure through it now![reset]", 0,
     FailureWeaponJammed, FAIL_JAMMED, 0},
    {"[fg=red bold]Electric ignition shorts out! Restarting![reset]", 0,
     FailureWeaponSpike, FAIL_SHORTED, 0},
    {"[fg=red bold]Fuel leaks on the chassis and ignites![reset]", 100,
     FailureWeaponHeat, FAIL_NONE, 0},

    {"[fg=red bold]Fuel at critical point!! Shutting down weapon to vent "
     "heat![reset]",
     0, FailureWeaponSpike, FAIL_SHORTED, 0},
    {"[fg=red bold]Ejection nozzle gums up!  Please wait while pressure is "
     "applied![reset]",
     0, FailureWeaponJammed, FAIL_JAMMED, 0},
    {"[fg=red bold]Fuel canisters explode!  No fuel left to burn![reset]", 0,
     FailureWeaponSpike, FAIL_EMPTY, 0},

/* Computer - 24 */
#define COMPUTER_INDEX 24

    {"[fg=red bold]Computer Glitch!  Target lost, please reacquire![reset]", 0,
     FailureComputerTarget, FAIL_NONE, REQ_TARGET},
    {"[fg=red bold]Tactical shorts out! Fixing .. Please stand by.[reset]", 1,
     FailureComputerScanner, FAIL_NONE, REQ_TAC},
    {"[fg=red bold]Long Range Sensors short out! .. Fixing .. Please stand "
     "by.[reset]",
     2, FailureComputerScanner, FAIL_NONE, REQ_LRS},
    {"[fg=red bold]Scanners short out! Fixing .. Please stand by.[reset]", 4,
     FailureComputerScanner, FAIL_NONE, REQ_SCANNERS},
    {"[fg=red bold]A sudden *SNAP* echos in your cockpit then all your "
     "displays "
     "die![reset]",
     7, FailureComputerScanner, FAIL_NONE, REQ_SCANNERS},
    {"[fg=red bold]You hear a loud *SNAP* *CRACKLE* and then everything "
     "powers "
     "down![reset]",
     0, FailureComputerShutdown, FAIL_NONE, REQ_COMPUTER},

/* Radio - 30 */
#define RADIO_INDEX 30
    {"none", 50, FailureRadioStatic, FAIL_NONE, 0},
    {"none", 70, FailureRadioStatic, FAIL_NONE, 0},
    {"[fg=red bold]Your readouts register a power loss in your radio![reset]",
     15, FailureRadioRange, FAIL_NONE, REQ_RADIO},
    {"[fg=red bold]Your readouts register a power loss in your radio![reset]",
     30, FailureRadioRange, FAIL_NONE, REQ_RADIO},
    {"[fg=red bold]Your radio suddenly shorts out! Please wait for backup to "
     "come "
     "online![reset]",
     0, FailureRadioShort, FAIL_NONE, REQ_RADIO},
    {"[fg=red bold]Your entire radio system suddenly shorts out![reset]", 0,
     FailureRadioShort, FAIL_NONE, REQ_RADIO}};

#endif
