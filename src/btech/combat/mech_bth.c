/* Calculates unit combat base-to-hit values. */
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_bth_api.h"
#include "mech_c3_misc_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_network_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

#include <math.h>
#include <stdio.h>
#include <string.h>

typedef struct BthTrace {
  int total;
#ifdef BTH_DEBUG
  char debug[LBUF_SIZE];
  char summary[LBUF_SIZE];
#endif
} BthTrace;

static void bth_trace_begin(BthTrace *trace, Mech *attacker, Mech *target,
                            int initial) {
  trace->total = initial;
#ifdef BTH_DEBUG
  snprintf(trace->summary, sizeof(trace->summary), "Base %d", initial);
  if (target)
    snprintf(trace->debug, sizeof(trace->debug), "#%ld -> #%ld: Base %d",
             mech_dbref(attacker), mech_dbref(target), initial);
  else
    snprintf(trace->debug, sizeof(trace->debug), "#%ld -> (hex): Base %d",
             mech_dbref(attacker), initial);
#else
  (void)attacker;
  (void)target;
#endif
}

static void bth_trace_add(BthTrace *trace, const char *description,
                          int modifier) {
  if (!modifier)
    return;
#ifdef BTH_DEBUG
  char fragment[LBUF_SIZE];
  snprintf(fragment, sizeof(fragment), ", %s: %s%d", description,
           modifier > 0 ? "+" : "", modifier);
  strncat(trace->debug, fragment,
          sizeof(trace->debug) - strlen(trace->debug) - 1);
  strncat(trace->summary, fragment,
          sizeof(trace->summary) - strlen(trace->summary) - 1);
#else
  (void)description;
#endif
  trace->total += modifier;
}

static void bth_trace_finish(Mech *mech, const BthTrace *trace) {
#ifdef BTH_DEBUG
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_BTH_DEBUG, "%s",
                     tprintf("%s.", trace->debug));
#else
  (void)mech;
  (void)trace;
#endif
}
int mech_normal_to_hit_calculate(Mech *mech, BattleMap *mech_map, int section,
                                 int critical, int weapindx, float range,
                                 Mech *target, int indirectFire, DbRef *c3Ref) {
  Mech *spotter = nullptr;
  BthTrace trace;
  int wFireMode = mech_critical_fire_mode(mech, section, critical);
  int wAmmoMode = mech_critical_ammo_mode(mech, section, critical);
  int tInWater = 0;
  int wTargMoveMod = 0;
  int rangecheck = 0;
  int j, rbth = 0;
  float enemyX, enemyY, enemyZ;
  WeaponRangeBracket wRangeBracket = RANGE_TOFAR;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);
  *c3Ref = -1;
  if (target) {
    tInWater =
        ((mech_real_terrain_get(mech) == WATER) && (mech_position_z(mech) < 0));
  }
  bth_trace_begin(&trace, mech, target, FindPilotGunnery(mech, weapindx));
  if (indirectFire < 1000) {
    spotter = btech_context_get_mech(context, mech_spotter_dbref(mech));
    if (!spotter) {
      mech_notify(mech, MECHALL, "Error finding your spotter! (notify a wiz)");
      return 0;
    }

    bth_trace_add(&trace, "Spotting", FindPilotSpotting(spotter) - 4);
  }

  /* Our special bother for aeros */
  if (mech_is_aerospace_unit(mech) && target &&
      !mech_is_aerospace_unit(target) && !mech_is_landed(mech)) {
    bth_trace_add(&trace, "Aero strafing", 2);
  };

  /* MW need +2 added per FASA */
  if (target && mech_class(target) == CLASS_MW)
    bth_trace_add(&trace, "MechWarrior", 2);

  /* add in to-hit mods from criticals */
  bth_trace_add(&trace, "MechBTHMod", mech_base_to_hit_modifier(mech));

  /* add in to-hit mods for section damage */
  bth_trace_add(&trace, "MechLocBTHMod",
                mech_section_base_to_hit(mech, section));

  /* Add +1 if we're firing from water */
  if (tInWater)
    bth_trace_add(&trace, "InWater", 1);

  /* Add in the rangebase.. */
  const WeaponRangeProfile weapon_ranges = weapon_catalogue_ranges(weapindx);
  rangecheck = weapon_catalogue_effective_range(
      weapindx, btech_context_uses_extended_weapon_ranges(context));
  if (wAmmoMode & STINGER_MODE)
    rangecheck += 7;
  if ((float)rangecheck < range) {
    bth_trace_add(&trace, "OutOfRange", 1000);
  } else {
    if (((float)weapon_ranges.minimum >= range) &&
        (weapon_ranges.minimum > 0)) {
      if (!weapon_catalogue_is_hot_loaded(weapindx, wFireMode)) {
        /* if the target is in minimum range then the BTH is as good as it will
         * get */
        rbth = (int)((float)weapon_ranges.minimum - range + 1.0F);
      } else {
        if (btech_context_hotload_uses_half_modifier(context)) {
          rbth = (int)(((float)weapon_ranges.minimum - range + 2.0F) / 2.0F);
        }
      }

      bth_trace_add(&trace, "MinRange", rbth);
    } else if ((mech_technology_flags(mech) &
                (C3_MASTER_TECH | C3_SLAVE_TECH)) &&
               !condition.c3_destroyed && !mech_is_any_ecm_disturbed(mech) &&
               mech_c3_network_size(mech) > 0) {
      wRangeBracket = mech_c3_range_to_hit_calculate(
          mech, target, section, weapindx, range,
          mech_network_range(mech, target, range, c3Ref, 1), wAmmoMode, &rbth);

      bth_trace_add(&trace, "C3Range", rbth);
    } else if ((mech_technology_flags_secondary(mech) & C3I_TECH) &&
               !condition.c3i_destroyed && !mech_is_any_ecm_disturbed(mech) &&
               mech_c3i_network_size(mech) > 0) {
      wRangeBracket = mech_c3_range_to_hit_calculate(
          mech, target, section, weapindx, range,
          mech_network_range(mech, target, range, c3Ref, 0), wAmmoMode, &rbth);

      bth_trace_add(&trace, "C3iRange", rbth);
    } else {
      wRangeBracket = mech_range_to_hit_calculate(
          mech, target, section, weapindx, range, wFireMode, wAmmoMode, &rbth);
      bth_trace_add(&trace, "Range", rbth);
    }
  }
  /* I decided to put it here in this rewritten form. To add it to FindBTH*()
   * was a bit convoluted compared to original source method, Exile and
   * 3030/btechmux went a few different path's on internal representation and
   * organization of various weapon data and BTH handling. Someday I might,
   * after I review how some of the same things were done here, port some of
   * that stuff over. (Like GunStat() and such)
   *
   * For now we just do the below stuff. Besides, it might make BTH debug more
   * obvious that putting it into FindBTH*().
   */
  if (mech_targeting_computer_type(mech) == TARGCOMP_SHORT ||
      mech_targeting_computer_type(mech) == TARGCOMP_LONG) {
    int tmp_range;

    if (weapon_catalogue_is_personal_combat(weapindx))
      tmp_range = (int)(range * 10.0F + 0.95F);
    else
      tmp_range = (int)(range + 0.95F);

    if (tmp_range > (mech_section_is_underwater(mech, section)
                         ? weapon_ranges.water_medium_range
                         : weapon_ranges.medium_range))
      bth_trace_add(&trace, "TargComp/Long",
                    mech_targeting_computer_type(mech) == TARGCOMP_LONG ? -1
                                                                        : 1);
    else if (tmp_range <= (mech_section_is_underwater(mech, section)
                               ? weapon_ranges.water_medium_range
                               : weapon_ranges.medium_range))
      bth_trace_add(&trace, "TargComp/Short",
                    mech_targeting_computer_type(mech) == TARGCOMP_SHORT ? -1
                                                                         : 1);
  }

  if (target && mech_infantry_technology_flags(target) & STEALTH_TECH) {
    int infantry_technology = mech_infantry_technology_flags(target);
    if (infantry_technology & FWL_ACHILEUS_STEALTH_TECH) {
      if (wRangeBracket == RANGE_SHORT)
        bth_trace_add(&trace, "FWLStealthBonus", 1);
      else if (wRangeBracket == RANGE_MED)
        bth_trace_add(&trace, "FWLStealthBonus", 2);
      else if (wRangeBracket == RANGE_LONG)
        bth_trace_add(&trace, "FWLStealthBonus", 3);
    } else if (infantry_technology & DC_KAGE_STEALTH_TECH) {
      if (wRangeBracket == RANGE_MED)
        bth_trace_add(&trace, "DCStealthBonus", 1);
      else if (wRangeBracket == RANGE_LONG)
        bth_trace_add(&trace, "DCStealthBonus", 2);
    } else if (infantry_technology & FC_INFILTRATOR_STEALTH_TECH) {
      if (wRangeBracket == RANGE_MED)
        bth_trace_add(&trace, "FCStealthBonus", 1);
      else if (wRangeBracket == RANGE_LONG)
        bth_trace_add(&trace, "FCStealthBonus", 2);
    } else if (infantry_technology & FC_INFILTRATORII_STEALTH_TECH) {
      if (wRangeBracket == RANGE_SHORT)
        bth_trace_add(&trace, "FCStealthIIBonus", 1);
      if (wRangeBracket == RANGE_MED)
        bth_trace_add(&trace, "FCStealthIIBonus", 1);
      else if (wRangeBracket == RANGE_LONG)
        bth_trace_add(&trace, "FCStealthIIBonus", 2);
    }
  }

  /* Add in the movement modifiers */
  if (mech_section_configuration_has(mech, section, STABILIZERS_DESTROYED))
    bth_trace_add(&trace, "AttackMoveX2",
                  mech_attacker_movement_modifier(mech) * 2);
  else
    bth_trace_add(&trace, "AttackMove", mech_attacker_movement_modifier(mech));

  /* Add mods for overheating */
  bth_trace_add(&trace, "Overheat", mech_overheat_to_hit_modifier(mech));

  /* Add special weapon mods */
  if (wAmmoMode & AC_AP_MODE)
    bth_trace_add(&trace, "ArmorPiercing", 1);

  if (mech_has_section_special(mech, INARC_HAYWIRE_ATTACHED))
    bth_trace_add(&trace, "HaywirePod", 1);

  if (target && (wAmmoMode & NARC_MODE) &&
      !weapon_catalogue_is_narc(weapindx) &&
      mech_has_section_special(target, INARC_HOMING_ATTACHED))
    bth_trace_add(&trace, "iNARC", -1);

  if (weapon_catalogue_is_pulse(weapindx))
    bth_trace_add(&trace, "Pulse", -2);

  if (weapon_catalogue_is_mrm(weapindx))
    bth_trace_add(&trace, "MRM", 1);

  if (weapon_catalogue_is_heavy(weapindx))
    bth_trace_add(&trace, "HeavyWeapon", 1);
  if (target && (wAmmoMode & STINGER_MODE)) {
    if (mech_is_flying_type(target) && !mech_is_landed(target))
      bth_trace_add(&trace, "Stinger (Flying)", -3);
    else if (mech_is_out_of_control(target))
      bth_trace_add(&trace, "Stinger (OOD)", -1);
    else if (mech_is_jumping(target))
      bth_trace_add(&trace, "Stinger (Jumping)", 0);
  }

  if (weapon_catalogue_is_rocket(weapindx))
    bth_trace_add(&trace, "Rocket Launcher", 1);

  if (target && (mech_class(target) == CLASS_VTOL) &&
      (fabsf(mech_current_speed(target)) > 0.0F ||
       fabsf(mech_vertical_speed(target)) > 0.0F))
    bth_trace_add(&trace, "TargetVTOL", 1);

  if (target && mech_targeting_computer_type(mech) == TARGCOMP_AA) {
    if (!mech_is_landed(target) &&
        (mech_is_flying_type(target) || mech_is_jumping(target) ||
         mech_is_out_of_control(target)))
      bth_trace_add(&trace, "TargComp/AA-Fly",
                    mech_technology_flags(mech) & AA_TECH ? -3 : -2);
    else
      bth_trace_add(&trace, "TargComp/AA-Ground", 1);
  }

  /* -1 for LBX, unless it's a VTOL... then -3 */
  if (wAmmoMode & LBX_MODE)
    bth_trace_add(&trace, "LBX",
                  (target && (mech_class(target) == CLASS_VTOL) ? -3 : -1));

  /* Unstable lock */
  if (!btech_context_overrides_weapon_arcs(context) &&
      (!spotter && target &&
       ((mech_target_dbref(mech) != mech_dbref(target)) ||
        (mech_event_count(mech, EVENT_LOCK) &&
         mech_targeting_computer_type(mech) != TARGCOMP_MULTI)))) {
    if (FindTargetXY(mech, &enemyX, &enemyY, &enemyZ)) {
      if (InWeaponArc(mech, enemyX, enemyY) & (FORWARDARC | TURRETARC))
        bth_trace_add(&trace, "UnstableLock/Fwarc", 1);
      else
        bth_trace_add(&trace, "UnstableLock", 2);
    } else {
      bth_trace_add(&trace, "HipShot-NoLock", 2);
    }
  }

  if (mech_targeting_computer_type(mech) == TARGCOMP_MULTI) {
    if (FindTargetXY(mech, &enemyX, &enemyY, &enemyZ)) {
      if (!(InWeaponArc(mech, enemyX, enemyY) & FORWARDARC))
        bth_trace_add(&trace, "TargComp/MultiSideArc", 1);
    }
  }

  /* -4 for firing at a hex */
  if (!target && mech_targets_hex_or_building(mech))
    bth_trace_add(&trace, "HexBonus", -4);

  /* -2 for firing at someone dropping out of the sky */
  if (target && mech_cocoon_integrity(target) > 0)
    bth_trace_add(&trace, "OODbonus", -2);

  /* Indirect fire terrain modifiers */
  if (indirectFire < 1000)
    bth_trace_add(&trace, "IDFTerrain", indirectFire);

  /* +1 if spotting */
  if (mech_spotter_dbref(mech) == mech_dbref(mech))
    bth_trace_add(&trace, "Spotting", 1);

  /* if our target is another unit... */
  if (target) {
    /* Add the dig-in bonus */
    if (mech_condition_summary(target).dug_in &&
        (!btech_context_dig_bonus_requires_front(context) ||
         (mech_hit_group(mech, target) == FRONT)) &&
        (mech_position_z(target) >= mech_position_z(mech)))
      bth_trace_add(&trace, "DugIn", btech_context_dig_bonus(context));

    /* -3 if it's a DS... most people can hit the broadside of a barn */
    if (mech_is_dropship(target))
      bth_trace_add(&trace, "DSBonus", -3);

    /* Add +1 for BSuit dispersion */
    if (mech_class(target) == CLASS_BSUIT)
      bth_trace_add(&trace, "Bsuitbonus", 1);

    /* Let's see if we're targetting the head */
    if (target && !weapon_catalogue_is_missile(weapindx) &&
        mech_aim_section(mech) == HEAD &&
        (mech_class(target) == CLASS_MECH || mech_class(target) == CLASS_MW)) {
      if (mech_is_immobile(target))
        bth_trace_add(&trace, "HeadTarget", 7);
      else
        bth_trace_add(&trace, "HeadTarget-Fake", 25);
    } else {
      if ((wFireMode & ON_TC) && !condition.targeting_computer_destroyed &&
          !(wAmmoMode & LBX_MODE)) {
        if (mech_aim_section(mech) != NUM_SECTIONS && !mech_is_immobile(target))
          bth_trace_add(&trace, "TC-Target-NotImmobile", 3);
        else
          bth_trace_add(&trace, "TC", -1);
      }
    }

    /* Add aero targetting mods. TODO: Rewrite aero code :) */
    if (mech_class(mech) == CLASS_AERO) {
      wTargMoveMod = mech_target_movement_modifier(mech, target, range) * 3 / 4;
    } else {
      wTargMoveMod = mech_target_movement_modifier(mech, target, range);
    }

    if (wAmmoMode & AC_PRECISION_MODE)
      wTargMoveMod = MAX(wTargMoveMod -= 2, 0);

    /* We ignore Movemod if the weapon is in sguided AND its tagged by a
     * friendly TAG AND movemod > 0 */
    if ((wAmmoMode & SGUIDED_MODE) && (mech_tagged_by_dbref(target) > 0)) {
      Mech *tagger =
          btech_context_get_mech(context, mech_tagged_by_dbref(target));
      if (tagger != nullptr) {
        if ((mech_team(tagger) == mech_team(mech)) &&
            (mech_tagged_by_dbref(target) != mech_dbref(mech))) {
          if (wTargMoveMod < 0) {
            bth_trace_add(&trace, "TargetMove (SG Tag -MoveMod)", wTargMoveMod);
          }
        } else {
          bth_trace_add(&trace, "TargetMove (SG No Tag)", wTargMoveMod);
        }
      }
    } else {
      bth_trace_add(&trace, "TargetMove", wTargMoveMod);
    }

    /* Add in the terrain modifier */
    if (indirectFire >= 1000) {
      j = mech_los_terrain_modifier(mech, target, mech_map, range, wAmmoMode);
      if (j < 1000)
        bth_trace_add(&trace, "Terrain/is_light(Sensor)", j);
    }

    if (btech_context_woods_modify_damage(context) &&
        (map_real_terrain_get(mech_map, mech_position_x(target),
                              mech_position_y(target)) == LIGHT_FOREST ||
         map_real_terrain_get(mech_map, mech_position_x(target),
                              mech_position_y(target)) == HEAVY_FOREST) &&
        ((mech_position_z(target) - 2) <=
         map_elevation_get(mech_map, mech_position_x(target),
                           mech_position_y(target)))) {
      if (map_real_terrain_get(mech_map, mech_position_x(target),
                               mech_position_y(target)) == LIGHT_FOREST)
        bth_trace_add(&trace, "Light Woods bonus", -1);
      else if (map_real_terrain_get(mech_map, mech_position_x(target),
                                    mech_position_y(target)) == HEAVY_FOREST)
        bth_trace_add(&trace, "Heavy Woods bonus", -2);
    }
#ifdef BT_MOVEMENT_MODES
    MechConditionSummary target_condition = mech_condition_summary(target);
    if (target_condition.sprinting || target_condition.evading) {
      if (target_condition.sprinting)
        bth_trace_add(&trace, "SprintingTarget",
                      btech_context_sprint_to_hit_modifier(context));
      if (!target_condition.fallen && target_condition.evading)
        bth_trace_add(&trace, "EvadingTarget", 1);
    }
#endif
  }

  /* Check for damage */
  bth_trace_add(&trace, "CritDamage",
                mech_weapon_critical_to_hit_modifier(mech, section, critical,
                                                     wRangeBracket));
#ifdef BTH_DEBUG
  if (condition.to_hit_debug)
    notify_printf(btech_context_evaluation(context), mech_pilot_dbref(mech),
                  "BTHDebug: %s", trace.summary);
#endif

  bth_trace_finish(mech, &trace);
  return trace.total;
}

int mech_artillery_to_hit_calculate(Mech *mech, int section, int weapindx,
                                    int indirect, float range) {
  int baseToHit = 11;
  Mech *spotter;

  BtechContext *context = mech_context(mech);

  if (mech_section_is_underwater(mech, section))
    return 5000;

  const int effective_range = weapon_catalogue_effective_range(
      weapindx, btech_context_uses_extended_weapon_ranges(context));
  if ((float)effective_range < range)
    return 1000;

  baseToHit += (FindPilotArtyGun(mech) - 4);
  if (indirect) {
    spotter = btech_context_get_mech(context, mech_spotter_dbref(mech));
    if (spotter && spotter != mech)
      baseToHit += (FindPilotSpotting(spotter) - 4) / 2;
    /* the usual +2, added by +1 make +3 */
    if (indirect && (mech_spotter_dbref(mech) == NOTHING ||
                     mech_spotter_dbref(mech) == mech_dbref(mech)))
      baseToHit += 1;
  } else
    baseToHit -= 2;
  return baseToHit - mech_fire_adjustment(mech);
}

WeaponRangeBracket mech_range_to_hit_calculate(Mech *mech, Mech *target,
                                               int section, int weapindx,
                                               float frange, int firemode,
                                               int ammomode, int *wBTH) {
  int range;
  int wTargetStealth = 0;
  BtechContext *context = mech_context(mech);
  const WeaponRangeProfile ranges = weapon_catalogue_ranges(weapindx);

  if (target)
    wTargetStealth = mech_condition_summary(target).stealth_armor_active ||
                     mech_condition_summary(target).null_signature_active;

  if (weapon_catalogue_is_personal_combat(weapindx))
    range = (int)(frange * 10.0F + 0.95F);
  else
    range = (int)(frange + 0.95F);

  if (mech_section_is_underwater(mech, section)) {
    if (ranges.water_short_range <= 0) {
      *wBTH = 5000;
      return RANGE_NOWATER;
    }

    /* Out of range range */
    if (range >
        weapon_catalogue_effective_water_range(
            weapindx, btech_context_uses_extended_weapon_ranges(context))) {
      *wBTH = 1000;
      return RANGE_TOFAR;
    }

    /* Very long range */
    if (range > weapon_catalogue_effective_water_range(weapindx, false)) {
      *wBTH = wTargetStealth ? 12 : 8;
      return RANGE_EXTREME;
    }

    /* Long range... */
    if (range > ranges.water_medium_range) {
      *wBTH = wTargetStealth ? 6 : 4;
      return RANGE_LONG;
    }

    /* Medium range */
    if (range > ranges.water_short_range) {
      *wBTH = wTargetStealth ? 3 : 2;
      return RANGE_MED;
    }

    /* Short range */
    if (range > ranges.water_minimum) {
      *wBTH = 0;
      return RANGE_SHORT;
    }

    if (range == 0) {
      if (ranges.water_minimum == 0) {
        *wBTH = 0;
        return RANGE_SHORT;
      } else {
        *wBTH = ranges.water_minimum - range;
        return RANGE_SHORT;
      }
    }

    /* Less than or equal to minimum range */
    *wBTH = ranges.water_minimum - range + 1;
  }

  /* Beyond range */
  if (range > ((ammomode & STINGER_MODE)
                   ? (weapon_catalogue_effective_range(
                          weapindx,
                          btech_context_uses_extended_weapon_ranges(context)) +
                      7)
                   : weapon_catalogue_effective_range(
                         weapindx,
                         btech_context_uses_extended_weapon_ranges(context)))) {
    *wBTH = 1000;
    return RANGE_TOFAR;
  }

  /* V. Long range */
  if (range > weapon_catalogue_effective_range(weapindx, false)) {
    *wBTH = wTargetStealth ? 12 : 8;
    return RANGE_EXTREME;
  }

  /* Long range... */
  if (range > ranges.medium_range) {
    *wBTH = wTargetStealth ? 6 : 4;
    return RANGE_LONG;
  }

  /* Medium range */
  if (range > ranges.short_range) {
    *wBTH = wTargetStealth ? 3 : 2;
    return RANGE_MED;
  }

  /* Short range */
  if (range > ranges.minimum) {
    *wBTH = 0;
    return RANGE_SHORT;
  }
  /* If we are at range 0.0

   * Added 8/3/99 by Kipsta (to fix a 0.0 bug)
   */

  if (range == 0) {
    if (ranges.minimum == 0) {
      *wBTH = 0;
      return RANGE_SHORT;
    } else {
      if (!weapon_catalogue_is_hot_loaded(weapindx, firemode)) {
        *wBTH = ranges.minimum - range;
      } else {
        if (btech_context_hotload_uses_half_modifier(context))
          *wBTH = ((ranges.minimum - range + 1) / 2);
        else
          *wBTH = 0;
      }

      return RANGE_SHORT;
    }
  }

  if (weapon_catalogue_is_hot_loaded(weapindx, firemode)) {
    if (btech_context_hotload_uses_half_modifier(context))
      *wBTH = ((ranges.minimum - range + 1) / 2);
    else
      *wBTH = 0;

    return RANGE_SHORT;
  }

  /* Less than or equal to minimum range */
  *wBTH = ranges.minimum - range + 1;
  return RANGE_SHORT;
}

WeaponRangeBracket mech_c3_range_to_hit_calculate(Mech *mech, Mech *target,
                                                  int section, int weapindx,
                                                  float realRange,
                                                  float c3Range, int mode,
                                                  int *wBTH) {
  int realRangeAdj = 0;
  int c3RangeAdj = 0;
  int wTargetStealth = 0;
  BtechContext *context = mech_context(mech);
  const WeaponRangeProfile ranges = weapon_catalogue_ranges(weapindx);

  if (target)
    wTargetStealth = mech_condition_summary(target).stealth_armor_active ||
                     mech_condition_summary(target).null_signature_active;

  if (weapon_catalogue_is_personal_combat(weapindx)) {
    realRangeAdj = (int)(realRange * 10.0F + 0.95F);
    c3RangeAdj = (int)(c3Range * 10.0F + 0.95F);
  } else {
    realRangeAdj = (int)(realRange + 0.95F);
    c3RangeAdj = (int)(c3Range + 0.95F);
  }

  if (mech_section_is_underwater(mech, section)) {
    if (ranges.water_short_range <= 0) {
      *wBTH = 5000;
      return RANGE_NOWATER;
    }

    /* Out of range. No ERange in C3 */
    if (realRangeAdj >
        weapon_catalogue_effective_water_range(weapindx, false)) {
      *wBTH = 1000;
      return RANGE_TOFAR;
    }

    /* Long range... */
    if (c3RangeAdj > ranges.water_medium_range) {
      *wBTH = wTargetStealth ? 6 : 4;
      return RANGE_LONG;
    }

    /* Medium range */
    if (c3RangeAdj > ranges.water_short_range) {
      *wBTH = wTargetStealth ? 3 : 2;
      return RANGE_MED;
    }

    /* Short range */
    *wBTH = 0;
    return RANGE_SHORT;
  }

  /* Beyond range */
  if (realRangeAdj > weapon_catalogue_effective_range(weapindx, false)) {
    *wBTH = 1000;
    return RANGE_TOFAR;
  }

  /* No V. Long range in a C3 network */
  /* Long range... */
  if (c3RangeAdj > ranges.medium_range) {
    *wBTH = wTargetStealth ? 6 : 4;
    return RANGE_LONG;
  }

  /* Medium range */
  if (c3RangeAdj > ranges.short_range) {
    *wBTH = wTargetStealth ? 3 : 2;
    return RANGE_MED;
  }

  /* Short range */
  if (realRange > (float)ranges.minimum) {
    *wBTH = 0;
    return RANGE_SHORT;
  }

  /* Check for range 0.0 */
  if (c3RangeAdj == 0) {
    if (ranges.minimum == 0) {
      *wBTH = 0;
      return RANGE_SHORT;
    }
  }

  /* We don't care about min range if we're Hotloading */
  if (!weapon_catalogue_is_hot_loaded(weapindx, mode)) {
    if (btech_context_hotload_uses_half_modifier(context))
      *wBTH = (int)(((float)ranges.minimum - realRange + 1.0F) / 2.0F);
    else
      *wBTH = 0;

    return RANGE_SHORT;
  }

  /* Less than or equal to minimum PHYSICAL range */
  *wBTH = (int)((float)ranges.minimum - realRange + 1.0F);
  return RANGE_SHORT;
}
