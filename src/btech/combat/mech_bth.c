
/*
 * $Id: mech.bth.c,v 1.2 2005/06/23 15:27:04 av1-op Exp $
 *
 * Author: Cord Awtry <kipsta@mediaone.net>
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "aero_move_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
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
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

#ifndef BTH_DEBUG
#define BTHBASE(m, t, n) baseToHit = n;
#define BTHADD(desc, n) baseToHit += n;
#define BTHEND(m)
#else
#define BTHBASE(m, t, n)                                                       \
  do {                                                                         \
    if (t)                                                                     \
      snprintf(buf, LBUF_SIZE, "#%ld -> #%ld: Base %d", mech_dbref(m),         \
               mech_dbref(t), n);                                              \
    else                                                                       \
      snprintf(buf, LBUF_SIZE, "#%ld -> (hex): Base %d", mech_dbref(m), n);    \
    baseToHit = n;                                                             \
    snprintf(bthbuf, sizeof(bthbuf), "Base %d", n);                            \
  } while (0)
#define BTHADD(desc, n)                                                        \
  do {                                                                         \
    i = n;                                                                     \
    if (i) {                                                                   \
      snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf), ", %s: %s%d",     \
               desc, i > 0 ? "+" : "", i);                                     \
      snprintf(bthbuf + strlen(bthbuf), sizeof(bthbuf) - strlen(bthbuf),       \
               ", %s: %s%d", desc, i > 0 ? "+" : "", i);                       \
      baseToHit += i;                                                          \
    }                                                                          \
  } while (0)
#define BTHEND(m)                                                              \
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_BTH_DEBUG, "%s",   \
                     tprintf("%s.", buf))
#endif

int mech_normal_to_hit_calculate(Mech *mech, BattleMap *mech_map, int section,
                                 int critical, int weapindx, float range,
                                 Mech *target, int indirectFire, DbRef *c3Ref) {
  Mech *spotter = nullptr;
  int baseToHit;
  int wFireMode = mech_critical_fire_mode(mech, section, critical);
  int wAmmoMode = mech_critical_ammo_mode(mech, section, critical);
  int tInWater = 0;
  int wTargMoveMod = 0;
  int rangecheck = 0;
#ifdef BTH_DEBUG
  char buf[LBUF_SIZE];
  int i;
#endif
  char bthbuf[LBUF_SIZE];
  int j, rbth = 0;
  float enemyX, enemyY, enemyZ;
  int wRangeBracket = RANGE_TOFAR;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);

  *c3Ref = -1;

  if (target) {
    tInWater =
        ((mech_real_terrain_get(mech) == WATER) && (mech_position_z(mech) < 0));
  }

  BTHBASE(mech, target, FindPilotGunnery(mech, weapindx));

  if (indirectFire < 1000) {
    spotter = btech_context_get_mech(context, mech_spotter_dbref(mech));

    if (!spotter) {
      mech_notify(mech, MECHALL, "Error finding your spotter! (notify a wiz)");
      return 0;
    }

    BTHADD("Spotting", FindPilotSpotting(spotter) - 4);
  }

  /* Our special bother for aeros */
  if (mech_is_aerospace_unit(mech) && target &&
      !mech_is_aerospace_unit(target) && !mech_is_landed(mech)) {
    BTHADD("Aero strafing", 2);
  };

  /* MW need +2 added per FASA */
  if (target && mech_class(target) == CLASS_MW)
    BTHADD("MechWarrior", 2);

  /* add in to-hit mods from criticals */
  BTHADD("MechBTHMod", mech_base_to_hit_modifier(mech));

  /* add in to-hit mods for section damage */
  BTHADD("MechLocBTHMod", mech_section_base_to_hit(mech, section));

  /* Add +1 if we're firing from water */
  if (tInWater)
    BTHADD("InWater", 1);

  /* Add in the rangebase.. */
  rangecheck = weapon_catalogue_effective_range(
      weapindx, btech_context_uses_extended_weapon_ranges(context));
  if (wAmmoMode & STINGER_MODE)
    rangecheck += 7;
  if (rangecheck < range) {
    BTHADD("OutOfRange", 1000);
  } else {
    if ((MechWeapons[weapindx].min >= range) &&
        (MechWeapons[weapindx].min > 0)) {
      if (!weapon_catalogue_is_hot_loaded(weapindx, wFireMode)) {
        /* if the target is in minimum range then the BTH is as good as it will
         * get */
        rbth = (MechWeapons[weapindx].min - range + 1);
      } else {
        if (btech_context_hotload_uses_half_modifier(context)) {
          rbth = ((MechWeapons[weapindx].min - range + 2) / 2);
        }
      }

      BTHADD("MinRange", rbth);
    } else if ((mech_technology_flags(mech) &
                (C3_MASTER_TECH | C3_SLAVE_TECH)) &&
               !condition.c3_destroyed && !mech_is_any_ecm_disturbed(mech) &&
               mech_c3_network_size(mech) > 0) {
      wRangeBracket = mech_c3_range_to_hit_calculate(
          mech, target, section, weapindx, range,
          mech_network_range(mech, target, range, c3Ref, 1), wAmmoMode, &rbth);

      BTHADD("C3Range", rbth);
    } else if ((mech_technology_flags_secondary(mech) & C3I_TECH) &&
               !condition.c3i_destroyed && !mech_is_any_ecm_disturbed(mech) &&
               mech_c3i_network_size(mech) > 0) {
      wRangeBracket = mech_c3_range_to_hit_calculate(
          mech, target, section, weapindx, range,
          mech_network_range(mech, target, range, c3Ref, 0), wAmmoMode, &rbth);

      BTHADD("C3iRange", rbth);
    } else {
      wRangeBracket = mech_range_to_hit_calculate(
          mech, target, section, weapindx, range, wFireMode, wAmmoMode, &rbth);
      BTHADD("Range", rbth);
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

    if (MechWeapons[weapindx].special & PCOMBAT)
      tmp_range = (int)(range * 10 + 0.95);
    else
      tmp_range = (int)(range + 0.95);

    if (tmp_range > (mech_section_is_underwater(mech, section)
                         ? MechWeapons[weapindx].medrange_water
                         : MechWeapons[weapindx].medrange))
      BTHADD("TargComp/Long",
             mech_targeting_computer_type(mech) == TARGCOMP_LONG ? -1 : 1);
    else if (tmp_range <= (mech_section_is_underwater(mech, section)
                               ? MechWeapons[weapindx].medrange_water
                               : MechWeapons[weapindx].medrange))
      BTHADD("TargComp/Short",
             mech_targeting_computer_type(mech) == TARGCOMP_SHORT ? -1 : 1);
  }

  if (target && mech_infantry_technology_flags(target) & STEALTH_TECH) {
    int infantry_technology = mech_infantry_technology_flags(target);
    if (infantry_technology & FWL_ACHILEUS_STEALTH_TECH) {
      if (wRangeBracket == RANGE_SHORT)
        BTHADD("FWLStealthBonus", 1);
      else if (wRangeBracket == RANGE_MED)
        BTHADD("FWLStealthBonus", 2);
      else if (wRangeBracket == RANGE_LONG)
        BTHADD("FWLStealthBonus", 3);
    } else if (infantry_technology & DC_KAGE_STEALTH_TECH) {
      if (wRangeBracket == RANGE_MED)
        BTHADD("DCStealthBonus", 1);
      else if (wRangeBracket == RANGE_LONG)
        BTHADD("DCStealthBonus", 2);
    } else if (infantry_technology & FC_INFILTRATOR_STEALTH_TECH) {
      if (wRangeBracket == RANGE_MED)
        BTHADD("FCStealthBonus", 1);
      else if (wRangeBracket == RANGE_LONG)
        BTHADD("FCStealthBonus", 2);
    } else if (infantry_technology & FC_INFILTRATORII_STEALTH_TECH) {
      if (wRangeBracket == RANGE_SHORT)
        BTHADD("FCStealthIIBonus", 1);
      if (wRangeBracket == RANGE_MED)
        BTHADD("FCStealthIIBonus", 1);
      else if (wRangeBracket == RANGE_LONG)
        BTHADD("FCStealthIIBonus", 2);
    }
  }

  /* Add in the movement modifiers */
  if (mech_section_configuration_has(mech, section, STABILIZERS_DESTROYED))
    BTHADD("AttackMoveX2", mech_attacker_movement_modifier(mech) * 2);
  else
    BTHADD("AttackMove", mech_attacker_movement_modifier(mech));

  /* Add mods for overheating */
  BTHADD("Overheat", mech_overheat_to_hit_modifier(mech));

  /* Add special weapon mods */
  if (wAmmoMode & AC_AP_MODE)
    BTHADD("ArmorPiercing", 1);

  if (mech_has_section_special(mech, INARC_HAYWIRE_ATTACHED))
    BTHADD("HaywirePod", 1);

  if (target && (wAmmoMode & NARC_MODE) &&
      (!(MechWeapons[weapindx].special & NARC)) &&
      mech_has_section_special(target, INARC_HOMING_ATTACHED))
    BTHADD("iNARC", -1);

  if (MechWeapons[weapindx].special & PULSE)
    BTHADD("Pulse", -2);

  if (MechWeapons[weapindx].special & MRM)
    BTHADD("MRM", 1);

  if (MechWeapons[weapindx].special & HVYW)
    BTHADD("HeavyWeapon", 1);
  if (target && (wAmmoMode & STINGER_MODE)) {
    if (mech_is_flying_type(target) && !mech_is_landed(target))
      BTHADD("Stinger (Flying)", -3);
    else if (mech_is_out_of_control(target))
      BTHADD("Stinger (OOD)", -1);
    else if (mech_is_jumping(target))
      BTHADD("Stinger (Jumping)", 0);
  }

  if (MechWeapons[weapindx].special & ROCKET)
    BTHADD("Rocket Launcher", 1);

  if (target && (mech_class(target) == CLASS_VTOL) &&
      (fabs(mech_current_speed(target)) > 0.0 ||
       fabs(mech_vertical_speed(target)) > 0.0))
    BTHADD("TargetVTOL", 1);

  if (target && mech_targeting_computer_type(mech) == TARGCOMP_AA) {
    if (!mech_is_landed(target) &&
        (mech_is_flying_type(target) || mech_is_jumping(target) ||
         mech_is_out_of_control(target)))
      BTHADD("TargComp/AA-Fly",
             mech_technology_flags(mech) & AA_TECH ? -3 : -2);
    else
      BTHADD("TargComp/AA-Ground", 1);
  }

  /* -1 for LBX, unless it's a VTOL... then -3 */
  if (wAmmoMode & LBX_MODE)
    BTHADD("LBX", (target && (mech_class(target) == CLASS_VTOL) ? -3 : -1));

  /* Unstable lock */
  if (!btech_context_overrides_weapon_arcs(context) &&
      (!spotter && target &&
       ((mech_target_dbref(mech) != mech_dbref(target)) ||
        (mech_event_count(mech, EVENT_LOCK) &&
         mech_targeting_computer_type(mech) != TARGCOMP_MULTI)))) {
    if (FindTargetXY(mech, &enemyX, &enemyY, &enemyZ)) {
      if (InWeaponArc(mech, enemyX, enemyY) & (FORWARDARC | TURRETARC))
        BTHADD("UnstableLock/Fwarc", 1);
      else
        BTHADD("UnstableLock", 2);
    } else {
      BTHADD("HipShot-NoLock", 2);
    }
  }

  if (mech_targeting_computer_type(mech) == TARGCOMP_MULTI) {
    if (FindTargetXY(mech, &enemyX, &enemyY, &enemyZ)) {
      if (!(InWeaponArc(mech, enemyX, enemyY) & FORWARDARC))
        BTHADD("TargComp/MultiSideArc", 1);
    }
  }

  /* -4 for firing at a hex */
  if (!target && mech_targets_hex_or_building(mech))
    BTHADD("HexBonus", -4);

  /* -2 for firing at someone dropping out of the sky */
  if (target && mech_cocoon_integrity(target) > 0)
    BTHADD("OODbonus", -2);

  /* Indirect fire terrain modifiers */
  if (indirectFire < 1000)
    BTHADD("IDFTerrain", indirectFire);

  /* +1 if spotting */
  if (mech_spotter_dbref(mech) == mech_dbref(mech))
    BTHADD("Spotting", 1);

  /* if our target is another unit... */
  if (target) {
    /* Add the dig-in bonus */
    if (mech_condition_summary(target).dug_in &&
        (!btech_context_dig_bonus_requires_front(context) ||
         (mech_hit_group(mech, target) == FRONT)) &&
        (mech_position_z(target) >= mech_position_z(mech)))
      BTHADD("DugIn", btech_context_dig_bonus(context));

    /* -3 if it's a DS... most people can hit the broadside of a barn */
    if (mech_is_dropship(target))
      BTHADD("DSBonus", -3);

    /* Add +1 for BSuit dispersion */
    if (mech_class(target) == CLASS_BSUIT)
      BTHADD("Bsuitbonus", 1);

    /* Let's see if we're targetting the head */
    if (target && !IsMissile(weapindx) && mech_aim_section(mech) == HEAD &&
        (mech_class(target) == CLASS_MECH || mech_class(target) == CLASS_MW)) {
      if (mech_is_immobile(target))
        BTHADD("HeadTarget", 7);
      else
        BTHADD("HeadTarget-Fake", 25);
    } else {
      if ((wFireMode & ON_TC) && !condition.targeting_computer_destroyed &&
          !(wAmmoMode & LBX_MODE)) {
        if (mech_aim_section(mech) != NUM_SECTIONS && !mech_is_immobile(target))
          BTHADD("TC-Target-NotImmobile", 3);
        else
          BTHADD("TC", -1);
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
            BTHADD("TargetMove (SG Tag -MoveMod)", wTargMoveMod);
          }
        } else {
          BTHADD("TargetMove (SG No Tag)", wTargMoveMod);
        }
      }
    } else {
      BTHADD("TargetMove", wTargMoveMod);
    }

    /* Add in the terrain modifier */
    if (indirectFire >= 1000) {
      j = mech_los_terrain_modifier(mech, target, mech_map, range, wAmmoMode);
      if (j < 1000)
        BTHADD("Terrain/is_light(Sensor)", j);
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
        BTHADD("Light Woods bonus", -1);
      else if (map_real_terrain_get(mech_map, mech_position_x(target),
                                    mech_position_y(target)) == HEAVY_FOREST)
        BTHADD("Heavy Woods bonus", -2);
    }
#ifdef BT_MOVEMENT_MODES
    MechConditionSummary target_condition = mech_condition_summary(target);
    if (target_condition.sprinting || target_condition.evading) {
      if (target_condition.sprinting)
        BTHADD("SprintingTarget",
               btech_context_sprint_to_hit_modifier(context));
      if (!target_condition.fallen && target_condition.evading)
        BTHADD("EvadingTarget", 1);
    }
#endif
  }

  /* Check for damage */
  BTHADD("CritDamage", getCritAddedBTH(mech, section, critical, wRangeBracket));
  if (condition.to_hit_debug)
    notify_printf(btech_context_evaluation(context), mech_pilot_dbref(mech),
                  "BTHDebug: %s", bthbuf);

  BTHEND(mech);
  return baseToHit;
}

int mech_artillery_to_hit_calculate(Mech *mech, int section, int weapindx,
                                    int indirect, float range) {
  int baseToHit = 11;
  Mech *spotter;

  BtechContext *context = mech_context(mech);

  if (mech_section_is_underwater(mech, section))
    return 5000;

  if (weapon_catalogue_effective_range(
          weapindx, btech_context_uses_extended_weapon_ranges(context)) < range)
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

int mech_range_to_hit_calculate(Mech *mech, Mech *target, int section,
                                int weapindx, float frange, int firemode,
                                int ammomode, int *wBTH) {
  int range;
  int wTargetStealth = 0;
  BtechContext *context = mech_context(mech);

  if (target)
    wTargetStealth = mech_condition_summary(target).stealth_armor_active ||
                     mech_condition_summary(target).null_signature_active;

  if (MechWeapons[weapindx].special & PCOMBAT)
    range = (int)(frange * 10 + 0.95);
  else
    range = (int)(frange + 0.95);

  if (mech_section_is_underwater(mech, section)) {
    if (MechWeapons[weapindx].shortrange_water <= 0) {
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
    if (range > GunWaterRange(weapindx)) {
      *wBTH = wTargetStealth ? 12 : 8;
      return RANGE_EXTREME;
    }

    /* Long range... */
    if (range > MechWeapons[weapindx].medrange_water) {
      *wBTH = wTargetStealth ? 6 : 4;
      return RANGE_LONG;
    }

    /* Medium range */
    if (range > MechWeapons[weapindx].shortrange_water) {
      *wBTH = wTargetStealth ? 3 : 2;
      return RANGE_MED;
    }

    /* Short range */
    if (range > MechWeapons[weapindx].min_water) {
      *wBTH = 0;
      return RANGE_SHORT;
    }

    if (range == 0) {
      if (MechWeapons[weapindx].min_water == 0) {
        *wBTH = 0;
        return RANGE_SHORT;
      } else {
        *wBTH = MechWeapons[weapindx].min_water - range;
        return RANGE_SHORT;
      }
    }

    /* Less than or equal to minimum range */
    *wBTH = MechWeapons[weapindx].min_water - range + 1;
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
  if (range > GunRange(weapindx)) {
    *wBTH = wTargetStealth ? 12 : 8;
    return RANGE_EXTREME;
  }

  /* Long range... */
  if (range > MechWeapons[weapindx].medrange) {
    *wBTH = wTargetStealth ? 6 : 4;
    return RANGE_LONG;
  }

  /* Medium range */
  if (range > MechWeapons[weapindx].shortrange) {
    *wBTH = wTargetStealth ? 3 : 2;
    return RANGE_MED;
  }

  /* Short range */
  if (range > MechWeapons[weapindx].min) {
    *wBTH = 0;
    return RANGE_SHORT;
  }
  /* If we are at range 0.0

   * Added 8/3/99 by Kipsta (to fix a 0.0 bug)
   */

  if (range == 0) {
    if (MechWeapons[weapindx].min == 0) {
      *wBTH = 0;
      return RANGE_SHORT;
    } else {
      if (!weapon_catalogue_is_hot_loaded(weapindx, firemode)) {
        *wBTH = MechWeapons[weapindx].min - range;
      } else {
        if (btech_context_hotload_uses_half_modifier(context))
          *wBTH = ((MechWeapons[weapindx].min - range + 1) / 2);
        else
          *wBTH = 0;
      }

      return RANGE_SHORT;
    }
  }

  if (weapon_catalogue_is_hot_loaded(weapindx, firemode)) {
    if (btech_context_hotload_uses_half_modifier(context))
      *wBTH = ((MechWeapons[weapindx].min - range + 1) / 2);
    else
      *wBTH = 0;

    return RANGE_SHORT;
  }

  /* Less than or equal to minimum range */
  *wBTH = MechWeapons[weapindx].min - range + 1;
  return RANGE_SHORT;
}

int mech_c3_range_to_hit_calculate(Mech *mech, Mech *target, int section,
                                   int weapindx, float realRange, float c3Range,
                                   int mode, int *wBTH) {
  int realRangeAdj = 0.0;
  int c3RangeAdj = 0.0;
  int wTargetStealth = 0;
  BtechContext *context = mech_context(mech);

  if (target)
    wTargetStealth = mech_condition_summary(target).stealth_armor_active ||
                     mech_condition_summary(target).null_signature_active;

  if (MechWeapons[weapindx].special & PCOMBAT) {
    realRangeAdj = (int)(realRange * 10 + 0.95);
    c3RangeAdj = (int)(c3Range * 10 + 0.95);
  } else {
    realRangeAdj = (int)(realRange + 0.95);
    c3RangeAdj = (int)(c3Range + 0.95);
  }

  if (mech_section_is_underwater(mech, section)) {
    if (MechWeapons[weapindx].shortrange_water <= 0) {
      *wBTH = 5000;
      return RANGE_NOWATER;
    }

    /* Out of range. No ERange in C3 */
    if (realRangeAdj > GunWaterRange(weapindx)) {
      *wBTH = 1000;
      return RANGE_TOFAR;
    }

    /* Long range... */
    if (c3RangeAdj > MechWeapons[weapindx].medrange_water) {
      *wBTH = wTargetStealth ? 6 : 4;
      return RANGE_LONG;
    }

    /* Medium range */
    if (c3RangeAdj > MechWeapons[weapindx].shortrange_water) {
      *wBTH = wTargetStealth ? 3 : 2;
      return RANGE_MED;
    }

    /* Short range */
    *wBTH = 0;
    return RANGE_SHORT;
  }

  /* Beyond range */
  if (realRangeAdj > GunRange(weapindx)) {
    *wBTH = 1000;
    return RANGE_TOFAR;
  }

  /* No V. Long range in a C3 network */
  /* Long range... */
  if (c3RangeAdj > MechWeapons[weapindx].medrange) {
    *wBTH = wTargetStealth ? 6 : 4;
    return RANGE_LONG;
  }

  /* Medium range */
  if (c3RangeAdj > MechWeapons[weapindx].shortrange) {
    *wBTH = wTargetStealth ? 3 : 2;
    return RANGE_MED;
  }

  /* Short range */
  if (realRange > MechWeapons[weapindx].min) {
    *wBTH = 0;
    return RANGE_SHORT;
  }

  /* Check for range 0.0 */
  if (c3RangeAdj == 0) {
    if (MechWeapons[weapindx].min == 0) {
      *wBTH = 0;
      return RANGE_SHORT;
    }
  }

  /* We don't care about min range if we're Hotloading */
  if (!weapon_catalogue_is_hot_loaded(weapindx, mode)) {
    if (btech_context_hotload_uses_half_modifier(context))
      *wBTH = ((MechWeapons[weapindx].min - realRange + 1) / 2);
    else
      *wBTH = 0;

    return RANGE_SHORT;
  }

  /* Less than or equal to minimum PHYSICAL range */
  *wBTH = MechWeapons[weapindx].min - realRange + 1;
  return RANGE_SHORT;
}
