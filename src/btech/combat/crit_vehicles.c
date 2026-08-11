/* Resolves critical hits for vehicles. */

#include <math.h>

#include "btconfig.h"
#include "btech_event.h"
#include "crit_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
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
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "section_types.h"

void mech_cargo_infantry_critical_apply(Mech *obj_mech, int w_loc) {
  /*
   * If there's infantry in the unit, the infantry takes
   * damage as if the weapon that caused the crit hit them.
   *
   * If there's cargo, damage is to be determined.
   */

  mech_notify(
      obj_mech, MECHALL,
      "The shot pierces your armor yet fails to hit a critical system!");
}

void mech_vehicle_engine_critical_apply(Mech *obj_mech, Mech *obj_attacker) {
  /*
   * Vehicle engine is severly damaged.
   * The vehicle may not move or change facing.
   *
   * If the vehicle is a VTOL, it may land successfully.
   * If the VTOL is over a clear, paved, rough or building hex is can
   * make a pskill roll to avoid crashing. If the roll fails or it's
   * above any other terrain, the VTOL crashes.
   */

  if (mech_condition_summary(obj_mech).fallen) {
    mech_notify(obj_mech, MECHALL,
                "Your destroyed engine takes another direct hit!");
    return;
  }

  mech_notify(obj_mech, MECHALL,
              "[fg=red bold]Your engine takes a direct hit![reset]");

  if (mech_class(obj_mech) == CLASS_VTOL) {
    if (!mech_is_landed(obj_mech)) {
      if (mech_real_terrain_get(obj_mech) == GRASSLAND ||
          mech_real_terrain_get(obj_mech) == ROAD ||
          mech_real_terrain_get(obj_mech) == BUILDING) {

        if (made_pilot_skill_roll(obj_mech,
                                  mech_position_z(obj_mech) -
                                      mech_position_elevation(obj_mech))) {
          mech_notify(obj_mech, MECHALL, "You land safely!");
          mech_landed_set(obj_mech, true);
          mech_position_z_set(obj_mech, mech_position_elevation(obj_mech));
          mech_position_real_z_sync(obj_mech);
          mech_max_speed_set(obj_mech, 0.0);
          mech_vertical_speed_set(obj_mech, 0.0);
        }
      } else {
        mech_notify(obj_mech, MECHALL, "The ground rushes up to meet you!");
        mech_notify(obj_attacker, MECHALL,
                    "You knock the VTOL out of the sky!");
        mech_los_broadcast(obj_mech, "falls from the sky!");
        mech_fall(obj_mech, mech_position_elevation_magnitude(obj_mech), 0);
      }
    }
  } else {
    mech_max_speed_set(obj_mech, 0.0);
    mech_make_fall(obj_mech);
  }
}

void mech_vehicle_fuel_tank_critical_apply(Mech *obj_mech, Mech *obj_attacker) {
  /*
   * The fuel tank is breached causing the
   * the vehicle to explode. If the unit does not
   * have an ICE engine, treat this as an engine crit.
   */

  if (!(mech_technology_flags(obj_mech) & ICE_TECH)) {
    mech_vehicle_engine_critical_apply(obj_mech, obj_attacker);
    return;
  }

  mech_notify(obj_mech, MECHALL,
              "[fg=red bold]Your fuel tank explodes in a ball of fire![reset]");

  if (obj_mech != obj_attacker)
    mech_los_broadcast(obj_mech, "explodes in a ball of fire!");

  mech_position_z_set(obj_mech, mech_position_elevation(obj_mech));
  mech_position_real_z_sync(obj_mech);
  mech_current_speed_set(obj_mech, 0.0);
  mech_vertical_speed_set(obj_mech, 0.0);
  mech_destroy(obj_mech, obj_attacker, 1, KILL_TYPE_FUELTANK);
  mech_explosion_apply(obj_mech, obj_attacker);
}

void mech_vehicle_crew_stun_critical_apply(Mech *obj_mech) {
  /*
   * For one turn the crew can not move over cruising
   * speed and not fire weapons/ram/use radio/etc, just turn.
   */
  mech_crew_stunned_set(obj_mech, true);
  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]The shot resonates throughout the crew compartment, "
      "temporarily stunning you![reset]");

  mech_stun_crew(obj_mech);
  mech_speed_limit_to_cruise(obj_mech);
}

void mech_vehicle_driver_critical_apply(Mech *obj_mech) {
  /*
   * Driver is hit, apply a +2 to all driving skills
   */

  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]Your vehicle's driver takes a piece of shrapnel, making "
      "it harder to control the vehicle![reset]");
  mech_pilot_skill_modifier_add(obj_mech, 2);
}

void mech_vehicle_sensor_critical_apply(Mech *obj_mech) {
  /*
   * Add +1 to BTH for each sensor crit.
   */

  mech_notify(obj_mech, MECHALL,
              "[fg=red bold]Your sensor suite takes a hit![reset]");
  mech_base_to_hit_modifier_add(obj_mech, 1);
}

void mech_vehicle_commander_critical_apply(Mech *obj_mech) {
  /*
   * Commander is hit. Vehicle suffers a +1 to BTH and driving
   * skills. Also does result of crew stunned.
   */

  mech_notify(obj_mech, MECHALL,
              "[fg=red bold]Your vehicle's commander takes a piece of "
              "shrapnel![reset]");
  mech_pilot_skill_modifier_add(obj_mech, 1);
  mech_base_to_hit_modifier_add(obj_mech, 1);

  mech_vehicle_crew_stun_critical_apply(obj_mech);
}

void mech_vehicle_crew_killed_critical_apply(Mech *obj_mech,
                                             Mech *obj_attacker) {
  /*
   * The whole crew is instantly killed
   * leaving the tank 'destroyed' but fixable.
   */

  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]The shot ricochets around the crew compartment, instantly "
      "killing everyone![reset]");
  mech_destroy(obj_mech, obj_attacker, 0, KILL_TYPE_PILOT);
  mech_contents_kill_if_in_character(obj_mech);

  if (fabsf(mech_current_speed(obj_mech)) > 0.0F)
    mech_los_broadcast(obj_mech, "careens out of control and starts to slow!");

  mech_make_fall(obj_mech);
}

void mech_vtol_copilot_critical_apply(Mech *obj_mech) {
  /*
   * +1 BTH for weapons fire
   */

  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]Your VTOL's pilot takes a piece of shrapnel, making it "
      "harder to aim your weapons![reset]");
  mech_base_to_hit_modifier_add(obj_mech, 1);
}

void mech_vtol_pilot_critical_apply(Mech *obj_mech) {
  /*
   * +2 pskill rolls
   * Must make sucessful pskill roll
   * or fall 1 elevation.
   */

  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]Your VTOL's copilot takes a piece of shrapnel, making it "
      "harder to control the VTOL![reset]");
  mech_pilot_skill_modifier_add(obj_mech, 2);

  /* TODO: make vtol drop a level if it fails a pskill roll */
}

void mech_vtol_rotor_destroyed_critical_apply(Mech *obj_mech,
                                              Mech *obj_attacker, int los) {
  /*
   * Rotors are destroyed sending the vehicle crashing to the ground,
   * if it's not already there.
   */

  if (mech_section_is_destroyed(obj_mech, ROTOR))
    return;

  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]The shot hits your fragile rotor mechanism![reset]");
  mech_los_broadcast(obj_mech, "'s rotor snaps into several parts!");
  mech_section_destroy(&(SectionDestructionRequest){.wounded = obj_mech,
                                                    .attacker = obj_attacker,
                                                    .line_of_sight = los,
                                                    .section = ROTOR});

  if (!obj_attacker) {
    mech_notify(obj_mech, MECHALL, "Your rotor has been destroyed!");
    mech_los_broadcast(obj_mech, "'s rotor has been destroyed!");
  }
}

void mech_vtol_rotor_damaged_critical_apply(Mech *obj_mech) {
  /*
   * -1MP
   */

  /*
   * KipstaFeature: if we have less than 1 MP left then we don't have
   * enough rotor speed to keep us aloft, logically... so let's blow the sucker
   * off and crash the VTOL.
   */
  if (mech_maximum_speed(obj_mech) <= MP1) {
    mech_vtol_rotor_destroyed_critical_apply(obj_mech, nullptr, 1);
    return;
  }

  mech_notify(obj_mech, MECHALL, "Your rotor is damaged!");

  if (!mech_condition_summary(obj_mech).fallen)
    mech_max_speed_lower(obj_mech, MP1);
}

void mech_vtol_tail_rotor_critical_apply(Mech *obj_mech) {
  /*
   * May not move faster than cruising speed.
   * Turns slower.
   */

  if (mech_condition_summary(obj_mech).tail_rotor_destroyed)
    mech_notify(obj_mech, MECHALL,
                "Your damaged tail rotor suffers more damage!");
  else {
    mech_tail_rotor_destroyed_set(obj_mech, true);
    mech_notify(
        obj_mech, MECHALL,
        "[fg=red bold]Your tail rotor is damaged, slowing you down![reset]");

    mech_speed_limit_to_cruise(obj_mech);
  }
}

void mech_vtol_crash_start(Mech *obj_mech) {
  if (!mech_condition_summary(obj_mech).fallen) {
    mech_current_speed_set(obj_mech, 0.0);
    mech_desired_speed_set(obj_mech, 0.0);
    mech_max_speed_set(obj_mech, 0.0);

    if (!mech_is_landed(obj_mech)) {
      mech_notify(obj_mech, MECHALL,
                  "You ponder F = ma, S = F/m, S = at^2 => S=agt^2 in relation "
                  "to the ground.");
      mech_vertical_speed_set(obj_mech, 0.0);
      mech_notify(obj_mech, MECHALL, "You start free-fall.. Enjoy the ride!");
      mech_los_broadcast(obj_mech, "starts to fall to the ground!");
      mech_event_schedule(obj_mech, EVENT_FALL, mech_fall_event, FALL_TICK, -1);

      /*
         MechVerticalSpeed(objMech) = 0;
         mech_notify(objMech, MECHALL, "You fall rapidly from the sky!");
         mech_los_broadcast(objMech, "plummets from the sky!");
         mech_fall(objMech, mech_height_above_surface(objMech), 0);
       */
    }
  }
}

void mech_advanced_vehicle_critical_handle(
    const VehicleCriticalRequest *request) {
  Mech *wounded = request->wounded;
  Mech *attacker = request->attacker;
  const int LOS = request->line_of_sight;
  const int HITLOC = request->section;
  int w_roll = btech_random_roll(mech_context(wounded));

  if (mech_movement_type(wounded) == MOVE_NONE)
    return;

  if (w_roll < 6)
    return;

  mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");

  switch (mech_class(wounded)) {
  case CLASS_VEH_GROUND:
    switch (HITLOC) {
    case FSIDE:
      switch (w_roll) {
      case 6:
        mech_vehicle_driver_critical_apply(wounded);
        break;

      case 7:
        mech_weapon_jam_critical_apply(wounded, HITLOC);
        break;

      case 8:
        mech_vehicle_stabilizer_critical_apply(wounded, HITLOC);
        break;

      case 9:
        mech_vehicle_sensor_critical_apply(wounded);
        break;

      case 10:
        mech_vehicle_commander_critical_apply(wounded);
        break;

      case 11:
        mech_weapon_destroyed_critical_apply(&(RandomWeaponDestructionRequest){
            .attacker = attacker, .mech = wounded, .section = HITLOC});
        break;

      case 12:
        mech_vehicle_crew_killed_critical_apply(wounded, attacker);
        break;
      }
      break;

    case LSIDE:
    case RSIDE:
      switch (w_roll) {
      case 6:
        mech_cargo_infantry_critical_apply(wounded, HITLOC);
        break;

      case 7:
        mech_weapon_jam_critical_apply(wounded, HITLOC);
        break;

      case 8:
        mech_vehicle_crew_stun_critical_apply(wounded);
        break;

      case 9:
        mech_vehicle_stabilizer_critical_apply(wounded, HITLOC);
        break;

      case 10:
        mech_weapon_destroyed_critical_apply(&(RandomWeaponDestructionRequest){
            .attacker = attacker, .mech = wounded, .section = HITLOC});
        break;

      case 11:
        mech_vehicle_engine_critical_apply(wounded, attacker);
        break;

      case 12:
        mech_vehicle_fuel_tank_critical_apply(wounded, attacker);
        break;
      }
      break;

    case BSIDE:
      switch (w_roll) {
      case 6:
        mech_weapon_jam_critical_apply(wounded, HITLOC);
        break;

      case 7:
        mech_cargo_infantry_critical_apply(wounded, HITLOC);
        break;

      case 8:
        mech_vehicle_stabilizer_critical_apply(wounded, HITLOC);
        break;

      case 9:
        mech_weapon_destroyed_critical_apply(&(RandomWeaponDestructionRequest){
            .attacker = attacker, .mech = wounded, .section = HITLOC});
        break;

      case 10:
        mech_vehicle_engine_critical_apply(wounded, attacker);
        break;

      case 11:
        mech_ammunition_critical_apply(&(AmmunitionCriticalRequest){
            .mech = wounded, .attacker = attacker, .section = HITLOC});
        break;

      case 12:
        mech_vehicle_fuel_tank_critical_apply(wounded, attacker);
        break;
      }
      break;

    case TURRET:
      switch (w_roll) {
      case 6:
        mech_vehicle_stabilizer_critical_apply(wounded, HITLOC);
        break;

      case 7:
        mech_turret_jam_critical_apply(wounded);
        break;

      case 8:
        mech_weapon_jam_critical_apply(wounded, HITLOC);
        break;

      case 9:
        mech_turret_lock_critical_apply(wounded);
        break;

      case 10:
        mech_weapon_destroyed_critical_apply(&(RandomWeaponDestructionRequest){
            .attacker = attacker, .mech = wounded, .section = HITLOC});
        break;

      case 11:
        mech_turret_blown_off_critical_apply(wounded, attacker, LOS);
        break;

      case 12:
        mech_ammunition_critical_apply(&(AmmunitionCriticalRequest){
            .mech = wounded, .attacker = attacker, .section = HITLOC});
        break;
      }
      break;
    }
    break;

  case CLASS_VTOL:
    switch (HITLOC) {
    case FSIDE:
      switch (w_roll) {
      case 6:
        mech_vtol_copilot_critical_apply(wounded);
        break;

      case 7:
        mech_weapon_jam_critical_apply(wounded, HITLOC);
        break;

      case 8:
        mech_vehicle_stabilizer_critical_apply(wounded, HITLOC);
        break;

      case 9:
        mech_vehicle_sensor_critical_apply(wounded);
        break;

      case 10:
        mech_vtol_pilot_critical_apply(wounded);
        break;

      case 11:
        mech_weapon_destroyed_critical_apply(&(RandomWeaponDestructionRequest){
            .attacker = attacker, .mech = wounded, .section = HITLOC});
        break;

      case 12:
        mech_vehicle_crew_killed_critical_apply(wounded, attacker);
        break;
      }
      break;

    case LSIDE:
    case RSIDE:
      switch (w_roll) {
      case 6:
        mech_weapon_jam_critical_apply(wounded, HITLOC);
        break;

      case 7:
        mech_cargo_infantry_critical_apply(wounded, HITLOC);
        break;

      case 8:
        mech_vehicle_stabilizer_critical_apply(wounded, HITLOC);
        break;

      case 9:
        mech_weapon_destroyed_critical_apply(&(RandomWeaponDestructionRequest){
            .attacker = attacker, .mech = wounded, .section = HITLOC});
        break;

      case 10:
        mech_vehicle_engine_critical_apply(wounded, attacker);
        break;

      case 11:
        mech_ammunition_critical_apply(&(AmmunitionCriticalRequest){
            .mech = wounded, .attacker = attacker, .section = HITLOC});
        break;

      case 12:
        mech_vehicle_fuel_tank_critical_apply(wounded, attacker);
        break;
      }
      break;

    case BSIDE:
      switch (w_roll) {
      case 6:
        mech_cargo_infantry_critical_apply(wounded, HITLOC);
        break;

      case 7:
        mech_weapon_jam_critical_apply(wounded, HITLOC);
        break;

      case 8:
        mech_vehicle_stabilizer_critical_apply(wounded, HITLOC);
        break;

      case 9:
        mech_weapon_destroyed_critical_apply(&(RandomWeaponDestructionRequest){
            .attacker = attacker, .mech = wounded, .section = HITLOC});
        break;

      case 10:
        mech_vehicle_sensor_critical_apply(wounded);
        break;

      case 11:
        mech_vehicle_engine_critical_apply(wounded, attacker);
        break;

      case 12:
        mech_vehicle_fuel_tank_critical_apply(wounded, attacker);
        break;
      }
      break;

    case ROTOR:
      switch (w_roll) {
      case 6:
      case 7:
      case 8:
        mech_vtol_rotor_damaged_critical_apply(wounded);
        break;

      case 9:
      case 10:
        mech_vtol_tail_rotor_critical_apply(wounded);
        break;

      case 11:
      case 12:
        mech_vtol_rotor_destroyed_critical_apply(wounded, attacker, LOS);
        break;
      }
      break;
    }
    break;
  case CLASS_MECH:
  case CLASS_VEH_NAVAL:
  case CLASS_SPHEROID_DS:
  case CLASS_AERO:
  case CLASS_MW:
  case CLASS_DS:
  case CLASS_BSUIT:
  default:
    break;
  }
}

void mech_vtol_critical_handle(const VehicleCriticalRequest *request) {
  Mech *wounded = request->wounded;
  Mech *attacker = request->attacker;
  const int LOS = request->line_of_sight;
  mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
  switch (btech_random_range(mech_context(wounded), 0, 5)) {
  case 0:
    /* Crew killed */
    mech_notify(wounded, MECHALL, "Your cockpit is destroyed!");
    if (!mech_is_landed(wounded)) {
      mech_notify(attacker, MECHALL, "You knock the VTOL out of the sky!");
      mech_los_broadcast(wounded, "falls down from the sky!");
    }
    mech_destroy(wounded, attacker, 0, KILL_TYPE_COCKPIT);
    mech_contents_kill_if_in_character(wounded);
    break;
  case 1:
    /* Weapon jams, set them recylcling maybe */
    /* hmm. nothing for now, tanks are so weak */
    mech_main_weapon_jam(wounded);
    break;
  case 2:
    /* Engine Hit */
    mech_notify(wounded, MECHALL, "Your engine takes a direct hit!");
    if (!mech_is_landed(wounded)) {
      if (mech_real_terrain_get(wounded) == GRASSLAND ||
          mech_real_terrain_get(wounded) == ROAD ||
          mech_real_terrain_get(wounded) == BUILDING) {
        if (made_pilot_skill_roll(wounded,
                                  mech_position_z(wounded) -
                                      mech_position_elevation(wounded))) {
          mech_notify(wounded, MECHALL, "You land safely!");
          mech_landed_set(wounded, true);
          mech_position_z_set(wounded, mech_position_elevation(wounded));
          mech_position_real_z_sync(wounded);
          mech_max_speed_set(wounded, 0.0);
          mech_vertical_speed_set(wounded, 0.0);
        }
      } else {
        mech_notify(wounded, MECHALL, "The ground rushes up to meet you!");
        mech_notify(attacker, MECHALL, "You knock the VTOL out of the sky!");
        mech_los_broadcast(wounded, "falls from the sky!");
        mech_fall(wounded, mech_position_elevation_magnitude(wounded), 0);
      }
    }
    mech_max_speed_set(wounded, 0.0);
    break;
  case 3:
    /* Crew Killed */
    mech_notify(wounded, MECHALL, "Your cockpit is destroyed!");
    if (!mech_is_landed(wounded)) {
      mech_notify(attacker, MECHALL, "You knock the VTOL out of the sky!");
      mech_los_broadcast(wounded, "falls from the sky!");
    }

    mech_destroy(wounded, attacker, 0, KILL_TYPE_COCKPIT);

    mech_contents_kill_if_in_character(wounded);
    break;
  case 4:
    /* Fuel Tank Explodes */
    mech_notify(wounded, MECHALL, "Your fuel tank explodes in a ball of fire!");
    if (wounded != attacker)
      mech_los_broadcast(wounded, "'s fuel tank explodes in a ball of fire!");
    mech_position_z_set(wounded, mech_position_elevation(wounded));
    mech_position_real_z_sync(wounded);
    mech_current_speed_set(wounded, 0.0);
    mech_vertical_speed_set(wounded, 0.0);
    mech_destroy(wounded, attacker, 1, KILL_TYPE_FUELTANK);
    mech_explosion_apply(wounded, attacker);
    break;
  case 5:
    /* Ammo/Power Plant Explodes */
    mech_notify(wounded, MECHALL, "Your power plant explodes!");
    mech_los_broadcast(wounded, "'s power plant suddenly explodes!");
    mech_position_z_set(wounded, mech_position_elevation(wounded));
    mech_position_real_z_sync(wounded);
    mech_current_speed_set(wounded, 0.0);
    mech_vertical_speed_set(wounded, 0.0);
    mech_destroy(wounded, attacker, 1, KILL_TYPE_POWERPLANT);
    if (!mech_section_configuration_has(wounded, BSIDE, CASE_TECH))
      mech_explosion_apply(wounded, attacker);
    else
      mech_section_destroy(&(SectionDestructionRequest){.wounded = wounded,
                                                        .attacker = attacker,
                                                        .line_of_sight = LOS,
                                                        .section = BSIDE});
  }
}
