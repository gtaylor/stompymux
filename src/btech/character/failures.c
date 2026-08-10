
/* This is the code that runs the parts failures.
   Written by: Nim
   9-28-96

   Parts copyright (c) 2000-2002 Thomas Wouters

 */

/* Implements BattleTech unit part failures. */

#include <assert.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "failures.h"
#include "failures_api.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_startup_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"

extern const int num_def_weapons;

static PartFailureResult FailureRadioStatic(const PartFailureCall *call);
static PartFailureResult FailureRadioShort(const PartFailureCall *call);
static PartFailureResult FailureRadioRange(const PartFailureCall *call);
static PartFailureResult FailureComputerShutdown(const PartFailureCall *call);
static PartFailureResult FailureComputerScanner(const PartFailureCall *call);
static PartFailureResult FailureComputerTarget(const PartFailureCall *call);
static PartFailureResult FailureWeaponMissiles(const PartFailureCall *call);
static PartFailureResult FailureWeaponDud(const PartFailureCall *call);
static PartFailureResult FailureWeaponJammed(const PartFailureCall *call);
static PartFailureResult FailureWeaponDamage(const PartFailureCall *call);
static PartFailureResult FailureWeaponHeat(const PartFailureCall *call);
static PartFailureResult FailureWeaponSpike(const PartFailureCall *call);

static const PartBrand brands[] = {
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

static const PartFailure failures[] = {
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

static const PartBrand *part_brand_at(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(brands, sizeof(brands) / sizeof(*brands),
                                  sizeof(*brands), (size_t)index);
}

static const PartFailure *part_failure_at(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(failures,
                                  sizeof(failures) / sizeof(*failures),
                                  sizeof(*failures), (size_t)index);
}

static int part_brand_failure_index(int type) {
  if (type == -1)
    return COMPUTER_INDEX;
  if (type == -2)
    return RADIO_INDEX;
  if (equipment_is_weapon(type))
    if (type < weapon_equipment_index(num_def_weapons)) {
      type = weapon_from_equipment_index(type);
      if (weapon_catalogue_is_personal_combat(type))
        return -1;
      if (weapon_catalogue_is_flamer(type))
        return FLAMMER_INDEX;
      if (weapon_catalogue_is_energy(type))
        return ENERGY_INDEX;
      if (weapon_catalogue_is_ballistic(type))
        return AC_INDEX;
      if (weapon_catalogue_is_missile(type))
        return MISSILE_INDEX;
      return -1;
    }
  return -1;
}

const char *mech_part_brand_name(const PartBrandRequest *request) {
  int i;

  if (!request->quality_level)
    return NULL;
  i = part_brand_failure_index(request->equipment_type);
  if (i < 0)
    return NULL;
  return part_brand_at(i * 5 / 6 + request->quality_level - 1)->name;
}

static int failure_index_for_critical(const Mech *mech, int section,
                                      int critical) {
  return part_brand_failure_index(
             mech_critical_part_type(mech, section, critical)) -
         1;
}

static PartFailureResult FailureRadioStatic(const PartFailureCall *call) {
  int modifier =
      part_failure_at(part_brand_failure_index(-2) + call->roll - 1)->data;
  return (PartFailureResult){.type = FAIL_STATIC, .modifier = modifier};
}

static void mech_rrec_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  const intptr_t event_value = (intptr_t)e->data2;
  assert(event_value >= INT_MIN && event_value <= INT_MAX);
  const int val = (int)event_value;

  mech_radio_range_add(mech, val);
  if (!mech_is_destroyed(mech) && val == mech_radio_range(mech))
    mech_notify(mech, MECHALL, "Your radio is now operational again.");
}

static void mech_srec_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  const intptr_t event_value = (intptr_t)e->data2;
  assert(event_value >= INT_MIN && event_value <= INT_MAX);
  const int val = (int)event_value;
  const int vt = val / 256;

  switch (vt) {
  case 0:
    mech_tactical_range_set(mech, val);
    if (!mech_is_destroyed(mech))
      mech_notify(mech, MECHALL,
                  "Your tactical scanners are operational again.");
    break;
  case 1:
    mech_long_range_sensor_range_set(mech, val);
    if (!mech_is_destroyed(mech))
      mech_notify(mech, MECHALL,
                  "Your long-range scanners are operational again.");
    break;
  case 2:
    mech_scanner_range_set(mech, val);
    if (!mech_is_destroyed(mech))
      mech_notify(mech, MECHALL, "Your scanners are operational again.");
    break;
  }
}

static PartFailureResult FailureRadioShort(const PartFailureCall *call) {
  Mech *mech = call->mech;
  mech_event_schedule(mech, EVENT_MRECOVERY, mech_rrec_event,
                      btech_random_range_int(
                          mech_context(mech), 30,
                          btech_random_range_int(mech_context(mech), 40, 200)),
                      (long)mech_radio_range(mech));
  mech_radio_range_set(mech, 0);
  return (PartFailureResult){0};
}

static PartFailureResult FailureRadioRange(const PartFailureCall *call) {
  Mech *mech = call->mech;
  int mod =
      part_failure_at(part_brand_failure_index(-2) + call->roll - 1)->data;

  mod = MIN(mech_radio_range(mech) - 1, mod);
  mech_event_schedule(mech, EVENT_MRECOVERY, mech_rrec_event,
                      btech_random_range_int(
                          mech_context(mech), 30,
                          btech_random_range_int(mech_context(mech), 40, 200)),
                      (long)mod);
  mech_radio_range_add(mech, -mod);
  return (PartFailureResult){0};
}

static PartFailureResult FailureComputerShutdown(const PartFailureCall *call) {
  if (mech_is_started(call->mech))
    mech_shutdown(mech_dbref(call->mech), call->mech, "");
  return (PartFailureResult){0};
}

static PartFailureResult FailureComputerScanner(const PartFailureCall *call) {
  Mech *mech = call->mech;
  int tmp =
      part_failure_at(part_brand_failure_index(-1) + call->roll - 1)->data;

  switch (tmp) {
  case 1:
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range_int(
            mech_context(mech), 30,
            btech_random_range_int(mech_context(mech), 40, 200)),
        (long)mech_tactical_range(mech));
    mech_tactical_range_set(mech, 0);
    break;
  case 2:
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range_int(
            mech_context(mech), 30,
            btech_random_range_int(mech_context(mech), 40, 200)),
        (long)mech_long_range_sensor_range(mech) + 256L);
    mech_long_range_sensor_range_set(mech, 0);
    break;
  case 4:
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range_int(
            mech_context(mech), 30,
            btech_random_range_int(mech_context(mech), 40, 200)),
        (long)mech_scanner_range(mech) + 512L);
    mech_scanner_range_set(mech, 0);
    break;
  case 7:
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range_int(
            mech_context(mech), 30,
            btech_random_range_int(mech_context(mech), 40, 200)),
        (long)mech_tactical_range(mech));
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range_int(
            mech_context(mech), 30,
            btech_random_range_int(mech_context(mech), 40, 200)),
        (long)mech_long_range_sensor_range(mech) + 256L);
    mech_event_schedule(
        mech, EVENT_MRECOVERY, mech_srec_event,
        btech_random_range_int(
            mech_context(mech), 30,
            btech_random_range_int(mech_context(mech), 40, 200)),
        (long)mech_scanner_range(mech) + 512L);
    mech_tactical_range_set(mech, 0);
    mech_long_range_sensor_range_set(mech, 0);
    mech_scanner_range_set(mech, 0);
    break;
  }
  return (PartFailureResult){0};
}

static PartFailureResult FailureComputerTarget(const PartFailureCall *call) {
  mech_targeting_target_clear(call->mech);
  return (PartFailureResult){0};
}

static PartFailureResult FailureWeaponMissiles(const PartFailureCall *call) {
  const PartFailure *failure = part_failure_at(
      failure_index_for_critical(call->mech, call->section, call->critical) +
      call->roll);
  mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
      .mech = call->mech,
      .slot = {.section = call->section, .critical = call->critical},
      .failure = failure->type});
  return (PartFailureResult){
      .type = CRAZY_MISSILES,
      .modifier = failure->data,
  };
}

static PartFailureResult FailureWeaponDud(const PartFailureCall *call) {
  Mech *mech = call->mech;
  const PartFailure *failure = part_failure_at(
      failure_index_for_critical(mech, call->section, call->critical) +
      call->roll);
  if (failure->type == FAIL_NONE) {
    mech_set_recycle_part(
        mech, call->section, call->critical,
        btech_weapon_settings_recycle_time(&mech_context(mech)->weapon_settings,
                                           call->weapon_type));
    return (PartFailureResult){0};
  }
  mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
      .mech = mech,
      .slot = {.section = call->section, .critical = call->critical},
      .failure = failure->type});
  if (call->roll == 6) {
    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = mech,
        .slot = {.section = call->section, .critical = call->critical},
        .failure = FAIL_DESTROYED});
  }
  mech_set_recycle_part(mech, call->section, call->critical,
                        30 + btech_random_range_int(mech_context(mech), 1, 60));
  return (PartFailureResult){.type = WEAPON_DUD};
}

static PartFailureResult FailureWeaponJammed(const PartFailureCall *call) {
  Mech *mech = call->mech;
  const PartFailure *failure = part_failure_at(
      failure_index_for_critical(mech, call->section, call->critical) +
      call->roll);
  mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
      .mech = mech,
      .slot = {.section = call->section, .critical = call->critical},
      .failure = failure->type});
  mech_set_recycle_part(mech, call->section, call->critical,
                        btech_random_range_int(mech_context(mech), 20, 40));
  return (PartFailureResult){.type = WEAPON_JAMMED};
}

static PartFailureResult FailureWeaponDamage(const PartFailureCall *call) {
  const int percentage =
      part_failure_at(failure_index_for_critical(call->mech, call->section,
                                                 call->critical) +
                      call->roll)
          ->data;
  return (PartFailureResult){
      .type = DAMAGE,
      .modifier =
          (weapon_catalogue_damage(call->weapon_type) * percentage) / 100,
  };
}

static PartFailureResult FailureWeaponHeat(const PartFailureCall *call) {
  const int percentage =
      part_failure_at(failure_index_for_critical(call->mech, call->section,
                                                 call->critical) +
                      call->roll)
          ->data;
  return (PartFailureResult){
      .type = HEAT,
      .modifier = (weapon_catalogue_heat(call->weapon_type) * percentage) / 100,
  };
}

static PartFailureResult FailureWeaponSpike(const PartFailureCall *call) {
  Mech *mech = call->mech;
  const PartFailure *failure = part_failure_at(
      failure_index_for_critical(mech, call->section, call->critical) +
      call->roll);
  mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
      .mech = mech,
      .slot = {.section = call->section, .critical = call->critical},
      .failure = failure->type});
  if (call->roll == 6) {
    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = mech,
        .slot = {.section = call->section, .critical = call->critical},
        .failure = FAIL_DESTROYED});
    return (PartFailureResult){.type = POWER_SPIKE};
  }
  mech_set_recycle_part(mech, call->section, call->critical,
                        btech_random_range_int(mech_context(mech), 20, 40));
  return (PartFailureResult){.type = POWER_SPIKE};
}

PartFailureResult mech_generic_failure_check(Mech *mech, FailureSystem system) {
  const PartFailureResult no_failure = {0};
  int type = system == FAILURE_SYSTEM_COMPUTER ? -1 : -2;
  int i = part_brand_failure_index(type);
  int l = type == -1 ? mech_computer_quality(mech) : mech_radio_quality(mech);
  int roll, in;

  if (i < 0)
    return no_failure;
  if (mech_context(mech)->configuration->btech_parts) {
    if (!l)
      l = 5;
  } else
    return no_failure;
  if (btech_random_range_int(mech_context(mech), 1, 5000) != 42)
    return no_failure; /* ~1/5000 chance */
  if (btech_random_range_int(mech_context(mech), 1, 100) <=
      part_brand_at((i + l - 1) * 5 / 6)->success)
    return no_failure;
  roll = btech_random_range_int(mech_context(mech), 1, 6);
  if (roll == 6)
    roll = btech_random_range_int(mech_context(mech), 1, 6);
  in = i + roll - 1;
  const PartFailure *failure = part_failure_at(in);
  switch (failure->flag) {
  case REQ_TARGET:
    if (mech_target_dbref(mech) <= 0)
      return no_failure;
    break;
  case REQ_TAC:
    if (mech_tactical_range(mech) == 0)
      return no_failure;
    break;
  case REQ_LRS:
    if (mech_long_range_sensor_range(mech) == 0)
      return no_failure;
    break;
  case REQ_SCANNERS:
    if (mech_tactical_range(mech) == 0 ||
        mech_long_range_sensor_range(mech) == 0 ||
        mech_scanner_range(mech) == 0)
      return no_failure;
    break;
  case REQ_COMPUTER:
    /* */
    break;
  case REQ_RADIO:
    if (mech_radio_range(mech) == 0)
      return no_failure;
    break;
  }
  if (failure->message && strcmp(failure->message, "none"))
    mech_notify(mech, MECHALL, failure->message);
  PartFailureCall call = {
      .mech = mech,
      .weapon_number = -1,
      .weapon_type = -1,
      .section = -1,
      .critical = -1,
      .roll = roll,
  };
  return failure->handler(&call);
}

PartFailureResult
mech_weapon_failure_check(const MechWeaponFailureRequest *request) {
  const PartFailureResult no_failure = {0};
  Mech *mech = request->mech;
  int roll;
  int l = mech_critical_brand(mech, request->section, request->critical);
  int t = mech_critical_part_type(mech, request->section, request->critical);
  int i = part_brand_failure_index(t), in;

  if (i < 0)
    return no_failure;
  if (mech_context(mech)->configuration->btech_parts) {
    if (!l)
      l = 5;
    if (!equipment_is_weapon(t))
      return no_failure;
    if (weapon_catalogue_is_personal_combat(weapon_from_equipment_index(t)))
      return no_failure;
  } else
    return no_failure;
  if (btech_random_range_int(mech_context(mech), 1, 10) < 9)
    return no_failure;
  if (btech_random_range_int(mech_context(mech), 1, 100) <=
      part_brand_at((i + l - 1) * 5 / 6)->success)
    return no_failure;
  roll = btech_random_range_int(mech_context(mech), 1, 6);
  if (roll == 6)
    roll = btech_random_range_int(mech_context(mech), 1, 6);
  in = i + roll - 1;
  const PartFailure *failure = part_failure_at(in);
  if (failure->flag & REQ_HEAT)
    if (!weapon_catalogue_heat(request->weapon_type))
      return no_failure;
  if (failure->message && strcmp(failure->message, "none"))
    mech_notify(mech, MECHALL, failure->message);
  PartFailureCall call = {
      .mech = mech,
      .weapon_number = request->weapon_number,
      .weapon_type = request->weapon_type,
      .section = request->section,
      .critical = request->critical,
      .roll = roll,
  };
  return failure->handler(&call);
}
