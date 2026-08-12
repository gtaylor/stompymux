/* Implements BattleTech combat mechanics for unit combat missile. */

#include "btech/context.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_lifecycle.h"
#include "weapon_catalogue_api.h"
#include <stdio.h>
#include <string.h>

#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "section_types.h"

static void swap_ints(int *left, int *right) {
  int temporary = *left;
  *left = *right;
  *right = temporary;
}

static Mech **swarm_target_slot(Mech **targets, size_t count, size_t index) {
  return (Mech **)checked_storage_at((void *)targets, count, sizeof(*targets),
                                     index);
}

void mech_missile_apply_hits(const MissileHitsRequest *request) {
  Mech *mech = request->attacker;
  Mech *target = request->target;
  int isrear = request->rear;
  int iscritical = request->critical;
  int weapindx = request->weapon.weapon_index;
  int fire_mode = request->fire_mode;
  int ammo_mode = request->ammunition_mode;
  int num_missiles_hit = request->missile_count;
  int damage = request->damage_per_missile;
  int orig_num_missiles = num_missiles_hit;
  int this_time;
  int this_damage;
  int total_damage = 0;
  int clear_damage = 0;
  int hitloc;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char buf[SBUF_SIZE];

  total_damage = num_missiles_hit * damage;

  if (target && btech_context_woods_modify_damage(mech_context(mech)) &&
      battle_terrain_is_forest(map_real_terrain_get(
          mech_map, mech_position_x(target), mech_position_y(target))) &&
      (fire_mode > -1) && (ammo_mode > -1) &&
      ((mech_position_z(target) - 2) <=
       battle_map_hex_elevation(mech_map, mech_position_x(target),
                                mech_position_y(target)))) {
    clear_damage = total_damage;

    if (map_real_terrain_get(mech_map, mech_position_x(target),
                             mech_position_y(target)) == LIGHT_FOREST)
      total_damage -= 2;
    else if (map_real_terrain_get(mech_map, mech_position_x(target),
                                  mech_position_y(target)) == HEAVY_FOREST)
      total_damage -= 4;

    if (total_damage <= 0)
      num_missiles_hit = 0;
    else
      num_missiles_hit = total_damage / damage;

    mech_terrain_possibly_ignite_or_clear(&(TerrainWeaponEffectRequest){
        .mech = mech,
        .position = {.x = mech_position_x(target),
                     .y = mech_position_y(target)},
        .weapon_index = weapindx,
        .ammunition_mode = ammo_mode,
        .damage = clear_damage,
        .intentional = true});

    strcpy(buf, "");

    if (weapon_catalogue_is_missile(weapindx))
      (void)snprintf(buf, SBUF_SIZE, "%s%s", "missile",
                     orig_num_missiles > 1 ? "s" : "");
    else if (ammo_mode & LBX_MODE)
      (void)snprintf(buf, SBUF_SIZE, "%s%s", "pellet",
                     orig_num_missiles > 1 ? "s" : "");
    else if ((fire_mode & ULTRA_MODE) || (fire_mode & RFAC_MODE) ||
             (fire_mode & RAC_MODES))
      (void)snprintf(buf, SBUF_SIZE, "%s%s", "slug",
                     orig_num_missiles > 1 ? "s" : "");
    else
      (void)snprintf(buf, SBUF_SIZE, "%s", "damage");

    mech_printf(mech, MECHALL, "%s %s %s absorbed by the trees!",
                (orig_num_missiles == 1  ? "The"
                 : num_missiles_hit == 0 ? "All of the"
                                         : "Some of the"),
                buf, (orig_num_missiles == 1 ? "is" : "are"));
    mech_printf(target, MECHALL, "The trees absorb %s %s",
                ((orig_num_missiles == 1) || (num_missiles_hit == 0)
                     ? "the"
                     : "some of the"),
                buf);
  }

  while (num_missiles_hit) {
    this_time = min(request->salvo_size, num_missiles_hit);
    this_damage = this_time * damage;

    if (target) {
      hitloc = mech_target_hit_location(mech, target, &isrear, &iscritical);

      mech_damage_apply(&(MechDamageRequest){
          .target = target,
          .attacker = mech,
          .line_of_sight = request->los,
          .attack_pilot = mech_gunner_dbref(mech),
          .hit_location = hitloc,
          .rear = isrear,
          .critical = iscritical,
          .armor_damage =
              personal_combat_damage_to_unit(&(PersonalCombatDamageConversion){
                  .target = target,
                  .weapon_index = weapindx,
                  .damage = this_damage,
              }),
          .internal_damage = 0,
          .transfer = MECH_DAMAGE_NORMAL,
          .cause = weapindx,
          .base_to_hit = request->base_to_hit,
          .weapon_index = weapindx,
          .ammunition_mode = ammo_mode,
          .ignore_swarmers = request->swarm_attack});
    } else {
      mech_terrain_hex_hit(
          &(TerrainWeaponHitRequest){.attacker = mech,
                                     .position = request->target_hex,
                                     .weapon_index = weapindx,
                                     .ammunition_mode = ammo_mode,
                                     .damage = this_damage,
                                     .hit = true});
    }

    num_missiles_hit -= this_time;
  }
}

int mech_missile_hit_index(const MissileHitIndexRequest *request) {
  Mech *mech = request->attacker;
  Mech *hit_mech = request->target;
  int weapindx = request->weapon.weapon_index;
  int w_section = request->weapon.section;
  int w_crit_slot = request->weapon.critical;
  int hit_roll;
  int r1;
  int r2;
  int r3;
  int t_hotloading =
      (mech_critical_fire_mode(mech, w_section, w_crit_slot) & HOTLOAD_MODE) &&
      weapon_catalogue_supports_indirect_fire(weapindx);
  int w_roll_inc = 0;
  int w_final_roll = 0;
  int t_use_artemis_bonus =
      mech_critical_ammo_mode(mech, w_section, w_crit_slot) & ARTEMIS_MODE;
  int t_use_narc_bonus = 0;

  if (hit_mech) {
    MechConditionSummary target_condition = mech_condition_summary(hit_mech);
    if (target_condition.ecm_protected ||
        target_condition.angel_ecm_protected) {
      t_use_artemis_bonus = 0;
      t_use_narc_bonus = 0;
    } else {
      t_use_narc_bonus =
          (mech_critical_ammo_mode(mech, w_section, w_crit_slot) & NARC_MODE) &&
          (mech_has_section_special(hit_mech, NARC_ATTACHED) ||
           mech_has_section_special(hit_mech, INARC_HOMING_ATTACHED));
    }
  }

  MechConditionSummary firing_condition = mech_condition_summary(mech);
  if (firing_condition.ecm_disturbed || firing_condition.angel_ecm_disturbed) {
    t_use_artemis_bonus = 0;
    t_use_narc_bonus = 0;
  }

  /*
   * Figure out the modifiers to the roll table for missiles
   */
  if (weapon_catalogue_is_missile(weapindx) &&
      (t_use_artemis_bonus || t_use_narc_bonus))
    w_roll_inc = 2;

  /* Roll 3 times... if we're hotloading, we'll use the 2 lowest */
  r1 = btech_random_range_int(mech_context(mech), 1, 6);
  r2 = btech_random_range_int(mech_context(mech), 1, 6);
  r3 = btech_random_range_int(mech_context(mech), 1, 6);

  if (r1 > r2)
    swap_ints(&r1, &r2);
  if (r2 > r3)
    swap_ints(&r2, &r3);

  if (t_hotloading)
    hit_roll = r1 + r2 - 2;
  else
    hit_roll = btech_random_roll(mech_context(mech)) - 2;

  if ((!hit_mech || !mech_condition_summary(hit_mech).angel_ecm_protected) &&
      !firing_condition.angel_ecm_disturbed &&
      weapon_catalogue_is_streak(weapindx)) {
    return 10;
  }

  /* Glancing, per max tech, is -4 off the missile hit table */
  if (request->glancing)
    w_roll_inc += -4;
  if (w_roll_inc)
    hit_roll = hit_roll + w_roll_inc;
  /* Glancing, per max tech, if its lower than 2 on the hit table, we hit with
   * one missile. return -1 so we can test for this elsewhere
   */
  if (request->glancing && (hit_roll < 0))
    return -1;

  w_final_roll = max(min(hit_roll, 10), 0);

  return w_final_roll;
}

int mech_missile_hit_target(const MissileAttackRequest *request) {
  Mech *mech = request->attacker;
  Mech *hit_mech = request->target;
  int weapindx = request->weapon.weapon_index;
  int w_section = request->weapon.section;
  int w_crit_slot = request->weapon.critical;
  int los = request->los;
  int base_to_hit = request->base_to_hit;
  int roll = request->roll;
  int incoming = request->incoming;
  int isrear = 0;
  int iscritical = 0;
  int ams_shotdown = 0;
  int hit;
  int w_narc_type = 0;
  int ammo_mode = mech_critical_ammo_mode(mech, w_section, w_crit_slot);
  int t_is_inferno = (ammo_mode & INFERNO_MODE);
  int w_narc_hit_loc = 0;
  int t_is_rear = 0;
  char str_loc_name[30];
  int missileindex = 0;
  /* Check to see if we're a NARC or iNARC launcher firing homing missiles */
  if (weapon_catalogue_is_missile(weapindx)) {
    if (weapon_catalogue_is_narc(weapindx) &&
        !(mech_critical_ammo_mode(mech, w_section, w_crit_slot) & NARC_MODE)) {
      w_narc_type = 1;
    } else if (weapon_catalogue_is_inarc(weapindx) &&
               !(mech_critical_ammo_mode(mech, w_section, w_crit_slot) &
                 INARC_EXPLO_MODE)) {

      if (mech_critical_ammo_mode(mech, w_section, w_crit_slot) &
          INARC_HAYWIRE_MODE)
        w_narc_type = 3;
      else if (mech_critical_ammo_mode(mech, w_section, w_crit_slot) &
               INARC_ECM_MODE)
        w_narc_type = 4;
      else
        w_narc_type = 2;
    }

    /* Prefill our AMS data */
    if (hit_mech && (!((ammo_mode & SWARM_MODE) || (ammo_mode & SWARM1_MODE) ||
                       (ammo_mode & MINE_MODE)))) {
      AmsDefenseResult defense = mech_ams_locate_defenses(hit_mech);
      if (defense.found) {
        AmsInterceptRequest intercept = {
            .attacker = mech,
            .target = hit_mech,
            .incoming = w_narc_type ? 1 : incoming,
            .defense = defense,
            .los = los,
            .missiles_hit = roll >= base_to_hit,
        };
        ams_shotdown = mech_ams_intercept(&intercept);
      }
    }

    if (w_narc_type) {
      if (roll >= base_to_hit) {
        if (hit_mech) {
          if (ams_shotdown > 0) {
            if (los)
              mech_notify(mech, MECHALL, "The pod is shot down by the target!");

            mech_notify(hit_mech, MECHALL,
                        "Your Anti-Missile System activates and shoots down "
                        "the incoming pod!");

            return 0;
          }

          w_narc_hit_loc = mech_narc_hit_location(mech, hit_mech, &t_is_rear);

          /* sanity check */
          if (w_narc_hit_loc < 0) {
            mech_notify(mech, MECHALL,
                        "Your NARC Beacon attaches to the target!");

            return 0;
          }

          armor_string_from_index(w_narc_hit_loc, str_loc_name,
                                  mech_class(hit_mech),
                                  mech_movement_type(hit_mech));

          if (w_narc_type == 1) {
            mech_section_special_add(hit_mech, w_narc_hit_loc, NARC_ATTACHED);
          } else if (w_narc_type == 2) {
            mech_section_special_add(hit_mech, w_narc_hit_loc,
                                     INARC_HOMING_ATTACHED);
          } else if (w_narc_type == 3) {
            mech_section_special_add(hit_mech, w_narc_hit_loc,
                                     INARC_HAYWIRE_ATTACHED);

            mech_notify(hit_mech, MECHALL,
                        "Your targetting system goes a bit haywire!");
          } else if (w_narc_type == 4) {
            mech_section_special_add(hit_mech, w_narc_hit_loc,
                                     INARC_ECM_ATTACHED);

            mech_ecm_check(hit_mech);
          }

          mech_printf(hit_mech, MECHALL,
                      "A NARC Beacon has been attached to your %s%s!",
                      str_loc_name, t_is_rear == 1 ? " (Rear)" : "");
          mech_printf(mech, MECHALL,
                      "Your NARC Beacon attaches to the target's %s%s!",
                      str_loc_name, t_is_rear == 1 ? " (Rear)" : "");
        }
      } else {
        mech_notify(mech, MECHALL,
                    "Your NARC Beacon flies off into the distance.");
      }

      return 0;
    }
  }

  if (roll < base_to_hit)
    return incoming;
  if (!btech_context_has_missile_hit_table(mech_context(mech), weapindx))
    return 0;

  MissileHitIndexRequest index_request = {
      .attacker = mech,
      .target = hit_mech,
      .weapon = request->weapon,
      .glancing = btech_context_glancing_blows_enabled(mech_context(mech)) &&
                  request->player_roll == base_to_hit,
  };
  missileindex = mech_missile_hit_index(&index_request);
  if (missileindex < 0)
    hit = min(incoming, 1);
  else {
    hit = min(incoming, btech_context_missile_hit_count(&(MissileHitLookup){
                            .context = mech_context(mech),
                            .weapon = weapindx,
                            .roll = missileindex,
                        }));
  }

  if (los) {
    mech_printf(mech, MECHALL, "[fg=green]%s with %d missile%s![reset]",
                los == 1 ? "You hit" : "The swarm hits", hit,
                hit > 1 ? "s" : "");
  }

  if (ams_shotdown > 0) {
    if (ams_shotdown >= hit) {
      if (los)
        mech_notify(mech, MECHALL,
                    "All of your missiles are shot down by the target!");

      mech_notify(hit_mech, MECHALL,
                  "Your Anti-Missile System activates and shoots all the "
                  "incoming missiles!");
    } else {
      mech_printf(mech, MECHALL, "The target shoots down %d of your missiles!",
                  ams_shotdown);

      mech_printf(hit_mech, MECHALL,
                  "Your Anti-Missile System activates and shoots down %d "
                  "incoming missiles!",
                  ams_shotdown);
    }
  }

  hit = max(0, hit - ams_shotdown);

  if (hit <= 0)
    return 0;

  if (t_is_inferno) {
    if (hit_mech)
      mech_inferno_hit(mech, hit_mech, hit, los);
    else {
      mech_terrain_hex_hit(&(TerrainWeaponHitRequest){
          .attacker = mech,
          .position = request->target_hex,
          .weapon_index = weapindx,
          .ammunition_mode =
              mech_critical_ammo_mode(mech, w_section, w_crit_slot)});
    }
  } else {
    if (btech_context_glancing_blows_enabled(mech_context(mech)) &&
        (request->player_roll == base_to_hit) && hit_mech) {
      if (!weapon_catalogue_is_streak(weapindx)) {
        mech_los_broadcast(hit_mech, "is nicked by a glancing blow!");
        mech_notify(hit_mech, MECHALL, "You are nicked by a glancing blow!");
      }
    }
    MissileHitsRequest hits = {
        .attacker = mech,
        .target = hit_mech,
        .target_hex = request->target_hex,
        .rear = isrear,
        .critical = iscritical,
        .weapon = request->weapon,
        .fire_mode = mech_critical_fire_mode(mech, w_section, w_crit_slot),
        .ammunition_mode =
            mech_critical_ammo_mode(mech, w_section, w_crit_slot),
        .missile_count = hit,
        .damage_per_missile = weapon_catalogue_damage(weapindx),
        .salvo_size = weapon_catalogue_cluster_size(weapindx),
        .los = los,
        .base_to_hit = base_to_hit,
        .swarm_attack = request->swarm_attack,
    };
    mech_missile_apply_hits(&hits);
  }

  return incoming - hit;
}

void mech_swarm_missile_hit_target(const MissileAttackRequest *request) {
  Mech *mech = request->attacker;
  Mech *hit_mech = request->target;
  int weapindx = request->weapon.weapon_index;
  int roll = request->roll;
  enum { MAX_STAR = 10 };
  /* Max # of targets we'll try to hit: 10 */
  Mech *star[MAX_STAR];
  int present_target = 0;
  int missiles;
  BattleMap *map =
      btech_context_find_object(mech_context(mech), mech_map_dbref(mech));
  float r = 0.0;
  float ran = 0;
  float flrange = 0.0;
  Mech *source = mech;
  Mech *temp_mech;
  int i;
  int j;
  if (!btech_context_has_missile_hit_table(mech_context(mech), weapindx))
    return;
  missiles = btech_context_missile_hit_count(&(MissileHitLookup){
      .context = mech_context(mech), .weapon = weapindx, .roll = 10});
  while (missiles > 0) {
    flrange = flrange + mech_range_to(source, hit_mech);
    ran = mech_range_to(mech, hit_mech);
    const int EFFECTIVE_RANGE = weapon_catalogue_effective_range(
        weapindx,
        btech_context_uses_extended_weapon_ranges(mech_context(mech)));
    if (flrange > (float)EFFECTIVE_RANGE) {
      mech_notify(hit_mech, MECHALL,
                  "Luckily, the missiles fall short of you!");
      return;
    }
    MissileAttackRequest attack = *request;
    attack.target = hit_mech;
    attack.target_hex = (MapHexPosition){.x = -1, .y = -1};
    attack.los = mech_los_check_unblocked(mech, hit_mech, mech_position_x(mech),
                                          mech_position_y(mech), ran)
                     ? present_target == 0 ? 1 : 2
                     : 0;
    attack.roll =
        present_target == 0 ? roll : btech_random_roll(mech_context(mech));
    attack.incoming = missiles;
    missiles = mech_missile_hit_target(&attack);
    if (!missiles)
      return;
    /* Try to acquire a new target NOT in the star */
    if (present_target == MAX_STAR)
      return;
    *swarm_target_slot(star, MAX_STAR, (size_t)present_target++) = hit_mech;
    source = hit_mech;
    hit_mech = nullptr;
    for (i = 0; i < battle_map_unit_count(map); i++) {
      temp_mech = btech_context_find_object(mech_context(mech),
                                            battle_map_unit_dbref(map, i));
      if (temp_mech) {
        if (!request->friend_or_foe ||
            (mech_team(temp_mech) != mech_team(mech))) {
          for (j = 0; j < present_target; j++)
            if (temp_mech == *swarm_target_slot(star, MAX_STAR, (size_t)j))
              break;
          if (mech_condition_summary(temp_mech).combat_safe)
            continue;
          if (j != present_target)
            continue;
          if (!hit_mech) {
            r = mech_range_to(source, temp_mech);
            if (r < 1.9F) {
              if (mech_los_check_unblocked(source, temp_mech,
                                           mech_position_x(source),
                                           mech_position_y(source), r)) {
                hit_mech = temp_mech;
              }
            }
          }
        }
      }
    }
    if (!hit_mech)
      return;
    if (mech != hit_mech)
      mech_notify(hit_mech, MECHALL, "The missile-swarm turns towards you!");
    if (mech_los_check_unblocked(mech, source, mech_position_x(mech),
                                 mech_position_y(mech),
                                 mech_range_to(mech, source))) {
      mech_printf(
          mech, MECHALL, "Your missile-swarm of %d missile%s targets %s!",
          missiles, missiles > 1 ? "s" : "",
          mech == hit_mech ? "YOU!!"
                           : mech_to_mech_display_id(mech, hit_mech).text);
    }
    mech_los_broadcast_unit(mech, hit_mech, "'s missile-swarm targets %s!");
  }
}

/*
 * Fix AMS:
 *
 * - Applied after number missiles is determined
 * - Ammo used == missiles shot down
 * - d6 for IS, 2d6 for clan
 * - Not used against Arrow IV, Thunder, Flare, Swarm or Swarm-1
 */

/****************************************
 * START: AMS related functions
 ****************************************/
int mech_ams_intercept(const AmsInterceptRequest *request) {
  Mech *mech = request->attacker;
  Mech *hit_mech = request->target;
  int incoming = request->incoming;
  int type = request->defense.weapon_type;
  int ammo_loc = request->defense.ammunition_section;
  int ammo_crit = request->defense.ammunition_critical;
  int num_missiles_shotdown;

  if (weapon_catalogue_is_clan_anti_missile(type))
    num_missiles_shotdown = btech_random_roll(mech_context(mech));
  else
    num_missiles_shotdown = btech_random_range_int(mech_context(mech), 1, 6);

  if (num_missiles_shotdown > incoming)
    num_missiles_shotdown = incoming;

  if (num_missiles_shotdown >=
      mech_critical_data(hit_mech, ammo_loc, ammo_crit))
    mech_critical_data_set(hit_mech, ammo_loc, ammo_crit, 0);
  else
    mech_critical_data_set(hit_mech, ammo_loc, ammo_crit,
                           mech_critical_data(hit_mech, ammo_loc, ammo_crit) -
                               num_missiles_shotdown);

  if (!request->missiles_hit) {
    mech_notify(hit_mech, MECHALL,
                "Your Anti-Missile System activates and shoots at the incoming "
                "missiles!");
    return 0;
  }

  return num_missiles_shotdown;
}

AmsDefenseResult mech_ams_locate_defenses(Mech *target) {
  AmsDefenseResult result = {0};
  int am_ssect;
  int am_scrit;
  int i;
  int j = 0;
  int w;
  int t = 0;

  if (!(mech_technology_flags(target) &
        (IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH)) ||
      !mech_is_started(target) || !mech_condition_summary(target).ams_enabled)
    return result;

  for (i = 0; i < NUM_SECTIONS; i++) {
    for (j = 0; j < NUM_CRITICALS; j++) {
      t = mech_critical_part_type(target, i, j);
      if (equipment_is_weapon(t))
        if (weapon_catalogue_is_anti_missile(weapon_from_equipment_index(t)))
          if (!(mech_critical_is_nonfunctional(target, i, j) ||
                mech_weapon_is_recycling_at(target, i, j)))
            break;
    }
    if (j < NUM_CRITICALS)
      break;
  }

  if (i == NUM_SECTIONS)
    return result;

  w = weapon_from_equipment_index(t);
  am_ssect = i;
  am_scrit = j;
  result.weapon_type = w;

  CriticalSlotLookupResult ammunition = ammunition_find(
      &(AmmunitionLookupRequest){.mech = target,
                                 .weapon_index = w,
                                 .start_section = am_ssect,
                                 .forbidden_modes = AMMO_MODES});
  if (!ammunition.found)
    return result;
  result.ammunition_section = ammunition.slot.section;
  result.ammunition_critical = ammunition.slot.critical;

  if (!(mech_critical_data(target, result.ammunition_section,
                           result.ammunition_critical)))
    return result;

  mech_set_recycle_part(
      target, am_ssect, am_scrit,
      btech_context_weapon_recycle_time(mech_context(target), w));
  const int WEAPON_HEAT = weapon_catalogue_heat(w);
  mech_weapon_heat_add(target, (float)WEAPON_HEAT);
  result.found = true;
  return result;
}

/****************************************
 * END: AMS related functions
 ****************************************/
