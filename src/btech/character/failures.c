
/* This is the code that runs the parts failures.
   Written by: Nim
   9-28-96

   Parts copyright (c) 2000-2002 Thomas Wouters

 */

/*
 * $Id: failures.c,v 1.1.1.1 2005/01/11 21:18:07 kstevens Exp $
 * Last modified: Sat Jun  6 21:43:52 1998 fingon
 */

#include <stdio.h>
#include <string.h>

#include "btech_event.h"
#include "command_handlers_api.h"
#include "failures.h"
#include "failures_api.h"
#include "mech.h"
#include "mech_events.h"
#include "mech_macros.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_startup_api.h"
#include "mech_utils_api.h"
#include "weapon_settings.h"

extern const int num_def_weapons;

struct PartBrand brands[] = {
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

struct PartFailure failures[] = {
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

int GetBrandIndex(int type) {
  if (type == -1)
    return COMPUTER_INDEX;
  if (type == -2)
    return RADIO_INDEX;
  if (IsWeapon(type))
    if (type < I2Weapon(num_def_weapons)) {
      type = Weapon2I(type);
      if (MechWeapons[type].special & PCOMBAT)
        return -1;
      if (IsFlamer(type))
        return FLAMMER_INDEX;
      if (IsEnergy(type))
        return ENERGY_INDEX;
      if (IsAutocannon(type))
        return AC_INDEX;
      if (IsMissile(type))
        return MISSILE_INDEX;
      return -1;
    }
  return -1;
}

char *GetPartBrandName(int type, int level) {
  int i;

  if (!level)
    return NULL;
  i = GetBrandIndex(type);
  if (i < 0)
    return NULL;
  return brands[i * 5 / 6 + level - 1].name;
}

#define Conv(mech, section, critical)                                          \
  (GetBrandIndex(GetPartType(mech, section, critical)) - 1)

void FailureRadioStatic(Mech *mech, int weapnum, int weaptype, int section,
                        int critical, int roll, int *modifier, int *type) {
  int mod = failures[GetBrandIndex(-2) + roll - 1].data;

  *modifier = mod;
  *type = FAIL_STATIC;
}

static void mech_rrec_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long val = (long)e->data2;

  MechRadioRange(mech) += val;
  if (!Destroyed(mech) && val == MechRadioRange(mech))
    mech_notify(mech, MECHALL, "Your radio is now operational again.");
}

static void mech_srec_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long val = (long)e->data2;
  int vt = val / 256;

  switch (vt) {
  case 0:
    MechTacRange(mech) = val;
    if (!Destroyed(mech))
      mech_notify(mech, MECHALL,
                  "Your tactical scanners are operational again.");
    break;
  case 1:
    MechLRSRange(mech) = val;
    if (!Destroyed(mech))
      mech_notify(mech, MECHALL,
                  "Your long-range scanners are operational again.");
    break;
  case 2:
    MechScanRange(mech) = val;
    if (!Destroyed(mech))
      mech_notify(mech, MECHALL, "Your scanners are operational again.");
    break;
  }
}

void FailureRadioShort(Mech *mech, int weapnum, int weaptype, int section,
                       int critical, int roll, int *modifier, int *type) {
  mech_event_schedule(
      mech, EVENT_MRECOVERY, mech_rrec_event,
      btech_random_range(mech->xcode.context, 30,
                         btech_random_range(mech->xcode.context, 40, 200)),
      (long)MechRadioRange(mech));
  MechRadioRange(mech) = 0;
}

void FailureRadioRange(Mech *mech, int weapnum, int weaptype, int section,
                       int critical, int roll, int *modifier, int *type) {
  int mod = failures[GetBrandIndex(-2) + roll - 1].data;

  mod = MIN(MechRadioRange(mech) - 1, mod);
  mech_event_schedule(
      mech, EVENT_MRECOVERY, mech_rrec_event,
      btech_random_range(mech->xcode.context, 30,
                         btech_random_range(mech->xcode.context, 40, 200)),
      (long)mod);
  MechRadioRange(mech) -= mod;
}

void FailureComputerShutdown(Mech *mech, int weapnum, int weaptype, int section,
                             int critical, int roll, int *modifier, int *type) {
  if (Started(mech))
    mech_shutdown(mech->mynum, mech, "");
}

void FailureComputerScanner(Mech *mech, int weapnum, int weaptype, int section,
                            int critical, int roll, int *modifier, int *type) {
  int tmp = failures[GetBrandIndex(-1) + roll - 1].data;

  switch (tmp) {
  case 1:
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range(mech->xcode.context, 30,
                           btech_random_range(mech->xcode.context, 40, 200)),
        (long)MechTacRange(mech));
    MechTacRange(mech) = 0;
    break;
  case 2:
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range(mech->xcode.context, 30,
                           btech_random_range(mech->xcode.context, 40, 200)),
        (long)(MechLRSRange(mech) + 256));
    MechLRSRange(mech) = 0;
    break;
  case 4:
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range(mech->xcode.context, 30,
                           btech_random_range(mech->xcode.context, 40, 200)),
        (long)(MechScanRange(mech) + 512));
    MechScanRange(mech) = 0;
    break;
  case 7:
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range(mech->xcode.context, 30,
                           btech_random_range(mech->xcode.context, 40, 200)),
        (long)MechTacRange(mech));
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range(mech->xcode.context, 30,
                           btech_random_range(mech->xcode.context, 40, 200)),
        (long)(MechLRSRange(mech) + 256));
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range(mech->xcode.context, 30,
                           btech_random_range(mech->xcode.context, 40, 200)),
        (long)(MechScanRange(mech) + 512));
    MechTacRange(mech) = 0;
    MechLRSRange(mech) = 0;
    MechScanRange(mech) = 0;
    break;
  }
}

void FailureComputerTarget(Mech *mech, int weapnum, int weaptype, int section,
                           int critical, int roll, int *modifier, int *type) {
  MechTarget(mech) = -1;
}

void FailureWeaponMissiles(Mech *mech, int weapnum, int weaptype, int section,
                           int critical, int roll, int *modifier, int *type) {
  SetPartTempNuke(mech, section, critical,
                  failures[Conv(mech, section, critical) + roll].type);
  *type = CRAZY_MISSILES;
  *modifier = failures[Conv(mech, section, critical) + roll].data;
}

void FailureWeaponDud(Mech *mech, int weapnum, int weaptype, int section,
                      int critical, int roll, int *modifier, int *type) {
  if (failures[Conv(mech, section, critical) + roll].type == FAIL_NONE) {
    mech_set_recycle_part(mech, section, critical,
                          btech_weapon_settings_recycle_time(
                              &mech->xcode.context->weapon_settings, weaptype));
    return;
  }
  SetPartTempNuke(mech, section, critical,
                  failures[Conv(mech, section, critical) + roll].type);
  *type = WEAPON_DUD;
  if (roll == 6) {
    SetPartTempNuke(mech, section, critical, FAIL_DESTROYED);
  }
  mech_set_recycle_part(mech, section, critical,
                        30 + btech_random_range(mech->xcode.context, 1, 60));
}

void FailureWeaponJammed(Mech *mech, int weapnum, int weaptype, int section,
                         int critical, int roll, int *modifier, int *type) {
  SetPartTempNuke(mech, section, critical,
                  failures[Conv(mech, section, critical) + roll].type);
  *type = WEAPON_JAMMED;
  mech_set_recycle_part(mech, section, critical,
                        btech_random_range(mech->xcode.context, 20, 40));
}

void FailureWeaponRange(Mech *mech, int weapnum, int weaptype, int section,
                        int critical, int roll, int *modifier, int *type) {
  *modifier =
      (int)(EGunRangeWithCheck(mech, section, weaptype) *
            (failures[Conv(mech, section, critical) + roll].data / 100.0));
  *type = RANGE;
}

void FailureWeaponDamage(Mech *mech, int weapnum, int weaptype, int section,
                         int critical, int roll, int *modifier, int *type) {
  *modifier =
      (int)(MechWeapons[weaptype].damage *
            (failures[Conv(mech, section, critical) + roll].data / 100.0));
  *type = DAMAGE;
}

void FailureWeaponHeat(Mech *mech, int weapnum, int weaptype, int section,
                       int critical, int roll, int *modifier, int *type) {
  *modifier = (int)MechWeapons[weaptype].heat *
              (failures[Conv(mech, section, critical) + roll].data / 100.0);
  *type = HEAT;
}

void FailureWeaponSpike(Mech *mech, int weapnum, int weaptype, int section,
                        int critical, int roll, int *modifier, int *type) {
  SetPartTempNuke(mech, section, critical,
                  failures[Conv(mech, section, critical) + roll].type);
  *type = POWER_SPIKE;
  if (roll == 6) {
    SetPartTempNuke(mech, section, critical, FAIL_DESTROYED);
    return;
  }
  mech_set_recycle_part(mech, section, critical,
                        btech_random_range(mech->xcode.context, 20, 40));
}

void CheckGenericFail(Mech *mech, int type, int *result, int *mod) {
  int i = GetBrandIndex(type);
  int l = type == -1 ? MechComputer(mech) : MechRadio(mech);
  int roll, in;

  if (result)
    *result = FAIL_NONE;
  if (i < 0)
    return;
  if (mech->xcode.context->configuration->btech_parts) {
    if (!l)
      l = 5;
  } else
    return;
  if (btech_random_range(mech->xcode.context, 1, 5000) != 42)
    return; /* ~1/5000 chance */
  if (btech_random_range(mech->xcode.context, 1, 100) <=
      brands[(i + l - 1) * 5 / 6].success)
    return;
  roll = btech_random_range(mech->xcode.context, 1, 6);
  if (roll == 6)
    roll = btech_random_range(mech->xcode.context, 1, 6);
  in = i + roll - 1;
  switch (failures[in].flag) {
  case REQ_TARGET:
    if (MechTarget(mech) <= 0)
      return;
    break;
  case REQ_TAC:
    if (MechTacRange(mech) == 0)
      return;
    break;
  case REQ_LRS:
    if (MechLRSRange(mech) == 0)
      return;
    break;
  case REQ_SCANNERS:
    if (MechTacRange(mech) == 0 || MechLRSRange(mech) == 0 ||
        MechScanRange(mech) == 0)
      return;
    break;
  case REQ_COMPUTER:
    /* */
    break;
  case REQ_RADIO:
    if (MechRadioRange(mech) == 0)
      return;
    break;
  }
  if (failures[in].message && strcmp(failures[in].message, "none"))
    mech_notify(mech, MECHALL, failures[in].message);
  failures[in].func(mech, -1, -1, -1, -1, roll, mod, result);
}

void CheckWeaponFailed(Mech *mech, int weapnum, int weaptype, int section,
                       int critical, int *modifier, int *type) {
  short roll;
  int l = GetPartBrand(mech, section, critical);
  int t = GetPartType(mech, section, critical);
  int i = GetBrandIndex(t), in;

  *type = FAIL_NONE;
  if (i < 0)
    return;
  if (mech->xcode.context->configuration->btech_parts) {
    if (!l)
      l = 5;
    if (MechWeapons[Weapon2I(t)].special & PCOMBAT)
      return;
  } else
    return;
  if (btech_random_range(mech->xcode.context, 1, 10) < 9)
    return;
  if (btech_random_range(mech->xcode.context, 1, 100) <=
      brands[(i + l - 1) * 5 / 6].success)
    return;
  roll = btech_random_range(mech->xcode.context, 1, 6);
  if (roll == 6)
    roll = btech_random_range(mech->xcode.context, 1, 6);
  in = i + roll - 1;
  if (failures[in].flag & REQ_HEAT)
    if (!MechWeapons[weaptype].heat)
      return;
  if (failures[in].message && strcmp(failures[in].message, "none"))
    mech_notify(mech, MECHALL, failures[in].message);
  failures[in].func(mech, weapnum, weaptype, section, critical, roll, modifier,
                    type);
}
