/* Implements BattleTech movement mechanics for unit move controls. */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"

void mech_stand_empty(DbRef player, void *data) {
  char arguments[] = "";

  mech_stand(player, data, arguments);
}

static bool mech_control_requires_water(const Mech *mech) {
  return mech_movement_type(mech) == MOVE_HULL ||
         mech_movement_type(mech) == MOVE_FOIL;
}

static bool mech_control_is_on_water(Mech *mech) {
  return battle_terrain_is_water(mech_real_terrain_get(mech)) &&
         mech_position_z(mech) <= 0;
}

static bool mech_control_is_rolling(const Mech *mech) {
  return mech_class(mech) == CLASS_AERO || mech_class(mech) == CLASS_DS;
}

static bool mech_control_is_flying(const Mech *mech) {
  return mech_is_aerospace_unit(mech) || mech_movement_type(mech) == MOVE_VTOL;
}

static float mech_control_walking_speed(float maximum_speed) {
  return 2.0F * maximum_speed / 3.0F;
}

static bool mech_control_is_running(float speed, float maximum_speed) {
  return speed > mech_control_walking_speed(maximum_speed) + 0.1F;
}

/* Facing related */
void mech_heading(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  char *args[1];
  int newheading;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  if (mech_parseattributes(buffer, args, 1) == 1) {
    MechConditionSummary condition = mech_condition_summary(mech);
    if (mech_movement_type(mech) == MOVE_NONE) {
      mecha_notify(btech_context_evaluation(context), player,
                   "This piece of equipment is stationary!");
      return;
    }
    if (condition.fortified) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Your fortified state prevents you from moving.");
      return;
    }
    if (mech_control_requires_water(mech) && !mech_control_is_on_water(mech)) {
      mecha_notify(
          btech_context_evaluation(context), player,
          "You are regrettably unable to move at this time. We apologize for "
          "the inconvenience.");
      return;
    }
    if (mech_is_aerospace_unit(mech) && condition.spinning &&
        !mech_is_landed(mech)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are unable to control your craft at the moment.");
      return;
    }
    if (condition.performing_action) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are too busy at the moment to turn.");
      return;
    }
    if (condition.dug_in) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are in a hole you dug, unable to move [use "
                   "speed cmd to get out].");
      return;
    }
    if (condition.hull_down) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You can not turn while hulldown");
      return;
    }
    if (mech_event_count(mech, EVENT_CHANGING_HULLDOWN)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are busy changing your hulldown mode");
      return;
    }
    if (condition.digging) {
      mech_notify(mech, MECHALL, "You cease your attempts at digging in.");
      mech_stop_digging(mech);
    }
    if (!parse_int_checked(args[0], &newheading)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Invalid heading!");
      return;
    }
    newheading = acceptable_degree(newheading);
    mech_desired_heading_set(mech, newheading);
    mech_printf(mech, MECHALL, "Heading changed to %d.", newheading);
    mech_maybe_move(mech);
  } else {
    notify_printf(btech_context_evaluation(context), player,
                  "Your current heading is %i.", mech_heading_degrees(mech));
  }
}

void mech_turret(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);
  char *args[1];
  int newheading;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_class(mech) == CLASS_MECH || mech_class(mech) == CLASS_MW ||
      mech_class(mech) == CLASS_BSUIT || mech_is_aerospace_unit(mech) ||
      !mech_section_internal(mech, TURRET)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You don't have a turret.");
    return;
  }
  if (condition.turret_jammed) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your turret is jammed in position.");
    return;
  }
  if (condition.turret_locked) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your turret is locked in position.");
    return;
  }
  if (mech_parseattributes(buffer, args, 1) == 1) {
    if (!parse_int_checked(args[0], &newheading)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Invalid turret heading!");
      return;
    }
    newheading = acceptable_degree(newheading);
    mech_turret_heading_absolute_set(mech, newheading);
    mech_printf(mech, MECHALL, "Turret facing changed to %d.",
                mech_turret_heading_absolute(mech));
  } else {
    notify_printf(btech_context_evaluation(context), player,
                  "Your turret is currently facing %d.",
                  mech_turret_heading_absolute(mech));
  }

  mark_for_los_update(mech);
}

void mech_rotatetorso(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);
  char *args[2];

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_class(mech) == CLASS_BSUIT) {
    mecha_notify(btech_context_evaluation(context), player, "Huh?");
    return;
  }
  if (mech_class(mech) != CLASS_MECH && mech_class(mech) != CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You don't have a torso.");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You're lying flat on your face, you can't rotate your torso.");
    return;
  }
  if (mech_class(mech) == CLASS_MECH && mech_movement_type(mech) == MOVE_QUAD) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Quads can't rotate their torsos.");
    return;
  }
  if (mech_parseattributes(buffer, args, 2) == 1) {
    switch (args[0][0]) {
    case 'L':
    case 'l':
      if (condition.torso_left) {
        mecha_notify(btech_context_evaluation(context), player,
                     "You cannot rotate torso beyond 60 degrees!");
        return;
      }
      if (condition.torso_right)
        mech_torso_twist_set(mech, MECH_TORSO_CENTER);
      else
        mech_torso_twist_set(mech, MECH_TORSO_LEFT);
      mech_notify(mech, MECHALL, "You rotate your torso left.");
      break;
    case 'R':
    case 'r':
      if (condition.torso_right) {
        mecha_notify(btech_context_evaluation(context), player,
                     "You cannot rotate torso beyond 60 degrees!");
        return;
      }
      if (condition.torso_left)
        mech_torso_twist_set(mech, MECH_TORSO_CENTER);
      else
        mech_torso_twist_set(mech, MECH_TORSO_RIGHT);
      mech_notify(mech, MECHALL, "You rotate your torso right.");
      break;
    case 'C':
    case 'c':
      mech_torso_twist_set(mech, MECH_TORSO_CENTER);
      mech_notify(mech, MECHALL, "You center your torso.");
      break;
    default:
      mecha_notify(btech_context_evaluation(context), player,
                   "Rotate must have LEFT RIGHT or CENTER.");
      break;
    }
  } else {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments!");
  }
  mark_for_los_update(mech);
}

static const struct MechSpeedName {
  const char *name;
  int flag;
} SPEED_TABLES[] = {{"walk", 1},   {"run", 2},   {"stop", 0}, {"back", -1},
                    {"cruise", 1}, {"flank", 2}, {nullptr, 0}};

static const struct MechSpeedName *speed_table_entry(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(SPEED_TABLES, 7, sizeof(*SPEED_TABLES),
                                  (size_t)index);
}

void mech_speed(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);
  char *args[1];
  float newspeed, walkspeed, maxspeed;
  int i;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  if (condition.fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your fortified state prevents you from moving.");
    return;
  }
  if (mech_control_is_rolling(mech)) {
    if (!mech_is_landed(mech)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Use thrust command instead!");
      return;
    }
  } else if (mech_control_is_flying(mech)) {
    if (mech_class(mech) != CLASS_VTOL) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Use thrust command instead!");
      return;
    }
  }
  if (mech_movement_type(mech) == MOVE_NONE) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This piece of equipment is stationary!");
    return;
  }
  if (condition.performing_action) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are too busy at the moment to turn.");
    return;
  }
  if (mech_event_count(mech, EVENT_STAND)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are currently standing up and cannot move.");
    return;
  }
  if (mech_is_fallen(mech) && mech_class(mech) != CLASS_MECH &&
      mech_class(mech) != CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your vehicle's movement system is destroyed.");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are currently prone and cannot move.");
    return;
  }
  if (mech_control_requires_water(mech) && !mech_control_is_on_water(mech)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You are regrettably unable to move at this time. We apologize for "
        "the inconvenience.");
    return;
  }

  if (mech_class(mech) != CLASS_MECH)
    if (mech_event_count(mech, EVENT_REMOVE_PODS)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are too busy removing iNARC pods!");
      return;
    }
  if (condition.hull_down) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can not move while hulldown");
    return;
  }
  if (mech_event_count(mech, EVENT_CHANGING_HULLDOWN)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are busy changing your hulldown mode");
    return;
  }

  if (mech_parseattributes(buffer, args, 1) != 1) {
    notify_printf(btech_context_evaluation(context), player,
                  "Your current speed is %.2f.",
                  (double)mech_current_speed(mech));
    return;
  }
  if (mech_control_is_flying(mech) && mech_fuel(mech) <= 0 &&
      !mech_aero_has_free_fuel(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're out of fuel!");
    return;
  }
  maxspeed = mech_effective_maximum_speed(mech);

  if (mech_movement_type(mech) == MOVE_VTOL)
    maxspeed = sqrtf(maxspeed * maxspeed -
                     mech_vertical_speed(mech) * mech_vertical_speed(mech));

  maxspeed = maxspeed > 0.0F ? maxspeed : 0.0F;

  walkspeed = mech_control_walking_speed(maxspeed);
  char **speed_argument_slot =
      (char **)checked_storage_at((void *)args, 1, sizeof(*args), 0);
  newspeed = strtof(*speed_argument_slot, nullptr);

  if (newspeed < 0.1F) {

    /* Possibly a string speed instead? */
    for (i = 0; speed_table_entry(i)->name; i++)
      if (!strcasecmp(speed_table_entry(i)->name, *speed_argument_slot)) {
        switch (speed_table_entry(i)->flag) {
        case 0:
          newspeed = 0.0F;
          break;
        case -1:
          newspeed = -walkspeed;
          break;
        case 1:
          newspeed = walkspeed;
          break;
        case 2:
          newspeed = maxspeed;
          break;
        }
        break;
      }
  }

  if (newspeed > maxspeed)
    newspeed = maxspeed;
  if (newspeed < -walkspeed)
    newspeed = -walkspeed;

  if (newspeed < 0 && mech_carried_dbref(mech) > 0 &&
      !(mech_technology_flags(mech) & SALVAGE_TECH)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can not backup while towing!");
    return;
  }

  if (newspeed < 0 && condition.sprinting) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can not backup while sprinting!");
    return;
  }

  if (mech_control_is_running(newspeed, maxspeed)) {
    if (mech_event_count(mech, EVENT_DUMP)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You can not run while dumping ammo!");
      return;
    }
    if (mech_event_count(mech, EVENT_UNJAM_AMMO)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You can not run while unjamming your weapon!");
      return;
    }

    /* Exile Stun Code Effect */
    if (condition.stunned) {
      mech_notify(mech, MECHALL,
                  "You cannot move faster than cruise"
                  " speed while stunned!");
      return;
    }

    if (mech_event_count(mech, EVENT_UNSTUN_CREW)) {
      mecha_notify(
          btech_context_evaluation(context), player,
          "Your cannot possibly control a vehicle going this fast in your "
          "current mental state!");
      return;
    }
    if (condition.tail_rotor_destroyed) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Your cannot possibly control a VTOL going this fast with a "
                   "destroyed tail rotor!");
      return;
    }
    if (mech_class(mech) == CLASS_MECH &&
        ((mech_position_z(mech) < 0 &&
          battle_terrain_is_water(mech_real_terrain_get(mech))) ||
         mech_real_terrain_get(mech) == BATTLE_TERRAIN_HIGH_WATER)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You can't run through water!");
      return;
    }
  }
  if (!is_wizard(btech_context_database(context), player) &&
      is_in_character(btech_context_database(context), mech_dbref(mech)) &&
      mech_pilot_dbref(mech) != player) {
    if (newspeed < 0.0F) {
      mecha_notify(
          btech_context_evaluation(context), player,
          "Not being the Pilot of this beast, you cannot move it backwards.");
      return;
    }
    if (newspeed > walkspeed) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Not being the Pilot of this beast, you cannot go faster "
                   "than walking speed.");
      return;
    }
  }
  mech_desired_speed_set(mech, newspeed);
  mech_maybe_move(mech);
  if (fabsf(newspeed) > 0.1F) {
    if (condition.swarm_target > 0) {
      bsuit_swarm_stop(mech, 1);
      mech_hidden_set(mech, false);
    }
    if (condition.digging) {
      mech_notify(mech, MECHALL, "You cease your attempts at digging in.");
      mech_stop_digging(mech);
    }
    mech_dug_in_set(mech, false);
  }
  mech_printf(mech, MECHALL, "Desired speed changed to %d KPH.", (int)newspeed);
}

void mech_vertical(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  char *args[1];
  char buff[50] = {0};
  float newspeed, maxspeed;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  if (mech_class(mech) != CLASS_VTOL && mech_movement_type(mech) != MOVE_SUB) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This command is for VTOLs only.");
    return;
  }
  if (mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0 &&
      !mech_aero_has_free_fuel(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're out of fuel!");
    return;
  }
  if (mech_control_requires_water(mech) && !mech_control_is_on_water(mech)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You are regrettably unable to move at this time. We apologize for "
        "the inconvenience.");
    return;
  }
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 tprintf("Current vertical speed is %.2f KPH.",
                         (double)mech_vertical_speed(mech)));
    return;
  }
  newspeed = strtof(args[0], nullptr);
  maxspeed = mech_effective_maximum_speed(mech);
  maxspeed = sqrtf(maxspeed * maxspeed -
                   mech_desired_speed(mech) * mech_desired_speed(mech));
  if ((newspeed > maxspeed) || (newspeed < -maxspeed)) {
    (void)snprintf(buff, sizeof(buff),
                   "Max vertical speed is + %d KPH and - %d KPH", (int)maxspeed,
                   (int)maxspeed);
    mecha_notify(btech_context_evaluation(context), player, buff);
  } else {
    if (mech_is_fallen(mech)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Your vehicle's movement system is destroyed.");
      return;
    }
    if (mech_class(mech) == CLASS_VTOL && mech_is_landed(mech)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You need to take off first.");
      return;
    }
    mech_vertical_speed_set(mech, newspeed);
    mech_printf(mech, MECHALL, "Vertical speed changed to %d KPH",
                (int)newspeed);
    mech_maybe_move(mech);
  }
}
