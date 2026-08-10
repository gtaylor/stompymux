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
  int fireMode = request->fire_mode;
  int ammoMode = request->ammunition_mode;
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
      (fireMode > -1) && (ammoMode > -1) &&
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
        .ammunition_mode = ammoMode,
        .damage = clear_damage,
        .intentional = true});

    strcpy(buf, "");

    if (weapon_catalogue_is_missile(weapindx))
      (void)snprintf(buf, SBUF_SIZE, "%s%s", "missile",
                     orig_num_missiles > 1 ? "s" : "");
    else if (ammoMode & LBX_MODE)
      (void)snprintf(buf, SBUF_SIZE, "%s%s", "pellet",
                     orig_num_missiles > 1 ? "s" : "");
    else if ((fireMode & ULTRA_MODE) || (fireMode & RFAC_MODE) ||
             (fireMode & RAC_MODES))
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
    this_time = MIN(request->salvo_size, num_missiles_hit);
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
          .ammunition_mode = ammoMode,
          .ignore_swarmers = request->swarm_attack});
    } else {
      mech_terrain_hex_hit(
          &(TerrainWeaponHitRequest){.attacker = mech,
                                     .position = request->target_hex,
                                     .weapon_index = weapindx,
                                     .ammunition_mode = ammoMode,
                                     .damage = this_damage,
                                     .hit = true});
    }

    num_missiles_hit -= this_time;
  }
}

int mech_missile_hit_index(const MissileHitIndexRequest *request) {
  Mech *mech = request->attacker;
  Mech *hitMech = request->target;
  int weapindx = request->weapon.weapon_index;
  int wSection = request->weapon.section;
  int wCritSlot = request->weapon.critical;
  int hit_roll;
  int r1, r2, r3;
  int tHotloading =
      (mech_critical_fire_mode(mech, wSection, wCritSlot) & HOTLOAD_MODE) &&
      weapon_catalogue_supports_indirect_fire(weapindx);
  int wRollInc = 0;
  int wFinalRoll = 0;
  int tUseArtemisBonus =
      mech_critical_ammo_mode(mech, wSection, wCritSlot) & ARTEMIS_MODE;
  int tUseNARCBonus = 0;

  if (hitMech) {
    MechConditionSummary target_condition = mech_condition_summary(hitMech);
    if (target_condition.ecm_protected ||
        target_condition.angel_ecm_protected) {
      tUseArtemisBonus = 0;
      tUseNARCBonus = 0;
    } else {
      tUseNARCBonus =
          (mech_critical_ammo_mode(mech, wSection, wCritSlot) & NARC_MODE) &&
          (mech_has_section_special(hitMech, NARC_ATTACHED) ||
           mech_has_section_special(hitMech, INARC_HOMING_ATTACHED));
    }
  }

  MechConditionSummary firing_condition = mech_condition_summary(mech);
  if (firing_condition.ecm_disturbed || firing_condition.angel_ecm_disturbed) {
    tUseArtemisBonus = 0;
    tUseNARCBonus = 0;
  }

  /*
   * Figure out the modifiers to the roll table for missiles
   */
  if (weapon_catalogue_is_missile(weapindx) &&
      (tUseArtemisBonus || tUseNARCBonus))
    wRollInc = 2;

  /* Roll 3 times... if we're hotloading, we'll use the 2 lowest */
  r1 = btech_random_range_int(mech_context(mech), 1, 6);
  r2 = btech_random_range_int(mech_context(mech), 1, 6);
  r3 = btech_random_range_int(mech_context(mech), 1, 6);

  if (r1 > r2)
    swap_ints(&r1, &r2);
  if (r2 > r3)
    swap_ints(&r2, &r3);

  if (tHotloading)
    hit_roll = r1 + r2 - 2;
  else
    hit_roll = btech_random_roll(mech_context(mech)) - 2;

  if ((!hitMech || !mech_condition_summary(hitMech).angel_ecm_protected) &&
      !firing_condition.angel_ecm_disturbed &&
      weapon_catalogue_is_streak(weapindx)) {
    return 10;
  }

  /* Glancing, per max tech, is -4 off the missile hit table */
  if (request->glancing)
    wRollInc += -4;
  if (wRollInc)
    hit_roll = hit_roll + wRollInc;
  /* Glancing, per max tech, if its lower than 2 on the hit table, we hit with
   * one missile. return -1 so we can test for this elsewhere
   */
  if (request->glancing && (hit_roll < 0))
    return -1;

  wFinalRoll = MAX(MIN(hit_roll, 10), 0);

  return wFinalRoll;
}

int mech_missile_hit_target(const MissileAttackRequest *request) {
  Mech *mech = request->attacker;
  Mech *hitMech = request->target;
  int weapindx = request->weapon.weapon_index;
  int wSection = request->weapon.section;
  int wCritSlot = request->weapon.critical;
  int LOS = request->los;
  int baseToHit = request->base_to_hit;
  int roll = request->roll;
  int incoming = request->incoming;
  int isrear = 0, iscritical = 0;
  int AMSShotdown = 0;
  int hit;
  int wNARCType = 0;
  int ammoMode = mech_critical_ammo_mode(mech, wSection, wCritSlot);
  int tIsInferno = (ammoMode & INFERNO_MODE);
  int wNARCHitLoc = 0;
  int tIsRear = 0;
  char strLocName[30];
  int missileindex = 0;
  /* Check to see if we're a NARC or iNARC launcher firing homing missiles */
  if (weapon_catalogue_is_missile(weapindx)) {
    if (weapon_catalogue_is_narc(weapindx) &&
        !(mech_critical_ammo_mode(mech, wSection, wCritSlot) & NARC_MODE))
      wNARCType = 1;
    else if (weapon_catalogue_is_inarc(weapindx) &&
             !(mech_critical_ammo_mode(mech, wSection, wCritSlot) &
               INARC_EXPLO_MODE)) {

      if (mech_critical_ammo_mode(mech, wSection, wCritSlot) &
          INARC_HAYWIRE_MODE)
        wNARCType = 3;
      else if (mech_critical_ammo_mode(mech, wSection, wCritSlot) &
               INARC_ECM_MODE)
        wNARCType = 4;
      else
        wNARCType = 2;
    }

    /* Prefill our AMS data */
    if (hitMech && (!((ammoMode & SWARM_MODE) || (ammoMode & SWARM1_MODE) ||
                      (ammoMode & MINE_MODE)))) {
      AmsDefenseResult defense = mech_ams_locate_defenses(hitMech);
      if (defense.found) {
        AmsInterceptRequest intercept = {
            .attacker = mech,
            .target = hitMech,
            .incoming = wNARCType ? 1 : incoming,
            .defense = defense,
            .los = LOS,
            .missiles_hit = roll >= baseToHit,
        };
        AMSShotdown = mech_ams_intercept(&intercept);
      }
    }

    if (wNARCType) {
      if (roll >= baseToHit) {
        if (hitMech) {
          if (AMSShotdown > 0) {
            if (LOS)
              mech_notify(mech, MECHALL, "The pod is shot down by the target!");

            mech_notify(hitMech, MECHALL,
                        "Your Anti-Missile System activates and shoots down "
                        "the incoming pod!");

            return 0;
          }

          wNARCHitLoc = mech_narc_hit_location(mech, hitMech, &tIsRear);

          /* sanity check */
          if (wNARCHitLoc < 0) {
            mech_notify(mech, MECHALL,
                        "Your NARC Beacon attaches to the target!");

            return 0;
          }

          ArmorStringFromIndex(wNARCHitLoc, strLocName, mech_class(hitMech),
                               mech_movement_type(hitMech));

          if (wNARCType == 1)
            mech_section_special_add(hitMech, wNARCHitLoc, NARC_ATTACHED);
          else if (wNARCType == 2)
            mech_section_special_add(hitMech, wNARCHitLoc,
                                     INARC_HOMING_ATTACHED);
          else if (wNARCType == 3) {
            mech_section_special_add(hitMech, wNARCHitLoc,
                                     INARC_HAYWIRE_ATTACHED);

            mech_notify(hitMech, MECHALL,
                        "Your targetting system goes a bit haywire!");
          } else if (wNARCType == 4) {
            mech_section_special_add(hitMech, wNARCHitLoc, INARC_ECM_ATTACHED);

            mech_ecm_check(hitMech);
          }

          mech_printf(hitMech, MECHALL,
                      "A NARC Beacon has been attached to your %s%s!",
                      strLocName, tIsRear == 1 ? " (Rear)" : "");
          mech_printf(mech, MECHALL,
                      "Your NARC Beacon attaches to the target's %s%s!",
                      strLocName, tIsRear == 1 ? " (Rear)" : "");
        }
      } else
        mech_notify(mech, MECHALL,
                    "Your NARC Beacon flies off into the distance.");

      return 0;
    }
  }

  if (roll < baseToHit)
    return incoming;
  if (!btech_context_has_missile_hit_table(mech_context(mech), weapindx))
    return 0;

  MissileHitIndexRequest index_request = {
      .attacker = mech,
      .target = hitMech,
      .weapon = request->weapon,
      .glancing = btech_context_glancing_blows_enabled(mech_context(mech)) &&
                  request->player_roll == baseToHit,
  };
  missileindex = mech_missile_hit_index(&index_request);
  if (missileindex < 0)
    hit = MIN(incoming, 1);
  else
    hit = MIN(incoming, btech_context_missile_hit_count(&(MissileHitLookup){
                            .context = mech_context(mech),
                            .weapon = weapindx,
                            .roll = missileindex,
                        }));

  if (LOS) {
    mech_printf(mech, MECHALL, "[fg=green]%s with %d missile%s![reset]",
                LOS == 1 ? "You hit" : "The swarm hits", hit,
                hit > 1 ? "s" : "");
  }

  if (AMSShotdown > 0) {
    if (AMSShotdown >= hit) {
      if (LOS)
        mech_notify(mech, MECHALL,
                    "All of your missiles are shot down by the target!");

      mech_notify(hitMech, MECHALL,
                  "Your Anti-Missile System activates and shoots all the "
                  "incoming missiles!");
    } else {
      mech_printf(mech, MECHALL, "The target shoots down %d of your missiles!",
                  AMSShotdown);

      mech_printf(hitMech, MECHALL,
                  "Your Anti-Missile System activates and shoots down %d "
                  "incoming missiles!",
                  AMSShotdown);
    }
  }

  hit = MAX(0, hit - AMSShotdown);

  if (hit <= 0)
    return 0;

  if (tIsInferno) {
    if (hitMech)
      mech_inferno_hit(mech, hitMech, hit, LOS);
    else
      mech_terrain_hex_hit(&(TerrainWeaponHitRequest){
          .attacker = mech,
          .position = request->target_hex,
          .weapon_index = weapindx,
          .ammunition_mode =
              mech_critical_ammo_mode(mech, wSection, wCritSlot)});
  } else {
    if (btech_context_glancing_blows_enabled(mech_context(mech)) &&
        (request->player_roll == baseToHit) && hitMech) {
      if (!weapon_catalogue_is_streak(weapindx)) {
        mech_los_broadcast(hitMech, "is nicked by a glancing blow!");
        mech_notify(hitMech, MECHALL, "You are nicked by a glancing blow!");
      }
    }
    MissileHitsRequest hits = {
        .attacker = mech,
        .target = hitMech,
        .target_hex = request->target_hex,
        .rear = isrear,
        .critical = iscritical,
        .weapon = request->weapon,
        .fire_mode = mech_critical_fire_mode(mech, wSection, wCritSlot),
        .ammunition_mode = mech_critical_ammo_mode(mech, wSection, wCritSlot),
        .missile_count = hit,
        .damage_per_missile = weapon_catalogue_damage(weapindx),
        .salvo_size = weapon_catalogue_cluster_size(weapindx),
        .los = LOS,
        .base_to_hit = baseToHit,
        .swarm_attack = request->swarm_attack,
    };
    mech_missile_apply_hits(&hits);
  }

  return incoming - hit;
}

void mech_swarm_missile_hit_target(const MissileAttackRequest *request) {
  Mech *mech = request->attacker;
  Mech *hitMech = request->target;
  int weapindx = request->weapon.weapon_index;
  int roll = request->roll;
  enum { MAX_STAR = 10 };
  /* Max # of targets we'll try to hit: 10 */
  Mech *star[MAX_STAR];
  int present_target = 0;
  int missiles;
  BattleMap *map =
      btech_context_find_object(mech_context(mech), mech_map_dbref(mech));
  float r = 0.0, ran = 0, flrange = 0.0;
  Mech *source = mech, *tempMech;
  int i, j;
  if (!btech_context_has_missile_hit_table(mech_context(mech), weapindx))
    return;
  missiles = btech_context_missile_hit_count(&(MissileHitLookup){
      .context = mech_context(mech), .weapon = weapindx, .roll = 10});
  while (missiles > 0) {
    flrange = flrange + mech_range_to(source, hitMech);
    ran = mech_range_to(mech, hitMech);
    const int effective_range = weapon_catalogue_effective_range(
        weapindx,
        btech_context_uses_extended_weapon_ranges(mech_context(mech)));
    if (flrange > (float)effective_range) {
      mech_notify(hitMech, MECHALL, "Luckily, the missiles fall short of you!");
      return;
    }
    MissileAttackRequest attack = *request;
    attack.target = hitMech;
    attack.target_hex = (MapHexPosition){.x = -1, .y = -1};
    attack.los = mech_los_check_unblocked(mech, hitMech, mech_position_x(mech),
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
    *swarm_target_slot(star, MAX_STAR, (size_t)present_target++) = hitMech;
    source = hitMech;
    hitMech = nullptr;
    for (i = 0; i < battle_map_unit_count(map); i++) {
      tempMech = btech_context_find_object(mech_context(mech),
                                           battle_map_unit_dbref(map, i));
      if (tempMech) {
        if (!request->friend_or_foe ||
            (mech_team(tempMech) != mech_team(mech))) {
          for (j = 0; j < present_target; j++)
            if (tempMech == *swarm_target_slot(star, MAX_STAR, (size_t)j))
              break;
          if (mech_condition_summary(tempMech).combat_safe)
            continue;
          if (j != present_target)
            continue;
          if (!hitMech) {
            r = mech_range_to(source, tempMech);
            if (r < 1.9F)
              if (mech_los_check_unblocked(source, tempMech,
                                           mech_position_x(source),
                                           mech_position_y(source), r)) {
                hitMech = tempMech;
              }
          }
        }
      }
    }
    if (!hitMech)
      return;
    if (mech != hitMech)
      mech_notify(hitMech, MECHALL, "The missile-swarm turns towards you!");
    if (mech_los_check_unblocked(mech, source, mech_position_x(mech),
                                 mech_position_y(mech),
                                 mech_range_to(mech, source)))
      mech_printf(
          mech, MECHALL, "Your missile-swarm of %d missile%s targets %s!",
          missiles, missiles > 1 ? "s" : "",
          mech == hitMech ? "YOU!!"
                          : mech_to_mech_display_id(mech, hitMech).text);
    mech_los_broadcast_unit(mech, hitMech, "'s missile-swarm targets %s!");
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
  Mech *hitMech = request->target;
  int incoming = request->incoming;
  int type = request->defense.weapon_type;
  int ammoLoc = request->defense.ammunition_section;
  int ammoCrit = request->defense.ammunition_critical;
  int num_missiles_shotdown;

  if (weapon_catalogue_is_clan_anti_missile(type))
    num_missiles_shotdown = btech_random_roll(mech_context(mech));
  else
    num_missiles_shotdown = btech_random_range_int(mech_context(mech), 1, 6);

  if (num_missiles_shotdown > incoming)
    num_missiles_shotdown = incoming;

  if (num_missiles_shotdown >= mech_critical_data(hitMech, ammoLoc, ammoCrit))
    mech_critical_data_set(hitMech, ammoLoc, ammoCrit, 0);
  else
    mech_critical_data_set(hitMech, ammoLoc, ammoCrit,
                           mech_critical_data(hitMech, ammoLoc, ammoCrit) -
                               num_missiles_shotdown);

  if (!request->missiles_hit) {
    mech_notify(hitMech, MECHALL,
                "Your Anti-Missile System activates and shoots at the incoming "
                "missiles!");
    return 0;
  }

  return num_missiles_shotdown;
}

AmsDefenseResult mech_ams_locate_defenses(Mech *target) {
  AmsDefenseResult result = {0};
  int AMSsect, AMScrit;
  int i, j = 0, w, t = 0;

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
  AMSsect = i;
  AMScrit = j;
  result.weapon_type = w;

  CriticalSlotLookupResult ammunition = ammunition_find(
      &(AmmunitionLookupRequest){.mech = target,
                                 .weapon_index = w,
                                 .start_section = AMSsect,
                                 .forbidden_modes = AMMO_MODES});
  if (!ammunition.found)
    return result;
  result.ammunition_section = ammunition.slot.section;
  result.ammunition_critical = ammunition.slot.critical;

  if (!(mech_critical_data(target, result.ammunition_section,
                           result.ammunition_critical)))
    return result;

  mech_set_recycle_part(
      target, AMSsect, AMScrit,
      btech_context_weapon_recycle_time(mech_context(target), w));
  const int weapon_heat = weapon_catalogue_heat(w);
  mech_weapon_heat_add(target, (float)weapon_heat);
  result.found = true;
  return result;
}

/****************************************
 * END: AMS related functions
 ****************************************/
