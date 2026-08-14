/* Resolves critical hits for BattleMechs. */

#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "crit_api.h"
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_ammodump_api.h"
#include "mech_ammunition_explosion_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_electronics_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_network_api.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor.h"
#include "mech_sensor_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tag_api.h"
#include "mech_tech_commands_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

int mech_critical_effect_apply(const CriticalEffectRequest *request) {
  Mech *wounded = request->wounded;
  Mech *attacker = request->attacker;
  const int LOS = request->line_of_sight;
  const int HITLOC = request->slot.section;
  int crit_hit = request->slot.critical;
  const int CRIT_TYPE = request->part_type;
  const int CRIT_DATA = request->part_data;
  Mech *mech = wounded;
  int weapindx;
  int damage;
  int destroycrit;
  int weapon_slot;
  int w_first_crit;
  int temp;
  char locname[30];
  char msgbuf[MBUF_SIZE];
  int t_loc_is_arm =
      ((HITLOC == LARM || HITLOC == RARM) && !mech_is_quad(wounded));
  int t_loc_is_leg =
      ((HITLOC == LLEG || HITLOC == RLEG) ||
       ((HITLOC == LARM || HITLOC == RARM) && mech_is_quad(wounded)));
  char part_buf[100];

  int f_crit;
  BtechContext *context = mech_context(wounded);
  BattleMap *map = btech_context_get_map(context, mech_map_dbref(wounded));

  armor_string_from_index(HITLOC, locname, mech_class(wounded),
                          mech_movement_type(wounded));
  mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT!![reset]");

  if (equipment_is_ammunition(CRIT_TYPE)) {
    /* BOOM! */
    /* That's going to hurt... */
    weapindx = ammunition_to_weapon_index(CRIT_TYPE);
    damage = CRIT_DATA * weapon_catalogue_damage(weapindx);
    if (weapon_catalogue_is_missile(weapindx) ||
        weapon_catalogue_is_artillery(weapindx)) {
      int missile_count = btech_context_missile_hit_count(&(MissileHitLookup){
          .context = context, .weapon = weapindx, .roll = 10});
      if (missile_count > 0)
        damage *= missile_count;
    }
    if (weapon_catalogue_is_gauss(weapindx) ||
        weapon_catalogue_does_not_explode(weapindx)) {
      if (weapon_catalogue_is_gauss(weapindx))
        mech_notify(wounded, MECHALL,
                    "One of your Gauss Rifle ammo feeds is destroyed");
      mech_critical_destroy(wounded, HITLOC, crit_hit);
    } else if (damage) {
      mech_ammunition_explode(&(AmmunitionExplosionRequest){
          .attacker = attacker,
          .target = wounded,
          .ammunition = {.section = HITLOC, .critical = crit_hit},
          .damage = damage});
    } else {
      mech_notify(wounded, MECHALL,
                  "You have no ammunition left in that location, lucky you!");
      mech_critical_destroy(wounded, HITLOC, crit_hit);
    }
    return 1;
  }

  if (mech_critical_is_broken(wounded, HITLOC, crit_hit) &&
      equipment_is_weapon(CRIT_TYPE) &&
      !mech_critical_is_disabled(wounded, HITLOC, crit_hit)) {
    for (;;) {
      --crit_hit;
      if (!crit_hit ||
          mech_critical_part_type(wounded, HITLOC, crit_hit) != CRIT_TYPE)
        break;
      if (mech_critical_is_destroyed(wounded, HITLOC, crit_hit))
        break;
    }
    mech_printf(
        wounded, MECHALL, "Your destroyed %s is damaged some more!",
        checked_string_suffix(
            weapon_catalogue_name(weapon_from_equipment_index(CRIT_TYPE)), 3));
    mech_critical_destroy(wounded, HITLOC, crit_hit + 1);
    return 1;
  }

  if (mech_critical_is_nonfunctional(wounded, HITLOC, crit_hit)) {
    if (equipment_is_special(CRIT_TYPE)) {
      switch (special_from_equipment_index(CRIT_TYPE)) {
      case LIFE_SUPPORT:
        strcpy(part_buf, "life support");
        break;
      case COCKPIT:
        strcpy(part_buf, "cockpit");
        break;
      case SENSORS:
        strcpy(part_buf, "sensors");
        break;
      case HEAT_SINK:
        strcpy(part_buf, "heatsink");
        break;
      case JUMP_JET:
        strcpy(part_buf, "jump jet");
        break;
      case ENGINE:
        strcpy(part_buf, "engine");
        break;
      case TARGETING_COMPUTER:
        strcpy(part_buf, "targeting computer");
        break;
      case GYRO:
        strcpy(part_buf, "gyro");
        break;
      case SHOULDER_OR_HIP:
        if (t_loc_is_arm)
          strcpy(part_buf, "shoulder");
        else
          strcpy(part_buf, "hip");
        break;
      case LOWER_ACTUATOR:
      case UPPER_ACTUATOR:
      case HAND_OR_FOOT_ACTUATOR:
        if (t_loc_is_arm) {
          if (special_from_equipment_index(CRIT_TYPE) == HAND_OR_FOOT_ACTUATOR)
            strcpy(part_buf, "hand actuator");
          else
            strcpy(part_buf, "arm actuator");
        } else {
          if (special_from_equipment_index(CRIT_TYPE) == HAND_OR_FOOT_ACTUATOR)
            strcpy(part_buf, "foot actuator");
          else
            strcpy(part_buf, "arm actuator");
        }
        break;
      case C3_MASTER:
        strcpy(part_buf, "C3 system");
        break;
      case C3_SLAVE:
        strcpy(part_buf, "C3 system");
        break;
      case C3I:
        strcpy(part_buf, "C3i system");
        break;
      case TAG:
        strcpy(part_buf, "TAG system");
        break;
      case ECM:
        strcpy(part_buf, "ECM system");
        break;
      case ANGELECM:
        strcpy(part_buf, "Angel ECM system");
        break;
      case BEAGLE_PROBE:
        strcpy(part_buf, "Beagle Active Probe");
        break;
      case BLOODHOUND_PROBE:
        strcpy(part_buf, "Bloodhound Active Probe");
        break;
      case LIGHT_BAP:
        strcpy(part_buf, "Light Beagle Active Probe");
        break;
      case ARTEMIS_IV:
        strcpy(part_buf, "ArtemisIV system");
        break;
      case AXE:
        strcpy(part_buf, "axe");
        break;
      case SWORD:
        strcpy(part_buf, "sword");
        break;
      case MACE:
        strcpy(part_buf, "mace");
        break;
      case DUAL_SAW:
        strcpy(part_buf, "dual saw");
        break;
      case DS_AERODOOR:
        strcpy(part_buf, "aero doors");
        break;
      case DS_MECHDOOR:
        strcpy(part_buf, "mech doors");
        break;
      case NULL_SIGNATURE_SYSTEM:
        strcpy(part_buf, "Null Signature System");
        break;
      } // end switch() - Part Names
    } // end if()

    if (equipment_is_weapon(CRIT_TYPE)) {
      mech_printf(
          wounded, MECHALL, "Part of your non-working %s has been hit!",
          checked_string_suffix(
              weapon_catalogue_name(weapon_from_equipment_index(CRIT_TYPE)),
              3));
    } else {
      mech_printf(wounded, MECHALL, "Part of your non-working %s has been hit!",
                  part_buf);
    }
    mech_critical_destroy(wounded, HITLOC, crit_hit);
    return 1;
  }

  if (equipment_is_weapon(CRIT_TYPE)) {
    if (mech_weapon_critical_handle(&(WeaponCriticalRequest){
            .attacker = attacker,
            .wounded = wounded,
            .slot = {.section = HITLOC, .critical = crit_hit},
            .part_type = CRIT_TYPE})) {
      return 1;
    }

    mech_weapon_critical_apply(&(WeaponCriticalApplication){
        .mech = mech, .slot = {.section = HITLOC, .critical = crit_hit}});

    return 1;
  }

  if (equipment_is_special(CRIT_TYPE)) {
    destroycrit = 1;
    switch (special_from_equipment_index(CRIT_TYPE)) {
    case LIFE_SUPPORT:
      mech_life_support_destroyed_set(wounded, true);
      mech_notify(wounded, MECHALL, "Your life support has been destroyed!");
      break;
    case COCKPIT:
      /* Destroy Mech for now, but later kill pilot as well */
      mech_notify(wounded, MECHALL,
                  "Your cockpit is destroyed, your blood boils, and your body "
                  "is fried! [fg=yellow]You're dead![reset]");
      if (!mech_is_destroyed(wounded)) {
        mech_destroy(wounded, attacker, 0, KILL_TYPE_COCKPIT);
      }

      if (LOS && attacker)
        mech_notify(attacker, MECHALL,
                    "You destroy the cockpit! The pilot's blood splatters down "
                    "the sides!");
      mech_los_broadcast(wounded,
                         "spasms for a second then remains oddly still.");
      mech_pilot_dbref_set(wounded, NOTHING);
      mech_contents_kill_if_in_character(wounded);
      break;
    case SENSORS:
      if (!mech_condition_summary(wounded).sensors_damaged) {
        mech_sensor_ranges_halve(wounded);
        mech_base_to_hit_modifier_add(wounded, 2);
        mech_sensors_damaged_set(wounded, true);
        mech_notify(wounded, MECHALL, "Your sensors have been damaged!");
      } else {
        mech_sensor_ranges_disable(wounded);
        mech_base_to_hit_modifier_set(wounded, 75);
        mech_notify(wounded, MECHALL, "Your sensors have been destroyed!");
      }
      break;
    case SPLIT_CRIT_LEFT:
    case SPLIT_CRIT_RIGHT:
      f_crit = mech_critical_data(wounded, HITLOC, crit_hit);
      temp = reverse_split_crit_loc(wounded, HITLOC, crit_hit);
      if (temp < 0) {
        mech_printf(wounded, MECHALL,
                    "ERROR: Could not find split weapon parent location. "
                    "Loc:%d Crit:%d temp:%d fCrit:%d",
                    HITLOC, crit_hit, temp, f_crit);
        break; // sanity check
      }
      destroycrit = 0;
      if (mech_weapon_critical_handle(&(WeaponCriticalRequest){
              .attacker = attacker,
              .wounded = wounded,
              .slot = {.section = temp, .critical = f_crit},
              .part_type = mech_critical_part_type(wounded, temp, f_crit)}))
        break;
      mech_weapon_critical_apply(&(WeaponCriticalApplication){
          .mech = wounded, .slot = {.section = temp, .critical = f_crit}});
      break;
    case HEAT_SINK:
      if (mech_has_double_heat_sinks(mech)) {
        int heat_sink_critical_size = mech_heat_sink_critical_size(mech);
        w_first_crit = mech_weapon_first_critical(&(WeaponCriticalSearch){
            .mech = wounded,
            .weapon = {.section = HITLOC, .critical = crit_hit},
            .start_critical = 0,
            .part_type = CRIT_TYPE,
            .maximum_criticals = heat_sink_critical_size,
        });
        mech_heat_sink_count_remove(wounded, 2);
        mech_weapon_destroy(&(WeaponDestructionRequest){
            .mech = wounded,
            .first = {.section = HITLOC, .critical = w_first_crit},
            .part_type = CRIT_TYPE,
            .criticals_to_destroy = 1,
            .total_criticals = heat_sink_critical_size});
        destroycrit = 0;
      } else {
        mech_heat_sink_count_remove(wounded, 1);
      }
      mech_notify(wounded, MECHALL, "You lost a heat sink!");
      if (!mech_is_destroyed(wounded)) {
        (void)snprintf(msgbuf, MBUF_SIZE, "'s %s is covered in a green mist!",
                       locname);
        mech_los_broadcast(wounded, msgbuf);
      }
      break;
    case JUMP_JET:
      if (!mech_is_destroyed(wounded) && mech_is_started(wounded)) {
        (void)snprintf(msgbuf, MBUF_SIZE,
                       "'s %s flares as superheated plasma spews out!",
                       locname);
        mech_los_broadcast(wounded, msgbuf);
      }
      /* IMPROVED JJ CHECK HERE. SIMILIAR TO DHS */
      if (mech_technology_flags_secondary(mech) & IMPROVED_JJ_TECH) {
        w_first_crit = mech_weapon_first_critical(&(WeaponCriticalSearch){
            .mech = wounded,
            .weapon = {.section = HITLOC, .critical = crit_hit},
            .start_critical = 0,
            .part_type = CRIT_TYPE,
            .maximum_criticals = 2,
        });
        mech_weapon_destroy(&(WeaponDestructionRequest){
            .mech = wounded,
            .first = {.section = HITLOC, .critical = w_first_crit},
            .part_type = CRIT_TYPE,
            .criticals_to_destroy = 1,
            .total_criticals = 2});
        destroycrit = 0;
      }
      mech_jump_speed_lower(wounded, MP1);
      mech_notify(wounded, MECHALL,
                  "One of your jump jet engines has shut down!");
      if (attacker && mech_jump_speed(wounded) < MP1 &&
          mech_is_jumping(wounded)) {
        mech_notify(wounded, MECHALL,
                    "Losing your last jump jet, you fall from the sky!");
        mech_los_broadcast(wounded, "falls from the sky!");
        mech_fall(wounded, 1, 0);
        mech_domino_resolve(wounded, MECH_DOMINO_FALL);
      }
      break;
    case ENGINE:
      if (!mech_is_destroyed(wounded) && mech_is_started(wounded)) {
        (void)snprintf(msgbuf, MBUF_SIZE, "'s %s spews black smoke!", locname);
        mech_los_broadcast(wounded, msgbuf);
      }
      if (mech_engine_heat(wounded) < 10) {
        mech_engine_heat_add(wounded, 5);
        mech_notify(
            wounded, MECHALL,
            "Your engine shielding takes a hit! It's getting hotter in here!");
      } else if (mech_engine_heat(wounded) < 15) {
        mech_engine_heat_set(wounded, 15);
        mech_notify(wounded, MECHALL, "Your engine is destroyed!");
        if (wounded != attacker && !mech_is_destroyed(wounded) && attacker)
          mech_notify(attacker, MECHALL, "You destroy the engine!");
        if (unit_is_fixable(mech))
          mech_destroy(wounded, attacker, 1, KILL_TYPE_ENGINE);
        else
          mech_destroy(wounded, attacker, 1, KILL_TYPE_NORMAL);
      }
      break;
    case TARGETING_COMPUTER:
      if (!mech_condition_summary(wounded).targeting_computer_destroyed) {
        mech_notify(wounded, MECHALL, "Your targeting computer is destroyed!");
        mech_targeting_computer_destroyed_set(wounded, true);
      }
      break;
    case GYRO:
      /* Hardened Gyro's take one extra hit before damaged */
      if (mech_technology_flags_secondary(wounded) & HDGYRO_TECH) {
        if (!mech_condition_summary(wounded).hardened_gyro_damaged) {
          (void)snprintf(msgbuf, MBUF_SIZE,
                         "emits a screech as its "
                         "hardened gyro buckles slightly!");
          mech_los_broadcast(wounded, msgbuf);
          mech_hardened_gyro_damaged_set(wounded, true);
          mech_notify(wounded, MECHALL, "Your hardened gyro takes a hit!");
          break;
        }
      }

      if (!mech_condition_summary(wounded).gyro_damaged) {
        if (!mech_is_destroyed(wounded) && mech_is_started(wounded)) {
          (void)snprintf(msgbuf, MBUF_SIZE,
                         "emits a loud screech as "
                         "its gyro buckles under the impact!");
          mech_los_broadcast(wounded, msgbuf);
        }
        mech_gyro_damage_set(wounded, true, false);
        mech_pilot_skill_modifier_add(wounded, 3);
        mech_notify(wounded, MECHALL, "Your Gyro has been damaged!");
        if (attacker) {
          if (!made_pilot_skill_roll(wounded, 0) &&
              !mech_condition_summary(wounded).fallen) {
            if (!mech_is_jumping(wounded) && !mech_is_out_of_control(wounded)) {
              mech_notify(wounded, MECHALL,
                          "You lose your balance and fall down!");
              mech_los_broadcast(wounded, "stumbles and falls down.");
              mech_fall(wounded, 1, 0);
            } else {
              mech_notify(wounded, MECHALL, "You fall from the sky!");
              mech_los_broadcast(wounded, "falls from the sky!");
              mech_fall(wounded, mech_jump_speed_mp_for_map(wounded, map), 0);
              mech_domino_resolve(wounded, MECH_DOMINO_FALL);
            }
          }
        }
      } else if (!mech_has_destroyed_gyro(wounded)) {
        mech_gyro_damage_set(wounded, true, true);
        mech_notify(wounded, MECHALL, "Your Gyro has been destroyed!");

        if (attacker) {
          if (!mech_condition_summary(wounded).fallen &&
              !mech_is_jumping(wounded) && !mech_is_out_of_control(wounded)) {
            mech_notify(wounded, MECHALL, "You fall and you can't get up!");
            mech_los_broadcast(wounded, "is knocked over!");
            mech_fall(wounded, 1, 0);
          } else if (!mech_condition_summary(wounded).fallen &&
                     (mech_is_jumping(wounded) ||
                      mech_is_out_of_control(wounded))) {
            mech_notify(wounded, MECHALL, "You fall from the sky!");
            mech_los_broadcast(wounded, "falls from the sky!");
            mech_fall(wounded, mech_jump_speed_mp_for_map(wounded, map), 0);
            mech_domino_resolve(wounded, MECH_DOMINO_FALL);
          }
        }
      } else {
        mech_notify(wounded, MECHALL, "Your destroyed gyro takes another hit!");
      }
      break;
    case SHOULDER_OR_HIP:
      mech_critical_destroy(wounded, HITLOC, crit_hit);
      destroycrit = 0;

      if (t_loc_is_arm) {
        mech_notify(wounded, MECHALL,
                    "Your shoulder joint takes a hit and is frozen!");
        mech_section_actuator_criticals_normalize(wounded, HITLOC);
      } else if (t_loc_is_leg) {
        if (!mech_is_destroyed(wounded) && mech_is_started(wounded)) {
          (void)snprintf(msgbuf, MBUF_SIZE, "'s hip locks into place!");
          mech_los_broadcast(wounded, msgbuf);
        }

        mech_notify(wounded, MECHALL,
                    "Your hip takes a direct hit and freezes up!");

        if (!mech_condition_summary(wounded).hip_damaged) {
          mech_hip_damage_set(wounded, true, false);
        } else {
          if (!mech_is_quad(wounded))
            mech_hip_damage_set(wounded, true, true);
        }

        mech_actuator_criticals_normalize(wounded);

        if (attacker && !mech_is_jumping(wounded) &&
            !mech_is_out_of_control(wounded) &&
            !made_pilot_skill_roll(wounded, 0)) {
          mech_notify(wounded, MECHALL, "You lose your balance and fall down!");
          mech_los_broadcast(wounded, "stumbles and falls down!");
          mech_fall(wounded, 1, 0);
        }
      }
      break;
    case LOWER_ACTUATOR:
    case UPPER_ACTUATOR:
    case HAND_OR_FOOT_ACTUATOR:
      mech_critical_destroy(wounded, HITLOC, crit_hit);
      destroycrit = 0;

      if (t_loc_is_arm) {
        if (special_from_equipment_index(CRIT_TYPE) == HAND_OR_FOOT_ACTUATOR) {
          mech_printf(wounded, MECHALL, "Your %s hand actuator is destroyed!",
                      HITLOC == LARM ? "left" : "right");
        } else {
          mech_printf(wounded, MECHALL, "Your %s %s arm actuator is destroyed!",
                      HITLOC == LARM ? "left" : "right",
                      special_from_equipment_index(CRIT_TYPE) == LOWER_ACTUATOR
                          ? "lower"
                          : "upper");
        }

        if ((special_from_equipment_index(CRIT_TYPE) ==
             HAND_OR_FOOT_ACTUATOR) &&
            mech_section_carries_club(mech, HITLOC))
          mech_drop_club(mech);
        if (mech_carried_dbref(mech) > 0) {
          mech_notify(mech, MECHALL, "The hit causes your tow line to let go!");
          mech_los_broadcast(mech,
                             "'s tow lines release and flap freely behind it!");
          mech_dropoff(GOD, mech, "");
        }
        mech_section_actuator_criticals_normalize(wounded, HITLOC);
      } else if (t_loc_is_leg) {
        mech_notify(wounded, MECHALL,
                    "One of your leg actuators is destroyed!");

        if (mech_critical_is_operational_special(&(CriticalSpecialCheck){
                .mech = wounded,
                .slot = {.section = HITLOC, .critical = 0},
                .special = SHOULDER_OR_HIP})) { /* don't
need to bother with crits if we already have a hip crit here */
          if (!mech_is_destroyed(wounded) && mech_is_started(wounded)) {
            (void)snprintf(msgbuf, MBUF_SIZE, "'s %s twists in an odd way!",
                           locname);
            mech_los_broadcast(wounded, msgbuf);
          }

          mech_actuator_criticals_normalize(wounded);

          if (attacker && !mech_is_jumping(wounded) &&
              !mech_is_out_of_control(wounded) &&
              !made_pilot_skill_roll(wounded, 0)) {
            mech_notify(wounded, MECHALL,
                        "You lose your balance and fall down!");
            mech_los_broadcast(wounded, "stumbles and falls down!");
            mech_fall(wounded, 1, 0);
          }
        }
      }
      break;
    case C3_MASTER:
      temp = mech_c3_working_masters(mech);
      mech_c3_working_masters_set(mech, mech_c3_working_master_count(mech));

      if (temp == mech_c3_working_masters(mech)) {
        mech_notify(wounded, MECHALL,
                    "Your destroyed C3 system takes another hit!");
      } else {
        if (mech_c3_working_masters(mech) == 0) {
          mech_c3_destroyed_set(wounded, true);

          mech_tag_check(mech);
        }

        if (mech_c3_total_master_count(mech))
          mech_notify(wounded, MECHALL,
                      "One of your C3 systems has been destroyed!");
        else
          mech_notify(wounded, MECHALL, "Your C3 system has been destroyed!");
      }

      break;
    case C3_SLAVE:
      mech_c3_destroyed_set(wounded, true);
      mech_notify(wounded, MECHALL, "Your C3 system has been destroyed!");
      break;
    case C3I:
      mech_c3i_destroyed_set(wounded, true);
      mech_notify(wounded, MECHALL, "Your C3i system has been destroyed!");

      mech_c3i_network_clear(mech, 1);
      break;
    case TAG:
      mech_tag_destroyed_set(wounded, true);
      mech_notify(wounded, MECHALL, "Your TAG system has been destroyed!");

      mech_tag_check(mech);
      break;
    case ECM:
      mech_ecm_destroyed_set(wounded, true);
      mech_notify(wounded, MECHALL, "Your ECM system has been destroyed!");
      mech_ecm_modes_disable(wounded);

      if (mech_condition_summary(wounded).stealth_armor_active) {
        mech_notify(wounded, MECHALL, "Your stealth armor system shuts down!");
        mech_stealth_armor_active_set(wounded, false);
      }

      break;
    case ANGELECM:
      mech_angel_ecm_destroyed_set(wounded, true);
      mech_notify(wounded, MECHALL,
                  "Your Angel ECM system has been destroyed!");
      mech_angel_ecm_modes_disable(wounded);

      break;
    case BEAGLE_PROBE:
      mech_beagle_probe_destroyed_set(wounded, true);
      mech_technology_flags_remove(wounded, BEAGLE_PROBE_TECH);
      mech_notify(wounded, MECHALL,
                  "Your Beagle Active Probe has been destroyed!");
      mech_sensors_disable_requiring(wounded, BEAGLE_PROBE_TECH);
      break;
    case BLOODHOUND_PROBE:
      mech_bloodhound_probe_destroyed_set(wounded, true);
      mech_technology_flags_secondary_remove(wounded, BLOODHOUND_PROBE_TECH);
      mech_notify(wounded, MECHALL,
                  "Your Bloodhound Probe has been destroyed!");
      mech_sensors_disable_requiring(wounded, BLOODHOUND_PROBE_TECH);
      break;
    case LIGHT_BAP:
      mech_light_beagle_probe_destroyed_set(wounded, true);
      mech_technology_flags_remove(wounded, LIGHT_BAP_TECH);
      mech_notify(wounded, MECHALL,
                  "Your Light Beagle Active Probe has been destroyed!");
      mech_sensors_disable_requiring(wounded, LIGHT_BAP_TECH);
      break;
    case ARTEMIS_IV:
      weapon_slot = mech_critical_data(wounded, HITLOC, crit_hit);
      if (weapon_slot > NUM_CRITICALS) {
        btech_channel_send(context, BTECH_CHANNEL_MECH_ERRORS,
                           "Artemis IV error on mech %ld", mech_dbref(wounded));
        break;
      }
      mech_critical_ammo_mode_clear(wounded, HITLOC, weapon_slot, ARTEMIS_MODE);
      mech_notify(wounded, MECHALL,
                  "Your Artemis IV system has been destroyed!");
      break;
    case AXE:
      mech_notify(wounded, MECHALL, "Your axe has been destroyed!");
      break;
    case SWORD:
      mech_notify(wounded, MECHALL, "Your sword has been destroyed!");
      break;
    case DUAL_SAW:
      mech_notify(wounded, MECHALL, "Your dual saw has been destroyed!");
      break;
    case MACE:
      mech_notify(wounded, MECHALL, "Your mace has been destroyed!");
      break;
    case DS_AERODOOR:
      mech_notify(wounded, MECHALL,
                  "One of the aero doors has been rendered useless!");
      break;
    case DS_MECHDOOR:
      mech_notify(wounded, MECHALL,
                  "One of the 'mech doors has been rendered useless!");
      [[fallthrough]];
    case NULL_SIGNATURE_SYSTEM:
      mech_notify(wounded, MECHALL,
                  "Your Null Signature System has been destroyed!");

      if (mech_condition_summary(wounded).null_signature_active) {
        mech_notify(wounded, MECHALL, "Your Null Signature System shuts down!");
        mech_null_signature_active_set(wounded, false);
      }

      mech_null_signature_destroyed_set(wounded, true);

      break;
    }

    if (destroycrit)
      mech_critical_destroy(wounded, HITLOC, crit_hit);
  }

  return 1;
}
