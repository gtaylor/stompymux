/* Implements BattleTech movement mechanics for unit ood. */

#include <stdlib.h>
#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_position_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/commands/action_messages.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"

void mech_ood_damage(Mech *wounded, Mech *attacker, int damage) {
  int cocoon_integrity;

  mech_printf(attacker, MECHALL,
              "[fg=green]You hit the cocoon for %d points of damage![reset]",
              damage);
  mech_printf(wounded, MECHALL,
              "[fg=yellow bold]Your cocoon has been hit for %d points of "
              "damage![reset]",
              damage);
  cocoon_integrity = MAX(0, mech_cocoon_integrity(wounded) - damage);
  mech_cocoon_integrity_set(wounded, cocoon_integrity);
  if (cocoon_integrity)
    return;
  /* Abort the OOD and initiate falling */
  char terrain = mech_real_terrain_get(wounded);
  int elevation = mech_position_elevation_magnitude(wounded);
  int surface_elevation =
      terrain == BATTLE_TERRAIN_WATER || terrain == BATTLE_TERRAIN_ICE
          ? -elevation
          : elevation;

  if (mech_position_z(wounded) > surface_elevation) {
    if (mech_jump_speed(wounded) >= MP1) {
      mech_notify(
          wounded, MECHALL,
          "You initiate your jumpjets to compensate for the breached cocoon!");
      mech_cocoon_integrity_set(wounded, -1);
      return;
    }
    mech_notify(wounded, MECHALL,
                "Your cocoon has been destroyed - have a nice fall!");
    mech_los_broadcast(
        wounded,
        "starts plummeting down, as the final blast blows the cocoon apart!");
    mech_event_cancel(wounded, EVENT_OOD);
    mech_event_schedule(wounded, EVENT_FALL, mech_fall_event, FALL_TICK, -1);
  }
}

void mech_ood_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int mof = 0, roll, roll_needed, para = 0;
  int unit_class;
  BtechContext *context;

  if (!mech_is_out_of_control(mech))
    return;
  context = mech_context(mech);
  unit_class = mech_class(mech);
  MarkForLOSUpdate(mech);
  if (mech_drop_height_above_surface(mech) > OOD_SPEED) {
    mech_position_z_set(mech, mech_position_z(mech) - OOD_SPEED);
    mech_event_schedule(mech, EVENT_OOD, mech_ood_event, OOD_TICK, 0);
    return;
  }
  /* Time to hit da ground */
  mech_notify(mech, MECHALL, "Your unit touches down!");

  notify_event(btech_context_evaluation(context), nullptr, mech_dbref(mech),
               mech_dbref(mech), mech_dbref(mech), LUA_EVENT_OOD_LAND,
               (char **)nullptr, 0);

  if (mech_condition_summary(mech).combat_safe) {
    /* If we're combat safe, we land regardless, since we're not gonna take any
     * damage */
    mech_cocoon_integrity_set(mech, 0);
    mech_los_broadcast(mech, "touches down safely!");
    mech_drop_surface_set(mech, true);
    mech_maybe_move(mech);
    return;
  }

  if (mech_is_fallen(mech))
    mof = -10;
  if (mech_pilot_is_unconscious(mech) || !mech_is_started(mech) ||
      mech_is_blinded(mech))
    mof = -20;
  roll = btech_random_roll(context);
  roll_needed =
      unit_class == CLASS_BSUIT || unit_class == CLASS_MW
          ? FindPilotPiloting(mech) - 1
          : FindSPilotPiloting(mech) + mech_pilot_skill_modifier(mech);

  if (!mech_is_started(mech))
    roll_needed += 10;
  if (mech_cocoon_integrity(mech) == 1) {
    para = 1;
  } else if (mech_cocoon_integrity(mech) < 0) {
    roll_needed += 4;
  } else if (mech_cocoon_integrity(mech) == 0) {
    roll_needed += 10;
  }

  if (mech_real_terrain_get(mech) != BATTLE_TERRAIN_GRASSLAND &&
      mech_real_terrain_get(mech) != BATTLE_TERRAIN_ROAD) {
    if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER ||
        mech_real_terrain_get(mech) == BATTLE_TERRAIN_HIGH_WATER)
      roll_needed += 2;
    else
      roll_needed += 3;
  }

  mech_cocoon_integrity_set(mech, 0);

  if (is_in_character(btech_context_database(context), mech_dbref(mech)) &&
      game_object_location(btech_context_database(context),
                           mech_pilot_dbref(mech)) != mech_dbref(mech))
    roll_needed += 99;

  mech_notify(mech, MECHPILOT, "You make a piloting skill roll!");

  mech_notify(
      mech, MECHPILOT,

      tprintf("Modified Pilot Skill: BTH %d\tRoll: %d", roll_needed, roll));

  mof += (roll - roll_needed);

  if (roll >= roll_needed) {

    if (roll_needed > 2)

      AccumulatePilXP(mech_pilot_dbref(mech), mech,
                      BOUNDED(1, (abs(mof) + 1) * 2, 20), 1);
  }

  mof += (roll - roll_needed);

  if (mof < 0) {

    if (unit_class == CLASS_MECH) {

      mech_notify(
          mech, MECHALL,

          "You are unable to control your momentum and fall on your face!");

      mech_los_broadcast(mech,

                         "touches down on the ground, twists, and falls down!");

      mech_fall(mech, (abs(mof) * (para ? 1 : 2)), 1);

    } else if (unit_class == CLASS_BSUIT) {

      int i, ii, dam;

      mech_notify(mech, MECHALL,

                  "You are unable to control your momentum and crash!");

      mech_los_broadcast(mech, "crashes to the ground!");

      for (i = 0; i < NUM_SECTIONS; i++) {

        dam = 0;

        if (mech_section_original_internal(mech, i) > 0) {

          for (ii = mof; ii < 0; ii++)

            dam += btech_random_range(context, 1, 4);

          DamageMech(mech, mech, 0, -1, i, 0, 0, dam, -1, -1, 0, 0, 0, 0);

          mech_flood(mech);
        }
      }

      mech_fall(mech, 0, 1);

    } else {

      mech_notify(mech, MECHALL,

                  "You are unable to control your momentum and crash!");

      mech_los_broadcast(mech, "crashes at the ground!");

      mech_fall(mech, (abs(mof) * (para ? 1 : 3)), 1);
    }

  } else if (!para) {

    mech_los_broadcast(mech, "touches down!");

  } else if (para) {

    mech_los_broadcast(mech, "touches down and rolls on the ground!");

    /* Legacy parachute landing intentionally applies no fall damage. */
  }

  mech_drop_surface_set(mech, true);

  if (!mech_is_fallen(mech))

    mech_domino_resolve(mech, MECH_DOMINO_FALL);

  if ((mech_movement_type(mech) == MOVE_HULL ||
       mech_movement_type(mech) == MOVE_FOIL) &&
      !(battle_terrain_is_water(mech_real_terrain_get(mech)) &&
        mech_position_z(mech) <= 0))
    mech_desired_speed_set(mech, 0.0F);

  mech_maybe_move(mech);

  /* Lets handle dropping right into the water. Anything but a mech/hover goes
   * glub */
  if (battle_terrain_is_water(mech_real_terrain_get(mech)) &&
      mech_position_z(mech) < 0 &&
      (unit_class == CLASS_VEH_GROUND || unit_class == CLASS_VTOL ||
       unit_class == CLASS_BSUIT || unit_class == CLASS_AERO ||
       unit_class == CLASS_DS) &&
      !(mech_technology_flags_secondary(mech) & WATERPROOF_TECH)) {

    mech_notify(mech, MECHALL,
                "Water floods your engine and your unit "
                "becomes unoperable.");
    if (unit_class == CLASS_BSUIT)
      mech_los_broadcast(mech,
                         "emits some bubbles and flails their arms around "
                         "as they sink to the bottom!");
    else
      mech_los_broadcast(mech,
                         "emits some bubbles as its engines are flooded.");
    mech_destroy(mech, mech, 0, KILL_TYPE_FLOOD);
  }
}

void mech_ood_initiate(DbRef player, Mech *mech, char *buffer) {
  char *args[4];
  int x, y, z = ORBIT_Z, argc;
  BtechContext *context = mech_context(mech);

  if ((argc = mech_parseattributes(buffer, args, 3)) < 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid attributes!");
    return;
  }
  if (!parse_int_checked(args[0], &x)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number! (x)");
    return;
  }
  if (!parse_int_checked(args[1], &y)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number! (y)");
    return;
  }
  if (argc == 3 && !parse_int_checked(args[2], &z)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number! (z)");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "OOD already in progress!");
    return;
  }
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", x, y));
  if (mech_position_x(mech) != x || mech_position_y(mech) != y) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid co-ordinates!");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You'll have to get up first.");
    return;
  }
  if (mech_condition_summary(mech).digging) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too busy digging in.");
    return;
  }
  mech_position_z_set(mech, z);
  MarkForLOSUpdate(mech);
  mecha_notify(btech_context_evaluation(context), player, "OOD initiated.");
  if (mech_condition_summary(mech).evading)
    mech_evading_set(mech, false);

  if (mech_condition_summary(mech).sprinting)
    mech_sprinting_set(mech, false);

  if (mech_is_flying_type(mech)) {
    mech_landed_set(mech, false);
    mech_desired_speed_set(mech, mech_maximum_speed(mech) / 2);
    if (mech_is_aerospace_unit(mech))
      mech_desired_angle_set(mech, 0);
    mech_maybe_move(mech);
  } else {
    mech_cocoon_integrity_set(mech,
                              mech_calculated_weight(mech) / 5 / 1024 + 1);
    mech_event_cancel(mech, EVENT_MOVE);
    mech_event_schedule(mech, EVENT_OOD, mech_ood_event, OOD_TICK, 0);
  }
}
