#include "artillery_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "failures.h"
#include "map_coordinates.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_combat.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_preparation_internal.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_progress_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_spot_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include <math.h>
#include <stdio.h>
#include <string.h>
static const char *weapon_display_name(int weapon_index) {
  return checked_string_suffix(weapon_catalogue_name(weapon_index), 3);
}
void mech_weapon_fire(const WeaponFireRequest *request) {
  float range = request->range;
  Mech *altTarget;
  int baseToHit, RbaseToHit;
  int roll;
  int type = -1, modifier;
  const bool is_artillery =
      weapon_catalogue_is_artillery(request->weapon_index);
  bool range_ok = true;
  int wGattlingShots =
      0; /* If we're a gattling MG, then we need to figure out how many shots */
  char buf[SBUF_SIZE] = {0};
  char buf3[SBUF_SIZE] = {0};
  char buf2[LBUF_SIZE] = {0};
  int wRACHeat = 0;
  int wHGRPSkillMod = 0;
  bool swarm_attack;
  DbRef c3Ref;
  Mech *c3Mech;
  int firstCrit = 0;
  const WeaponFirePreparation preparation = weapon_fire_prepare(request, range);
  if (!preparation.ready)
    return;
  Mech *target = preparation.target;
  const AmmunitionCheckResult ammunition = preparation.ammunition;
  wGattlingShots = ammunition.gatling_shots;
  baseToHit = preparation.base_to_hit;
  swarm_attack = preparation.swarm_attack;
  c3Ref = preparation.c3_reference;
  c3Mech = preparation.c3_mech;
  roll = weapon_fire_roll(request, range);
  if (request->line_of_sight)
    (void)snprintf(buf, sizeof(buf), "Roll: %d ", roll);
  if (target && !request->target_kind) {
    range = mech_range_to(request->mech, target);
    strcpy(buf2, "");
    if (mech_aim_section(request->mech) != NUM_SECTIONS &&
        mech_aim_unit_class(request->mech) == mech_class(target) &&
        !weapon_catalogue_is_missile(request->weapon_index)) {
      ArmorStringFromIndex(mech_aim_section(request->mech), buf3,
                           mech_class(target), mech_movement_type(target));
      (void)snprintf(buf2, sizeof(buf2), "'s %s", buf3);
    }
    if (request->sight) {
      if (baseToHit >= 900) {
        mech_notify(request->mech, MECHALL,
                    tprintf("You aim %s at %s%s - Out of range.",
                            weapon_display_name(request->weapon_index),
                            mech_to_mech_display_id(request->mech, target).text,
                            buf2));
        return;
      }
      mech_c3_track_emit(request->mech, c3Ref, c3Mech);
      mech_printf(
          request->mech, MECHALL, "You aim %s at %s%s - BTH: %d %s",
          weapon_display_name(request->weapon_index),
          mech_to_mech_display_id(request->mech, target).text, buf2, baseToHit,
          mech_condition_summary(target).partial_cover ? "(Partial cover)"
                                                       : "");
      return;
    }
    if (baseToHit > 12) {
      if (baseToHit >= 900) {
        mech_notify(request->mech, MECHALL,
                    tprintf("Fire %s at %s%s - Out of range.",
                            weapon_display_name(request->weapon_index),
                            mech_to_mech_display_id(request->mech, target).text,
                            buf2));
        return;
      }
      mech_printf(
          request->mech, MECHALL,
          "Fire %s at %s%s - BTH: %d  Roll: Impossible! %s",
          weapon_display_name(request->weapon_index),
          mech_to_mech_display_id(request->mech, target).text, buf2, baseToHit,
          mech_condition_summary(target).partial_cover ? "(Partial cover)"
                                                       : "");
      return;
    }
  } else {
    if (request->sight) {
      if (baseToHit > 900)
        mech_printf(request->mech, MECHPILOT,
                    "You aim your %s at (%d,%d) - Out of Range.",
                    weapon_display_name(request->weapon_index),
                    request->target_hex.x, request->target_hex.y);
      else {
        mech_c3_track_emit(request->mech, c3Ref, c3Mech);
        mech_printf(request->mech, MECHPILOT,
                    "You aim your %s at (%d,%d) - BTH: %d",
                    weapon_display_name(request->weapon_index),
                    request->target_hex.x, request->target_hex.y, baseToHit);
      }
      return;
    }
    if (!is_artillery && baseToHit > 12) {
      mech_printf(request->mech, MECHALL,
                  "Fire %s at (%d,%d) - BTH: %d  Roll: Impossible!",
                  weapon_display_name(request->weapon_index),
                  request->target_hex.x, request->target_hex.y, baseToHit);
      return;
    }
  }
  if (target && !request->target_kind) {
    mech_c3_track_emit(request->mech, c3Ref, c3Mech);
    mech_printf(request->mech, MECHALL, "You fire %s at %s%s - BTH: %d  %s%s",
                weapon_display_name(request->weapon_index),
                mech_to_mech_display_id(request->mech, target).text, buf2,
                baseToHit, buf,
                mech_condition_summary(target).partial_cover ? "(Partial cover)"
                                                             : "");
    btech_channel_send(
        mech_context(request->mech), BTECH_CHANNEL_MECH_ATTACKS, "%s",
        tprintf("#%li attacks #%li (weapon) (%i/%i)", mech_dbref(request->mech),
                mech_dbref(target), baseToHit, roll));
    if (mech_condition_summary(target).attack_emissions)
      btech_channel_send(mech_context(request->mech),
                         BTECH_CHANNEL_MECH_ATTACK_EMITS, "%s",
                         tprintf("#%li attacks #%li (weapon) (%i/%i)",
                                 mech_dbref(request->mech), mech_dbref(target),
                                 baseToHit, roll));
  } else {
    mech_c3_track_emit(request->mech, c3Ref, c3Mech);
    mech_printf(request->mech, MECHALL, "You fire %s %s (%d,%d) - BTH: %d  %s",
                weapon_display_name(request->weapon_index),
                mech_hex_target_description(request->mech),
                request->target_hex.x, request->target_hex.y, baseToHit, buf);
    btech_channel_send(
        mech_context(request->mech), BTECH_CHANNEL_MECH_ATTACKS, "%s",
        tprintf("#%li attacks %d,%d (%s) (weapon) (%i/%i)",
                mech_dbref(request->mech), request->target_hex.x,
                request->target_hex.y,
                mech_hex_target_short_name(request->mech), baseToHit, roll));
    {
      Mech *tmpmech;
      int foo;
      for (foo = 0; foo < battle_map_unit_count(request->map); foo++) {
        DbRef unit_dbref = battle_map_unit_dbref(request->map, foo);
        if (unit_dbref >= 0) {
          tmpmech =
              btech_context_get_mech(mech_context(request->mech), unit_dbref);
          if (!tmpmech)
            continue;
          if (mech_dbref(request->mech) == mech_dbref(tmpmech))
            continue;
          if (mech_position_x(tmpmech) != request->target_hex.x &&
              mech_position_y(tmpmech) != request->target_hex.y)
            continue;
          if (mech_condition_summary(tmpmech).attack_emissions)
            btech_channel_send(
                mech_context(request->mech), BTECH_CHANNEL_MECH_ATTACK_EMITS,
                "%s",
                tprintf("#%li attacks %d,%d (%s) (weapon)"
                        " (%i/%i)",
                        mech_dbref(request->mech), request->target_hex.x,
                        request->target_hex.y,
                        mech_hex_target_short_name(request->mech), baseToHit,
                        roll));
        }
      }
    }
  }
  WeaponFailureResolution failure =
      weapon_failure_resolve(&(WeaponFailureResolutionRequest){
          .mech = request->mech,
          .weapon_number = request->weapon_number,
          .weapon_index = request->weapon_index,
          .weapon = {.section = request->weapon.section,
                     .critical = request->weapon.critical},
          .primary_ammunition = ammunition.primary,
          .secondary_ammunition = ammunition.secondary,
          .range = range,
          .gatling_shots = wGattlingShots});
  modifier = failure.modifier;
  type = failure.type;
  range_ok = failure.range_ok;
  if (failure.handled)
    return;
  if (weapon_catalogue_is_streak(request->weapon_index)) {
    if (target && (mech_condition_summary(request->mech).angel_ecm_disturbed ||
                   mech_condition_summary(target).angel_ecm_protected))
      mech_notify(request->mech, MECHALL,
                  "The ECM confuses your streak homing system!");
    else if (roll < baseToHit) {
      mech_set_recycle_part(request->mech, request->weapon.section,
                            request->weapon.critical,
                            WEAPON_TICK * btech_context_weapon_recycle_time(
                                              mech_context(request->mech),
                                              request->weapon_index));
      mech_notify(request->mech, MECHALL, "Your streak fails to lock on.");
      return;
    }
  }
  if (mech_critical_fire_mode(request->mech, request->weapon.section,
                              request->weapon.critical) &
      HOTLOAD_MODE) {
    if (roll == 2 || roll == 3) {
      mech_printf(
          request->mech, MECHALL,
          "[fg=red bold]The ammo loading mechanism jams on your %s![reset]",
          weapon_display_name(request->weapon_index));
      mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
          .mech = request->mech,
          .slot = {.section = request->weapon.section,
                   .critical = request->weapon.critical},
          .failure = FAIL_AMMOJAMMED});
      return;
    }
  }
  if (mech_critical_ammo_mode(request->mech, request->weapon.section,
                              request->weapon.critical) &
      AC_CASELESS_MODE) {
    if (roll == 2 || roll == 3) {
      mech_printf(
          request->mech, MECHALL,
          "[fg=red bold]The ammo loading mechanism jams on your %s![reset]",
          weapon_display_name(request->weapon_index));
      mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
          .mech = request->mech,
          .slot = {.section = request->weapon.section,
                   .critical = request->weapon.critical},
          .failure = FAIL_AMMOJAMMED});
      if (btech_random_roll(mech_context(request->mech)) > 7) {
        mech_printf(request->mech, MECHALL,
                    "[fg=red bold]Propellant from your %s ignites and "
                    "destroys it![reset]",
                    weapon_display_name(request->weapon_index));
        firstCrit = mech_weapon_first_critical(&(WeaponCriticalSearch){
            .mech = request->mech,
            .weapon = {.section = request->weapon.section, .critical = -1},
            .start_critical = 0,
            .part_type = weapon_equipment_index(request->weapon_index),
            .maximum_criticals =
                GetWeaponCrits(request->mech, request->weapon_index),
        });
        mech_weapon_destroy(&(WeaponDestructionRequest){
            .mech = request->mech,
            .first = {.section = request->weapon.section,
                      .critical = firstCrit},
            .part_type = weapon_equipment_index(request->weapon_index),
            .criticals_to_destroy =
                GetWeaponCrits(request->mech, request->weapon_index),
            .total_criticals =
                GetWeaponCrits(request->mech, request->weapon_index)});
        mech_los_broadcast(request->mech,
                           "shudders from an internal explosion!");
        mech_damage_apply(&(MechDamageRequest){
            .target = request->mech,
            .attacker = request->mech,
            .line_of_sight = 0,
            .attack_pilot = -1,
            .hit_location = request->weapon.section,
            .rear = 0,
            .critical = 1,
            .armor_damage = 0,
            .internal_damage = weapon_catalogue_damage(request->weapon_index),
            .transfer = MECH_DAMAGE_NORMAL,
            .cause = -1,
            .base_to_hit = 0,
            .weapon_index = -1,
            .ammunition_mode = 0,
            .ignore_swarmers = 1});
        mech_ammunition_decrement(&(AmmunitionDecrementRequest){
            .mech = request->mech,
            .weapon_index = request->weapon_index,
            .weapon = {.section = request->weapon.section,
                       .critical = request->weapon.critical},
            .primary_ammunition = ammunition.primary,
            .secondary_ammunition = ammunition.secondary,
            .gatling_shots = wGattlingShots,
        });
      }
      return;
    }
  }
  if (mech_critical_fire_mode(request->mech, request->weapon.section,
                              request->weapon.critical) &
      RFAC_MODE) {
    if (roll == 2) {
      mech_printf(
          request->mech, MECHALL,
          "[fg=red bold]A catastrophic misload on your %s destroys it and "
          "causes an internal explosion![reset]",
          weapon_display_name(request->weapon_index));
      firstCrit = mech_weapon_first_critical(&(WeaponCriticalSearch){
          .mech = request->mech,
          .weapon = {.section = request->weapon.section, .critical = -1},
          .start_critical = 0,
          .part_type = weapon_equipment_index(request->weapon_index),
          .maximum_criticals =
              GetWeaponCrits(request->mech, request->weapon_index),
      });
      mech_weapon_destroy(&(WeaponDestructionRequest){
          .mech = request->mech,
          .first = {.section = request->weapon.section, .critical = firstCrit},
          .part_type = weapon_equipment_index(request->weapon_index),
          .criticals_to_destroy =
              GetWeaponCrits(request->mech, request->weapon_index),
          .total_criticals =
              GetWeaponCrits(request->mech, request->weapon_index)});
      mech_los_broadcast(request->mech, "shudders from an internal explosion!");
      mech_damage_apply(&(MechDamageRequest){
          .target = request->mech,
          .attacker = request->mech,
          .line_of_sight = 0,
          .attack_pilot = -1,
          .hit_location = request->weapon.section,
          .rear = 0,
          .critical = 0,
          .armor_damage = 0,
          .internal_damage = weapon_catalogue_damage(request->weapon_index),
          .transfer = MECH_DAMAGE_NORMAL,
          .cause = -1,
          .base_to_hit = 0,
          .weapon_index = -1,
          .ammunition_mode = 0,
          .ignore_swarmers = 1});
      mech_ammunition_decrement(&(AmmunitionDecrementRequest){
          .mech = request->mech,
          .weapon_index = request->weapon_index,
          .weapon = {.section = request->weapon.section,
                     .critical = request->weapon.critical},
          .primary_ammunition = ammunition.primary,
          .secondary_ammunition = ammunition.secondary,
          .gatling_shots = wGattlingShots,
      });
      return;
    } else if (roll < 5) {
      mech_printf(
          request->mech, MECHALL,
          "[fg=red bold]The ammo loader mechanism jams on your %s![reset]",
          weapon_display_name(request->weapon_index));
      mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
          .mech = request->mech,
          .slot = {.section = request->weapon.section,
                   .critical = request->weapon.critical},
          .failure = FAIL_AMMOJAMMED});
      return;
    }
  }
  if (weapon_catalogue_is_rotary_autocannon(request->weapon_index)) {
    if (((mech_critical_fire_mode(request->mech, request->weapon.section,
                                  request->weapon.critical) &
          RAC_TWOSHOT_MODE) &&
         (roll == 2)) ||
        ((mech_critical_fire_mode(request->mech, request->weapon.section,
                                  request->weapon.critical) &
          RAC_FOURSHOT_MODE) &&
         (roll <= 3)) ||
        ((mech_critical_fire_mode(request->mech, request->weapon.section,
                                  request->weapon.critical) &
          RAC_SIXSHOT_MODE) &&
         (roll <= 4))) {
      mech_printf(
          request->mech, MECHALL,
          "[fg=red bold]The ammo loader mechanism jams on your %s![reset]",
          weapon_display_name(request->weapon_index));
      mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
          .mech = request->mech,
          .slot = {.section = request->weapon.section,
                   .critical = request->weapon.critical},
          .failure = FAIL_AMMOJAMMED});
      return;
    }
  }
  if (mech_critical_fire_mode(request->mech, request->weapon.section,
                              request->weapon.critical) &
      ULTRA_MODE) {
    if (roll == 2) {
      mech_printf(request->mech, MECHALL,
                  "The loader jams on your %s, destroying it!",
                  weapon_display_name(request->weapon_index));
      firstCrit = mech_weapon_first_critical(&(WeaponCriticalSearch){
          .mech = request->mech,
          .weapon = {.section = request->weapon.section, .critical = -1},
          .start_critical = 0,
          .part_type = weapon_equipment_index(request->weapon_index),
          .maximum_criticals =
              GetWeaponCrits(request->mech, request->weapon_index),
      });
      mech_weapon_destroy(&(WeaponDestructionRequest){
          .mech = request->mech,
          .first = {.section = request->weapon.section, .critical = firstCrit},
          .part_type = weapon_equipment_index(request->weapon_index),
          .criticals_to_destroy =
              GetWeaponCrits(request->mech, request->weapon_index),
          .total_criticals =
              GetWeaponCrits(request->mech, request->weapon_index)});
      return;
    }
  }
  if (mech_weapon_critical_can_explode(
          &(WeaponCriticalRoll){.mech = request->mech,
                                .slot = {.section = request->weapon.section,
                                         .critical = request->weapon.critical},
                                .roll = roll})) {
    if (weapon_catalogue_is_energy(request->weapon_index)) {
      mech_printf(request->mech, MECHALL,
                  "[fg=red bold]The damaged charging crystal on your %s "
                  "overloads![reset]",
                  weapon_display_name(request->weapon_index));
    } else {
      mech_printf(request->mech, MECHALL,
                  "[fg=red bold]The damaged ammo feed on your %s triggers an "
                  "internal explosion![reset]",
                  weapon_display_name(request->weapon_index));
      mech_ammunition_decrement(&(AmmunitionDecrementRequest){
          .mech = request->mech,
          .weapon_index = request->weapon_index,
          .weapon = {.section = request->weapon.section,
                     .critical = request->weapon.critical},
          .primary_ammunition = ammunition.primary,
          .secondary_ammunition = ammunition.secondary,
          .gatling_shots = wGattlingShots,
      });
    }
    mech_los_broadcast(request->mech, "shudders from an internal explosion!");
    firstCrit = mech_weapon_first_critical(&(WeaponCriticalSearch){
        .mech = request->mech,
        .weapon = {.section = request->weapon.section, .critical = -1},
        .start_critical = 0,
        .part_type = weapon_equipment_index(request->weapon_index),
        .maximum_criticals =
            GetWeaponCrits(request->mech, request->weapon_index),
    });
    mech_weapon_destroy(&(WeaponDestructionRequest){
        .mech = request->mech,
        .first = {.section = request->weapon.section, .critical = firstCrit},
        .part_type = weapon_equipment_index(request->weapon_index),
        .criticals_to_destroy =
            GetWeaponCrits(request->mech, request->weapon_index),
        .total_criticals =
            GetWeaponCrits(request->mech, request->weapon_index)});
    mech_damage_apply(&(MechDamageRequest){
        .target = request->mech,
        .attacker = request->mech,
        .line_of_sight = 0,
        .attack_pilot = -1,
        .hit_location = request->weapon.section,
        .rear = 0,
        .critical = 0,
        .armor_damage = 0,
        .internal_damage = weapon_catalogue_damage(request->weapon_index),
        .transfer = MECH_DAMAGE_NORMAL,
        .cause = -1,
        .base_to_hit = 0,
        .weapon_index = -1,
        .ammunition_mode = 0,
        .ignore_swarmers = 1});
    return;
  }
  if (mech_weapon_critical_can_jam(
          &(WeaponCriticalRoll){.mech = request->mech,
                                .slot = {.section = request->weapon.section,
                                         .critical = request->weapon.critical},
                                .roll = roll})) {
    mech_printf(
        request->mech, MECHALL,
        "[fg=red bold]The ammo loader mechanism jams on your %s![reset]",
        weapon_display_name(request->weapon_index));
    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = request->mech,
        .slot = {.section = request->weapon.section,
                 .critical = request->weapon.critical},
        .failure = FAIL_AMMOCRITJAMMED});
    return;
  }
  if (mech_cocoon_integrity(request->mech)) {
    if (mech_position_z(request->mech) >
        mech_position_surface_elevation(request->mech)) {
      if (mech_jump_speed(request->mech) >= MP1) {
        mech_notify(
            request->mech, MECHALL,
            "You initiate your jumpjets to compensate for the opened cocoon!");
        mech_cocoon_integrity_set(request->mech, -1);
      } else {
        mech_notify(request->mech, MECHALL,
                    "Your action splits open the cocoon - have a nice fall!");
        mech_los_broadcast(request->mech,
                           "starts plummeting down, as the cocoon opens!.");
        mech_cocoon_integrity_set(request->mech, 0);
        mech_event_cancel(request->mech, EVENT_OOD);
        mech_event_schedule(request->mech, EVENT_FALL, mech_fall_event,
                            FALL_TICK, -1);
      }
    }
  }
  RbaseToHit = baseToHit;
  if (btech_context_glancing_blow_mode(mech_context(request->mech)) == 2)
    RbaseToHit = baseToHit - 1; /* only time we modify it */
  if (!is_artillery) {
    MechFireBroadcast(request->mech, request->target_kind ? nullptr : target,
                      request->target_hex.x, request->target_hex.y,
                      request->map, weapon_display_name(request->weapon_index),
                      (roll >= RbaseToHit) && range_ok);
  }
  if (target) {
    if (mech_los_check(target, request->mech, mech_position_x(request->mech),
                       mech_position_y(request->mech), range))
      mech_printf(target, MECHALL, "%s has fired a %s at you!",
                  mech_to_mech_display_id(target, request->mech).text,
                  weapon_display_name(request->weapon_index));
    else
      mech_printf(target, MECHALL,
                  "Something has fired a %s at you from bearing %d!",
                  weapon_display_name(request->weapon_index),
                  map_bearing(&(MapRealSegment){
                      .start = {.x = mech_position_real_x(target),
                                .y = mech_position_real_y(target)},
                      .end = {.x = mech_position_real_x(request->mech),
                              .y = mech_position_real_y(request->mech)}}));
  }
  mech_fired_recently_set(request->mech, true);
  if (!request->target_kind) /* only record against actual targets */
    mech_shots_fired_increment(request->mech);
  if (is_artillery) {
    ArtilleryShotRequest artillery_request = {
        .mech = request->mech,
        .target = {.x = request->target_hex.x, .y = request->target_hex.y},
        .weapon_index = request->weapon_index,
        .weapon_mode = mech_critical_ammo_mode(
            request->mech, request->weapon.section, request->weapon.critical),
        .hit = baseToHit <= roll,
    };
    artillery_shoot(&artillery_request);
  } else if (range_ok) {
    if (weapon_catalogue_is_missile(request->weapon_index)) {
      mech_hit_resolve(&(HitResolutionRequest){
          .attacker = request->mech,
          .weapon_index = request->weapon_index,
          .weapon = {.section = request->weapon.section,
                     .critical = request->weapon.critical},
          .target = target,
          .target_hex = {.x = request->target_hex.x,
                         .y = request->target_hex.y},
          .line_of_sight = request->line_of_sight,
          .failure_type = type,
          .failure_modifier = modifier,
          .hit = (roll >= RbaseToHit) && range_ok,
          .base_to_hit = baseToHit,
          .gatling_shots = wGattlingShots,
          .swarm_attack = swarm_attack,
          .player_roll = roll});
    } else {
      if (roll >= RbaseToHit) {
        mech_hit_resolve(&(HitResolutionRequest){
            .attacker = request->mech,
            .weapon_index = request->weapon_index,
            .weapon = {.section = request->weapon.section,
                       .critical = request->weapon.critical},
            .target = target,
            .target_hex = {.x = request->target_hex.x,
                           .y = request->target_hex.y},
            .line_of_sight = request->line_of_sight,
            .failure_type = type,
            .failure_modifier = modifier,
            .hit = true,
            .base_to_hit = RbaseToHit,
            .gatling_shots = wGattlingShots,
            .swarm_attack = swarm_attack,
            .player_roll = roll});
      } else {
        int tTryClear = 1;
        if (target) {
          if ((mech_class(target) == CLASS_BSUIT) &&
              (mech_swarm_target(target) > -1)) {
            altTarget = btech_context_get_mech(mech_context(request->mech),
                                               mech_swarm_target(target));
            if (altTarget) {
              MechNormalToHitResult alternate_to_hit =
                  mech_normal_to_hit_calculate(&(MechNormalToHitRequest){
                      .attacker = request->mech,
                      .map = request->map,
                      .section = request->weapon.section,
                      .critical = request->weapon.critical,
                      .weapon_index = request->weapon_index,
                      .range = range,
                      .target = altTarget,
                      .indirect_fire = request->indirect_fire});
              baseToHit = alternate_to_hit.value;
              if (roll >= baseToHit) {
                mech_notify(altTarget, MECHALL, "The shot hits you instead!");
                mech_los_broadcast(
                    altTarget,
                    "manages to get in the way of the shot instead!");
                mech_hit_resolve(&(HitResolutionRequest){
                    .attacker = request->mech,
                    .weapon_index = request->weapon_index,
                    .weapon = {.section = request->weapon.section,
                               .critical = request->weapon.critical},
                    .target = altTarget,
                    .target_hex = {.x = request->target_hex.x,
                                   .y = request->target_hex.y},
                    .line_of_sight = request->line_of_sight,
                    .failure_type = type,
                    .failure_modifier = modifier,
                    .hit = true,
                    .base_to_hit = baseToHit,
                    .gatling_shots = wGattlingShots,
                    .swarm_attack = swarm_attack,
                    .player_roll = roll});
                tTryClear = 0;
              } else {
                if (battle_map_hex_elevation(request->map,
                                             mech_position_x(target),
                                             mech_position_y(target)) <
                    (mech_position_z(target) - 2))
                  tTryClear = 0;
              }
            }
          }
        }
        if (tTryClear) {
          int tempDamage = mech_hit_damage_determine(&(HitDamageRequest){
              .attacker = request->mech,
              .weapon = {.section = request->weapon.section,
                         .critical = request->weapon.critical},
              .target = target,
              .target_hex = {.x = request->target_hex.x,
                             .y = request->target_hex.y},
              .weapon_index = request->weapon_index,
              .gatling_shots = wGattlingShots,
              .base_damage = weapon_catalogue_damage(request->weapon_index),
              .ammunition_mode = mech_critical_ammo_mode(
                  request->mech, request->weapon.section,
                  request->weapon.critical),
              .failure_type = type,
              .failure_modifier = modifier,
              .temporary_calculation = true});
          mech_terrain_possibly_ignite_or_clear(&(TerrainWeaponEffectRequest){
              .mech = request->mech,
              .position = {.x = request->target_hex.x,
                           .y = request->target_hex.y},
              .weapon_index = request->weapon_index,
              .ammunition_mode = mech_critical_ammo_mode(
                  request->mech, request->weapon.section,
                  request->weapon.critical),
              .damage = tempDamage});
        }
      }
    }
  }
  mech_set_recycle_part(
      request->mech, request->weapon.section, request->weapon.critical,
      WEAPON_TICK * btech_context_weapon_recycle_time(
                        mech_context(request->mech), request->weapon_index));
  if (type == HEAT)
    mech_weapon_heat_add(request->mech, (float)modifier);
  const int catalogue_heat = weapon_catalogue_heat(request->weapon_index);
  if (mech_critical_fire_mode(request->mech, request->weapon.section,
                              request->weapon.critical) &
      GATTLING_MODE) {
    mech_weapon_heat_add(request->mech, (float)wGattlingShots);
  } else if (weapon_catalogue_is_rotary_autocannon(request->weapon_index)) {
    if (mech_critical_fire_mode(request->mech, request->weapon.section,
                                request->weapon.critical) &
        RAC_TWOSHOT_MODE)
      wRACHeat = 2;
    else if (mech_critical_fire_mode(request->mech, request->weapon.section,
                                     request->weapon.critical) &
             RAC_FOURSHOT_MODE)
      wRACHeat = 4;
    else if (mech_critical_fire_mode(request->mech, request->weapon.section,
                                     request->weapon.critical) &
             RAC_SIXSHOT_MODE)
      wRACHeat = 6;
    else
      wRACHeat = 1;
    mech_weapon_heat_add(request->mech, (float)(catalogue_heat * wRACHeat));
    if (type == HEAT)
      mech_weapon_heat_add(request->mech, (float)(modifier * wRACHeat));
  } else {
    mech_weapon_heat_add(request->mech, (float)catalogue_heat);
    if (weapon_catalogue_is_energy(request->weapon_index)) {
      const int critical_heat_modifier = mech_weapon_critical_heat_modifier(
          request->mech, request->weapon.section, request->weapon.critical);
      mech_weapon_heat_add(request->mech, (float)critical_heat_modifier);
    }
    if ((mech_critical_fire_mode(request->mech, request->weapon.section,
                                 request->weapon.critical) &
         ULTRA_MODE) ||
        (mech_critical_fire_mode(request->mech, request->weapon.section,
                                 request->weapon.critical) &
         RFAC_MODE)) {
      if (type == HEAT)
        mech_weapon_heat_add(request->mech, (float)modifier);
      mech_weapon_heat_add(request->mech, (float)catalogue_heat);
    }
  }
  mech_ammunition_decrement(&(AmmunitionDecrementRequest){
      .mech = request->mech,
      .weapon_index = request->weapon_index,
      .weapon = {.section = request->weapon.section,
                 .critical = request->weapon.critical},
      .primary_ammunition = ammunition.primary,
      .secondary_ammunition = ammunition.secondary,
      .gatling_shots = wGattlingShots,
  });
  if (weapon_catalogue_is_heavy_gauss(request->weapon_index) &&
      (mech_class(request->mech) == CLASS_MECH)) {
    if (fabsf(mech_current_speed(request->mech)) > 0.0F) {
      mech_notify(request->mech, MECHALL,
                  "You realize that moving while firing this weapon may not be "
                  "a good idea after all.");
      if (mech_tonnage(request->mech) <= 35)
        wHGRPSkillMod = 2;
      else if (mech_tonnage(request->mech) <= 55)
        wHGRPSkillMod = 1;
      else if (mech_tonnage(request->mech) <= 75)
        wHGRPSkillMod = 0;
      else
        wHGRPSkillMod = -1;
      if (!MadePilotSkillRoll(request->mech, wHGRPSkillMod)) {
        mech_notify(request->mech, MECHALL,
                    "The weapon's recoil knocks you to the ground!");
        mech_los_broadcast(request->mech,
                           tprintf("topples over from the %s's recoil!",
                                   weapon_display_name(request->weapon_index)));
        mech_fall(request->mech, 1, 0);
      }
    }
  }
}
