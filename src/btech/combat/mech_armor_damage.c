#include "equipment_types.h"
#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/* Implements BattleTech combat mechanics for unit armor damage. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_ammodump_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static const char *const MY_COLOR_STRINGS[] = {"", "[fg=green bold]",
                                               "[fg=yellow bold]", "[fg=red]"};
static const char *const MY_MESSAGE_STRINGS[] = {
    "ERROR[reset]", "low.[reset]", "critical![reset]", "BREACHED![reset]"};
static inline const char *my_serious_color_str(Mech *mech, int index) {
  const char *const *value = (const char *const *)checked_storage_at_const(
      (const void *)MY_COLOR_STRINGS,
      sizeof(MY_COLOR_STRINGS) / sizeof(*MY_COLOR_STRINGS),
      sizeof(*MY_COLOR_STRINGS), (size_t)(index % 4));
  return *value;
}

static inline const char *my_serious_str(Mech *mech, int index) {
  const char *const *value = (const char *const *)checked_storage_at_const(
      (const void *)MY_MESSAGE_STRINGS,
      sizeof(MY_MESSAGE_STRINGS) / sizeof(*MY_MESSAGE_STRINGS),
      sizeof(*MY_MESSAGE_STRINGS), (size_t)(index % 4));
  return *value;
}

static inline int my_seriousness_check(Mech *mech, int hitloc) {
  int orig;
  int new;

  orig = mech_section_original_armor(mech, hitloc);
  if (!orig)
    return 0;
  new = mech_section_armor(mech, hitloc);
  if (!new)
    return 3;
  if (new < orig / 4)
    return 2;
  if (new < orig / 2)
    return 1;
  return 0;
}

static inline int my_seriousness_check_r(Mech *mech, int hitloc) {
  int orig;
  int new;

  orig = mech_section_original_rear_armor(mech, hitloc);
  if (!orig)
    return 0;
  new = mech_section_rear_armor(mech, hitloc);
  if (!new)
    return 3;
  if (new < orig / 4)
    return 2;
  if (new < orig / 2)
    return 1;
  return 0;
}

int cause_armordamage(const ArmorDamageRequest *request) {
  Mech *wounded = request->wounded;
  Mech *attacker = request->attacker;
  const int LOS = request->line_of_sight;
  const bool ISREAR = request->rear;
  const bool ISCRITICAL = request->critical;
  const int HITLOC = request->section;
  int damage = request->damage;
  int *crits = request->critical_hits;
  const int W_WEAP_INDX = request->weapon_index;
  const int W_AMMO_MODE = request->ammunition_mode;
  int int_damage = 0;
  int r;
  int seriousness = 0;
  int t_ap_critical = 0;
  int w_percent_left = 0;

  if (mech_class(wounded) == CLASS_MW)
    return (damage > 0) ? damage : 0;

  if ((mech_technology_flags(wounded) & HARDA_TECH) && damage > 0)
    damage = (damage + 1) / 2;

  /* Now decrement armor, and if neccessary, handle criticals... */
  if (mech_class(wounded) == CLASS_MECH && ISREAR &&
      (HITLOC == CTORSO || HITLOC == RTORSO || HITLOC == LTORSO)) {

    if ((mech_section_rear_armor(wounded, HITLOC) - damage) >= 0) {

      w_percent_left =
          (((mech_section_rear_armor(wounded, HITLOC) - damage) * 100) /
           mech_section_original_rear_armor(wounded, HITLOC));
    }

    int_damage = damage - mech_section_rear_armor(wounded, HITLOC);

    if (int_damage > 0) {
      mech_section_rear_armor_set(wounded, HITLOC, 0);
      if (int_damage != damage)
        seriousness = 3;
    } else {
      seriousness = my_seriousness_check_r(wounded, HITLOC);
      mech_section_rear_armor_set(
          wounded, HITLOC, mech_section_rear_armor(wounded, HITLOC) - damage);
      seriousness = (seriousness == my_seriousness_check_r(wounded, HITLOC))
                        ? 0
                        : my_seriousness_check_r(wounded, HITLOC);
    }

  } else {

    /* Silly stuff */
    /*
       mech_section_armor_set(wounded, hitloc, MAX(0, intDamage =
       mech_section_armor(wounded, hitloc) - damage));
       intDamage = abs(intDamage);
     */

    if (mech_section_original_armor(wounded, HITLOC) &&
        ((mech_section_armor(wounded, HITLOC) - damage) >= 0)) {

      w_percent_left = (((mech_section_armor(wounded, HITLOC) - damage) * 100) /
                        mech_section_original_armor(wounded, HITLOC));
    }

    int_damage = damage - mech_section_armor(wounded, HITLOC);

    if (int_damage > 0) {
      mech_section_armor_set(wounded, HITLOC, 0);
      if (int_damage != damage)
        seriousness = 3;
    } else {
      seriousness = my_seriousness_check(wounded, HITLOC);
      mech_section_armor_set(wounded, HITLOC,
                             mech_section_armor(wounded, HITLOC) - damage);
      seriousness = (seriousness == my_seriousness_check(wounded, HITLOC))
                        ? 0
                        : my_seriousness_check(wounded, HITLOC);
    }

    if (!mech_section_armor(wounded, HITLOC))
      mech_flood_section(wounded, HITLOC, mech_position_z(wounded));
  }

  if (!ISCRITICAL && (W_AMMO_MODE & AC_AP_MODE) && (int_damage <= 0) &&
      (w_percent_left < 50))
    t_ap_critical = 1;

  if (ISCRITICAL || t_ap_critical) {
    BtechContext *context = mech_context(wounded);
    r = btech_random_roll(context);
    btech_context_critical_roll_record(context, r);
    /* Do the AP ammo thang */
    if (t_ap_critical) {
      const char *weapon_name =
          checked_string_suffix(weapon_catalogue_name(W_WEAP_INDX), 3);
      if (!strcmp(weapon_name, "AC/2"))
        r -= 4;
      else if (!strcmp(weapon_name, "LightAC/2"))
        r -= 4;
      else if (!strcmp(weapon_name, "AC/5"))
        r -= 3;
      else if (!strcmp(weapon_name, "LightAC/5"))
        r -= 3;
      else if (!strcmp(weapon_name, "AC/10"))
        r -= 2;
      else if (!strcmp(weapon_name, "AC/20"))
        r -= 1;
      else
        r -= 10;
    }

    switch (r) {
    case 8:
    case 9:
      mech_critical_handle(&(CriticalHitDispatch){.wounded = wounded,
                                                  .attacker = attacker,
                                                  .line_of_sight = LOS,
                                                  .section = HITLOC,
                                                  .count = 1});
      (*crits) += 1;
      break;
    case 10:
    case 11:
      mech_critical_handle(&(CriticalHitDispatch){.wounded = wounded,
                                                  .attacker = attacker,
                                                  .line_of_sight = LOS,
                                                  .section = HITLOC,
                                                  .count = 2});
      (*crits) += 2;
      break;
    case 12:
      mech_critical_handle(&(CriticalHitDispatch){.wounded = wounded,
                                                  .attacker = attacker,
                                                  .line_of_sight = LOS,
                                                  .section = HITLOC,
                                                  .count = 3});
      (*crits) += 3;
      break;
    default:
      break;
    }
  }

  if (mech_class(wounded) == CLASS_AERO && int_damage >= 0) {
    mech_section_destroy(&(SectionDestructionRequest){.wounded = wounded,
                                                      .attacker = attacker,
                                                      .line_of_sight = LOS,
                                                      .section = HITLOC});
    if (mech_is_destroyed(wounded)) {
      return 0;
    }
    switch (HITLOC) {
    case AERO_AFT:
      mech_make_fall(wounded);
      mech_current_speed_set(wounded, 0);
      mech_max_speed_set(wounded, 0);
      mech_vertical_speed_set(wounded, 0);
      if (!mech_is_landed(wounded))
        mech_notify(wounded, MECHALL, "You feel the thrust die..");
      else
        mech_notify(wounded, MECHALL, "The computer reports engine destroyed!");
      if (!mech_is_landed(wounded))
        mech_event_schedule(wounded, EVENT_FALL, mech_fall_event, FALL_TICK,
                            -1);
      break;
    }
  }

  if (seriousness > 0 && mech_armor_warning_enabled(wounded)) {
    mech_printf(wounded, MECHALL, "%sWARNING: %s%s Armor %s",
                my_serious_color_str(wounded, seriousness),
                armor_section_abbreviation(
                    &(ArmorSectionReference){.unit_class = mech_class(wounded),
                                             .movement_type =
                                                 mech_movement_type(wounded),
                                             .location = HITLOC})
                    .text,
                ISREAR ? " (Rear)" : "", my_serious_str(wounded, seriousness));
  }

  return int_damage > 0 ? int_damage : 0;
}

int cause_internaldamage(const InternalDamageRequest *request) {
  Mech *wounded = request->wounded;
  Mech *attacker = request->attacker;
  const int LOS = request->line_of_sight;
  const int HITLOC = request->section;
  int int_damage = request->damage;
  int *crits = request->critical_hits;
  BtechContext *context = mech_context(wounded);
  int r = btech_random_roll(context);
  char locname[30];
  char msgbuf[MBUF_SIZE];

  armor_string_from_index(HITLOC, locname, mech_class(wounded),
                          mech_movement_type(wounded));
  if ((mech_technology_flags(wounded) & REINFI_TECH) && int_damage > 0)
    int_damage = (int_damage + 1) / 2;
  else if (mech_technology_flags(wounded) & COMPI_TECH)
    int_damage = int_damage * 2;
  /* Critical hits? */
  btech_context_critical_roll_record(context, r);
  if (!(*crits)) {
    switch (r) {
    case 8:
    case 9:
      mech_critical_handle(&(CriticalHitDispatch){.wounded = wounded,
                                                  .attacker = attacker,
                                                  .line_of_sight = LOS,
                                                  .section = HITLOC,
                                                  .count = 1});
      break;
    case 10:
    case 11:
      mech_critical_handle(&(CriticalHitDispatch){.wounded = wounded,
                                                  .attacker = attacker,
                                                  .line_of_sight = LOS,
                                                  .section = HITLOC,
                                                  .count = 2});
      break;
    case 12:
      if (mech_class(wounded) == CLASS_MECH ||
          mech_class(wounded) == CLASS_MW) {
        switch (HITLOC) {
        case RARM:
        case LARM:
        case RLEG:
        case LLEG:
        case HEAD:
          /* Limb blown off */
          mech_notify(wounded, MECHALL,
                      "[fg=yellow bold]CRITICAL HIT!![reset]");
          if (!mech_is_destroyed(wounded)) {
            (void)snprintf(
                msgbuf, sizeof(msgbuf),
                "'s %s is blown off in a shower of sparks and smoke!", locname);
            mech_los_broadcast(wounded, msgbuf);
          }
          mech_section_destroy(
              &(SectionDestructionRequest){.wounded = wounded,
                                           .attacker = attacker,
                                           .line_of_sight = LOS,
                                           .section = HITLOC});
          if (mech_class(wounded) != CLASS_MW)
            int_damage = 0;
          break;
        default:
          /* Ouch */
          mech_critical_handle(&(CriticalHitDispatch){.wounded = wounded,
                                                      .attacker = attacker,
                                                      .line_of_sight = LOS,
                                                      .section = HITLOC,
                                                      .count = 3});
          break;
        }
      } else {
        mech_critical_handle(&(CriticalHitDispatch){.wounded = wounded,
                                                    .attacker = attacker,
                                                    .line_of_sight = LOS,
                                                    .section = HITLOC,
                                                    .count = 3});
      }

      break;
    default:
      break;
      /* No critical hit */
    }
  }
  /* Hmm.. This should be interesting */
  if (mech_class(wounded) == CLASS_MECH && int_damage && HITLOC == CTORSO &&
      mech_section_internal(wounded, HITLOC) ==
          mech_section_original_internal(wounded, HITLOC))
    mech_reactor_instability_start_tick_set(wounded,
                                            btech_context_event_tick(context));

  if (mech_section_internal(wounded, HITLOC) <= int_damage) {
    int_damage -= mech_section_internal(wounded, HITLOC);
    mech_section_destroy(&(SectionDestructionRequest){.wounded = wounded,
                                                      .attacker = attacker,
                                                      .line_of_sight = LOS,
                                                      .section = HITLOC});

  } else {
    mech_section_internal_set(
        wounded, HITLOC, mech_section_internal(wounded, HITLOC) - int_damage);
    int_damage = 0;
  }
  return int_damage;
}
