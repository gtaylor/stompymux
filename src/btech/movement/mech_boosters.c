#include <math.h>
#include <stdio.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_advanced_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "registry_api.h"
#include "section_types.h"

static void mech_mascr_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (mech_masc_counter_regenerate(mech)) {
    mech_event_schedule(mech, EVENT_MASC_REGEN, mech_mascr_event, MASC_TICK, 0);
  }
}

static void mech_masc_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
#ifndef BT_MOVEMENT_MODES
  int needed = 2 * (1 + mech_masc_counter_advance(mech));
#else
  MechConditionSummary condition = mech_condition_summary(mech);
  int needed = (2 * (1 + mech_masc_counter_advance(mech))) +
               (mech_supercharger_movement_mode_is_enabled(mech) ? 1 : 0) +
               (condition.sprinting ? 2 : 0);
#endif
  BtechContext *context = mech_context(mech);
  int roll = btech_random_roll(context);

  if (!mech_is_started(mech))
    return;
  if (!(mech_technology_flags(mech) & MASC_TECH))
    return;
  if (mech_condition_summary(mech).supercharger_enabled)
    roll--;
  if (needed < 10 &&
      is_good_obj(btech_context_database(context), mech_pilot_dbref(mech)) &&
      is_wizard(btech_context_database(context), mech_pilot_dbref(mech)))
    roll = btech_random_range_int(context, needed + 1, 12);
  mech_printf(mech, MECHALL, "MASC: BTH %d+, Roll: %d", needed + 1, roll);
  if (roll > needed) {
    mech_event_schedule(mech, EVENT_MASC_FAIL, mech_masc_event, MASC_TICK, 0);
    return;
  }
  mech_masc_technology_destroy(mech);
  mech_masc_enabled_set(mech, false);
  if (fabsf(mech_current_speed(mech)) > MP1) {
    mech_notify(mech, MECHALL,
                "Your leg actuators freeze suddenly, and you fall!");
    mech_los_broadcast(mech, "stops and falls in mid-step!");
    mech_fall(mech, 1, 0);
  } else {
    mech_notify(mech, MECHALL, "Your leg actuators freeze suddenly!");
    if (mech_current_speed(mech) > 0.0F)
      mech_los_broadcast(mech, "stops suddenly!");
  }

  /* Break the Hips - FASA canon rule about MASC */
  mech_critical_destroy(mech, RLEG, 0);
  mech_critical_destroy(mech, LLEG, 0);
  /* Don't forget to add in Hipped penalties (for landing, etc) */
  mech_section_base_to_hit_add(mech, RLEG, 2);
  mech_section_base_to_hit_add(mech, LLEG, 2);
  if (mech_movement_type(mech) == MOVE_QUAD) {
    mech_critical_destroy(mech, RARM, 0);
    mech_critical_destroy(mech, LARM, 0);
  }

  /* Let the MUX know both hips gone */
  mech_hip_damage_set(mech, true, true);

  /* Reset the Speeds, this sets all 3 of them */
  mech_max_speed_set(mech, 0.0);
}

void mech_masc(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);

  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  if (!(mech_technology_flags(mech) & MASC_TECH)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your toy ain't prepared for what you're askin' it!");
    return;
  }
  if (mech_condition_summary(mech).masc_enabled) {
    mech_notify(mech, MECHALL, "MASC has been turned off.");
    mech_masc_enabled_set(mech, false);
    mech_desired_speed_set(mech, mech_desired_speed(mech) * 3.0F / 4.0F);
    mech_event_cancel(mech, EVENT_MASC_FAIL);
    mech_event_schedule(mech, EVENT_MASC_REGEN, mech_mascr_event, MASC_TICK, 0);
    return;
  }
  if (mech_effective_maximum_speed(mech) < MP1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't move. How is MASC going to work?");
    return;
  }
  mech_notify(mech, MECHALL, "MASC has been turned on.");
  mech_masc_enabled_set(mech, true);
  mech_event_cancel(mech, EVENT_MASC_REGEN);
  mech_desired_speed_set(mech, mech_desired_speed(mech) * 4.0F / 3.0F);
  mech_event_schedule(mech, EVENT_MASC_FAIL, mech_masc_event, 1, 0);
}

static void mech_scharger_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (mech_supercharger_counter_regenerate(mech)) {
    mech_event_schedule(mech, EVENT_SCHARGE_REGEN, mech_scharger_event,
                        SCHARGE_TICK, 0);
  }
}

static void mech_scharge_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
#ifndef BT_MOVEMENT_MODES
  int needed = 2 * (1 + mech_supercharger_counter_advance(mech));
#else
  MechConditionSummary condition = mech_condition_summary(mech);
  int needed = (2 * (1 + mech_masc_counter_advance(mech))) +
               (condition.masc_enabled ? 1 : 0) + (condition.sprinting ? 2 : 0);
#endif
  BtechContext *context = mech_context(mech);
  int roll = btech_random_roll(context);
  int j, count = 0;
  float maxspeed, newmaxspeed = 0.0F;
  int crit_type;
  char msgbuf[MBUF_SIZE] = {0};

  if (!mech_is_started(mech))
    return;
  if (!(mech_technology_flags_secondary(mech) & SUPERCHARGER_TECH))
    return;
  if (mech_condition_summary(mech).masc_enabled)
    roll = roll - 1;
  if (needed < 10 &&
      is_good_obj(btech_context_database(context), mech_pilot_dbref(mech)) &&
      is_wizard(btech_context_database(context), mech_pilot_dbref(mech)))
    roll = btech_random_range_int(context, needed + 1, 12);
  mech_printf(mech, MECHALL, "Supercharger: BTH %d, Roll: %d", needed + 1,
              roll);
  if (roll > needed) {
    mech_event_schedule(mech, EVENT_SCHARGE_FAIL, mech_scharge_event,
                        SCHARGE_TICK, 0);
    return;
  }

  mech_supercharger_technology_destroy(mech);
  mech_supercharger_enabled_set(mech, false);

  mech_notify(mech, MECHALL, "Your supercharger overloads and explodes!");

  if (mech_class(mech) == CLASS_MECH) {
    for (j = 0; j < mech_section_critical_count(mech, CTORSO); j++) {
      crit_type = mech_critical_part_type(mech, CTORSO, j);
      if (crit_type == special_equipment_index(SUPERCHARGER)) {
        if (!mech_critical_is_destroyed(mech, CTORSO, j))
          mech_critical_destroy(mech, CTORSO, j);
      }
    }

    count = btech_random_range_int(context, 1, 4);

    for (j = 0; count && j < mech_section_critical_count(mech, CTORSO); j++) {
      crit_type = mech_critical_part_type(mech, CTORSO, j);
      if (crit_type == special_equipment_index(ENGINE) &&
          !mech_critical_is_destroyed(mech, CTORSO, j)) {
        mech_critical_destroy(mech, CTORSO, j);
        if (!mech_is_destroyed(mech) && mech_is_started(mech)) {
          (void)snprintf(msgbuf, MBUF_SIZE,
                         "'s center torso spews black smoke!");
          mech_los_broadcast(mech, msgbuf);
        }
        if (mech_engine_heat(mech) < 10) {
          mech_engine_heat_add(mech, 5);
          mech_notify(mech, MECHALL,
                      "Your engine shielding takes a hit!  It's getting hotter "
                      "in here!!");
        } else if (mech_engine_heat(mech) < 15) {
          mech_engine_heat_set(mech, 15);
          mech_notify(mech, MECHALL, "Your engine is destroyed!!");
          mech_destroy(mech, mech, 1, KILL_TYPE_SCHARGE);
        }
        count--;
      }
    }
  }

  if (mech_class(mech) == CLASS_VTOL || mech_class(mech) == CLASS_VEH_GROUND) {
    (void)snprintf(msgbuf, MBUF_SIZE,
                   " coughs thick black smoke from its exhaust.");
    mech_los_broadcast(mech, msgbuf);
    maxspeed = mech_maximum_speed(mech);
    newmaxspeed = maxspeed * 0.5F;
    mech_max_speed_set(mech, newmaxspeed);
  }
}

void mech_scharge(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);

  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  if (!(mech_technology_flags_secondary(mech) & SUPERCHARGER_TECH)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your toy ain't prepared for what you're askin' it!");
    return;
  }
  if (mech_condition_summary(mech).supercharger_enabled) {
    mech_notify(mech, MECHALL, "Supercharger has been turned off.");
    mech_supercharger_enabled_set(mech, false);
    mech_desired_speed_set(mech, mech_desired_speed(mech) * 3.0F / 4.0F);
    mech_event_cancel(mech, EVENT_SCHARGE_FAIL);
    mech_event_schedule(mech, EVENT_SCHARGE_REGEN, mech_scharger_event,
                        SCHARGE_TICK, 0);
    return;
  }
  if (mech_effective_maximum_speed(mech) < MP1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "How much can you Supercharge if you can't move?");
    return;
  }
  mech_notify(mech, MECHALL, "Supercharger has been turned on.");
  mech_supercharger_enabled_set(mech, true);
  mech_event_cancel(mech, EVENT_SCHARGE_REGEN);
  mech_desired_speed_set(mech, mech_desired_speed(mech) * 4.0F / 3.0F);
  mech_event_schedule(mech, EVENT_SCHARGE_FAIL, mech_scharge_event, 1, 0);
}

static void mech_dig_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (!mech_condition_summary(mech).digging)
    return;

  if (!mech_is_started(mech))
    return;

  mech_digging_set(mech, false);
  mech_dug_in_set(mech, true);
  mech_notify(mech, MECHALL,
              "You finish burrowing for cover - only turret weapons are "
              "available now.");
}

void mech_dig(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition;
  char terrain;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  condition = mech_condition_summary(mech);
  terrain = mech_real_terrain_get(mech);
  if (condition.fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already fortified, there's no need to dig.");
    return;
  }
  if (fabsf(mech_current_speed(mech)) > 0.0F) {
    mecha_notify(btech_context_evaluation(context), player, "You are moving!");
    return;
  }
  if (mech_heading_degrees(mech) != mech_desired_heading_degrees(mech)) {
    mecha_notify(btech_context_evaluation(context), player, "You are turning!");
    return;
  }
  if (mech_movement_type(mech) == MOVE_NONE) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are stationary!");
    return;
  }
  if (condition.dug_in) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already dug in!");
    return;
  }
  if (condition.digging) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already digging in!");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "While dropping? I think not.");
    return;
  }
  if (terrain == BATTLE_TERRAIN_ROAD || terrain == BATTLE_TERRAIN_BRIDGE ||
      terrain == BATTLE_TERRAIN_BUILDING || terrain == BATTLE_TERRAIN_WALL) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The surface is slightly too hard for you to dig in.");
    return;
  }
  if (terrain == BATTLE_TERRAIN_WATER) {
    mecha_notify(btech_context_evaluation(context), player,
                 "In water? Who are you kidding?");
    return;
  }

  mech_digging_set(mech, true);
  mech_event_schedule(mech, EVENT_DIG, mech_dig_event, 20, 0);
  mech_notify(mech, MECHALL, "You start digging yourself in a nice hole..");
}

static void mech_unjam_turret_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (mech_is_destroyed(mech))
    return;

  if (mech_pilot_is_unconscious(mech))
    return;

  if (!mech_section_internal(mech, TURRET))
    return;

  if (!mech_is_started(mech))
    return;

  if (mech_condition_summary(mech).turret_locked) {
    mech_notify(mech, MECHALL, "You are unable to unjam the turret!");
    return;
  }

  mech_turret_jammed_set(mech, false);
  mech_notify(mech, MECHALL, "You manage to unjam your turret!");
}

void mech_fixturret(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  condition = mech_condition_summary(mech);
  if (condition.turret_locked) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your turret is locked! You need a repairbay to fix it!");
    return;
  }
  if (!condition.turret_jammed) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your turret is not jammed!");
    return;
  }
  mech_event_schedule(mech, EVENT_UNJAM_TURRET, mech_unjam_turret_event, 60, 0);
  mech_notify(mech, MECHALL, "You start to repair your jammed turret.");
}
