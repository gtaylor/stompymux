/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997-2002 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "artillery_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "failures.h"
#include "failures_api.h"
#include "map.h"
#include "map_api.h"
#include "map_conditions_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_bth_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_spot_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/attrs.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"

void mech_fireweapon(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map;
  char *args[5];
  int argc;
  int weapnum;

  BtechContext *context = mech_context(mech);
  mech_map = btech_context_get_map(context, mech_map_dbref(mech));
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_condition_summary(mech).weapons_hold) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Currently in weapons hold. Unable to fire weapons.");
    return;
  }
  argc = mech_parseattributes(buffer, args, 5);
  if (argc < 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Not enough arguments to the function");
    return;
  }
  weapnum = atoi(args[0]);
  FireWeaponNumber(player, mech, mech_map, weapnum, argc, args, 0);
}

typedef enum MechWeaponArcCheck {
  MECH_WEAPON_ARC_AVAILABLE,
  MECH_WEAPON_ARC_NOT_CONTROLLED,
  MECH_WEAPON_ARC_OUTSIDE,
} MechWeaponArcCheck;

static MechWeaponArcCheck mech_weapon_arc_check(Mech *mech, float x, float y,
                                                int section, int critical) {
  int unavailable = mech_unusable_weapon_arcs(mech);
  if (unavailable) {
    int arc = InWeaponArc(mech, x, y);
    int override = btech_context_weapon_arc_override(mech_context(mech));
    if ((!override && (unavailable & arc)) || (override && !(override & arc)))
      return MECH_WEAPON_ARC_NOT_CONTROLLED;
  }
  return IsInWeaponArc(mech, x, y, section, critical)
             ? MECH_WEAPON_ARC_AVAILABLE
             : MECH_WEAPON_ARC_OUTSIDE;
}

/*
 * Main weapon firing routine
 */
int FireWeaponNumber(DbRef player, Mech *mech, BattleMap *mech_map, int weapnum,
                     int argc, char **args, int sight) {
  int weaptype;
  DbRef target;
  char targetID[2];
  int mapx = 0, mapy = 0, LOS = 0;
  Mech *tempMech = NULL;
  int section, critical;
  float range = 0;
  float enemyX = 0, enemyY = 0, enemyZ = 0;
  int ishex = 0;
  int wcDeadLegs = 0;
  char location[20];
  int mode;
  int i;
  BtechContext *context = mech_context(mech);
  MechConditionSummary conditions = mech_condition_summary(mech);

  if (mech_class(mech) == CLASS_BSUIT) {
    for (i = 0; i < NUM_BSUIT_MEMBERS; i++) {
      if (!mech_section_is_destroyed(mech, i) &&
          mech_section_recycle_ticks(mech, i)) {
        mecha_notify(
            btech_context_evaluation(context), player,
            tprintf("Suit %d is still recovering from attack.", i + 1));
        return -1;
      }
    }
  }

  /* If they fire their weapon while hidden, they should appear */
  if (!sight && conditions.hidden) {
    mech_notify(mech, MECHALL,
                "You break out of your cover to initiate weapons fire!");
    mech_los_broadcast(mech,
                       "breaks out of its cover and begins firing rabidly!");
    mech_hidden_set(mech, false);
  }

  /* If they fire a weapon while trying to hide stop them from hiding */
  if (!sight) {
    mech_event_cancel(mech, EVENT_HIDE);
  }
#ifdef BT_MOVEMENT_MODES
  if (mech_move_mode_locked(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot fire while using a special movement mode.");
    return 0;
  }
#endif
  if (mech_spotter_dbref(mech) > 0 &&
      mech_spotter_dbref(mech) == mech_dbref(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot fire while spotting.");
    return 0;
  }
  if (weapnum < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return 0;
  }

  weaptype = FindWeaponNumberOnMech_Advanced(mech, weapnum, &section, &critical,
                                             sight);

  if (weaptype == -1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return 0;
  }

  mode = mech_critical_fire_mode(mech, section, critical);

  if (!sight) {

    /* Exile Stun Code Check */
    if (conditions.stunned) {
      mecha_notify(
          btech_context_evaluation(context), player,
          "You cannot take actions while stunned! That includes finding the "
          "trigger.");
      return 0;
    }

    if (mech_critical_temporary_failure(mech, section, critical)) {
      mecha_notify(
          btech_context_evaluation(context), player,
          "The weapons system chirps: 'That weapon is still unusable - "
          "please stand by.'");
      return 0;
    }
    if (weaptype == -3) {
      mecha_notify(
          btech_context_evaluation(context), player,
          "The weapons system chirps: 'That weapon is still reloading!'");
      return 0;
    }
    if (weaptype == -4) {
      mecha_notify(
          btech_context_evaluation(context), player,
          "The weapons system chirps: 'That weapon is still recharging!'");
      return 0;
    }

    /* New fancy message for when they try and fire a weapon and the section
     * is busy */
    if (weaptype == -5) {

      /* Get the section name and print the message */
      ArmorStringFromIndex(section, location, mech_class(mech),
                           mech_movement_type(mech));
      notify_printf(btech_context_evaluation(context), player,
                    "%s%s is still recovering from a "
                    "previous action!",
                    mech_class(mech) == CLASS_BSUIT ? "" : "Your ", location);
      return 0;
    }

    if (mech_section_carries_club(mech, section)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You're carrying a club in that arm.");
      return 0;
    }

    if (conditions.fallen && mech_class(mech) == CLASS_MECH) {

      /* if a quad has 3 of 4 legs dead, it can't fire at all while prone */
      wcDeadLegs = CountDestroyedLegs(mech);
      if (mech_is_quad(mech))
        if (wcDeadLegs > 2) {
          mecha_notify(btech_context_evaluation(context), player,
                       "Quads need at least 3 legs to fire while prone.");
          return 0;
        }

      /* quads with all 4 legs can fire all weapons while prone. They do not
       * need to prop. */
      if (!mech_is_quad(mech) || (mech_is_quad(mech) && wcDeadLegs > 0)) {
        if (section == RLEG || section == LLEG) {
          mecha_notify(btech_context_evaluation(context), player,
                       "You cannot fire leg mounted weapons when prone.");
          return 0;
        }
        switch (section) {
        case RARM:
          if (mech_section_has_recycling_weapon(mech, LARM) ||
              mech_section_recycle_ticks(mech, LARM) ||
              mech_section_is_destroyed(mech, LARM)) {
            mecha_notify(
                btech_context_evaluation(context), player,
                "You currently can't use your Left Arm to prop yourself up.");
            return 0;
          }
          break;
        case LARM:
          if (mech_section_has_recycling_weapon(mech, RARM) ||
              mech_section_recycle_ticks(mech, RARM) ||
              mech_section_is_destroyed(mech, RARM)) {
            mecha_notify(
                btech_context_evaluation(context), player,
                "Your currently can't use your Right Arm to prop yourself up.");
            return 0;
          }
          break;
        default:
          if ((mech_section_has_recycling_weapon(mech, RARM) ||
               mech_section_recycle_ticks(mech, RARM) ||
               mech_section_is_destroyed(mech, RARM)) &&
              (mech_section_has_recycling_weapon(mech, LARM) ||
               mech_section_recycle_ticks(mech, LARM) ||
               mech_section_is_destroyed(mech, LARM))) {
            mecha_notify(btech_context_evaluation(context), player,
                         "You currently don't have any arms to spare to prop "
                         "yourself up.");
            return 0;
          }
        }
      }
    }
  }

  if (bsuit_has_friendly_riders(mech)) {
    if (((section == CTORSO) || (section == RTORSO) || (section == LTORSO))) {
      mecha_notify(
          btech_context_evaluation(context), player,
          "You cannot fire torso-mounted weapons while you have battlesuits on "
          "you!");
      return 0;
    }
  }

  if (conditions.dug_in && section != TURRET) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Only turret weapons are available while in cover.");
    return 0;
  }
  if (weaptype == -2 || (mech_critical_temporary_failure(
                             mech, section, critical) == FAIL_DESTROYED)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "The weapons system chirps: 'That weapon has been destroyed!'");
    return 0;
  }
  if (MechWeapons[weaptype].special & AMS) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That weapon is defensive only!");
    return 0;
  }
  if (argc > 3) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments!");
    return 0;
  }

  if ((MechWeapons[weaptype].special & IDF) && mech_spotter_dbref(mech) != -1 &&
      mech_target_dbref(mech) == -1) {
    mech_spot_fire(player, mech, mech_map, weapnum, weaptype, sight, section,
                   critical);
    return 1;
  }

  /* We're set to look at a spotter, its a non-idf weapon. We should just not
   * fire */
  if ((mech_spotter_dbref(mech) != -1) &&
      !(MechWeapons[weaptype].special & IDF)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "The weapon system chirps: 'Somone is spotting for you. Remove your "
        "spotter to fire non-IDF weapons'");
    return 0;
  }

  switch (argc) {

    /* Fire at default target */
  case 1:

    /* If its a coolant gun in heat mode we should shot our mech */
    if (weapon_catalogue_is_coolant(weaptype) && (mode & HEAT_MODE)) {

      /* Setting our mech as the target and the other parameters
       * as well */
      tempMech = mech;
      if (!tempMech) {
        mecha_notify(btech_context_evaluation(context), player,
                     "Error in FireWeaponNumber routine");
        return 0;
      }
      enemyX = mech_position_real_x(tempMech);
      enemyY = mech_position_real_y(tempMech);
      enemyZ = mech_position_real_z(tempMech);
      mapx = mech_position_x(tempMech);
      mapy = mech_position_y(tempMech);
      range = 0.2F;
      LOS = 1;

    } else {

      if (!FindTargetXY(mech, &enemyX, &enemyY, &enemyZ)) {
        mecha_notify(btech_context_evaluation(context), player,
                     "You do not have a default target set!");
        return 0;
      }

      if (mech_target_dbref(mech) != -1) {

        tempMech = btech_context_get_mech(context, mech_target_dbref(mech));
        if (!tempMech) {
          mecha_notify(btech_context_evaluation(context), player,
                       "Error in FireWeaponNumber routine");
          return 0;
        }
        mapx = mech_position_x(tempMech);
        mapy = mech_position_y(tempMech);
        range = mech_range_to(mech, tempMech);
        LOS = mech_los_check_unblocked(mech, tempMech, mapx, mapy, range);

        if (!(MechWeapons[weaptype].special & IDF)) {
          if (!LOS) {
            mecha_notify(btech_context_evaluation(context), player,
                         "That target is not in your line of sight!");
            return 0;
          }
        } else if (battle_map_is_underground(mech_map)) {
          if (!LOS) {
            mecha_notify(btech_context_evaluation(context), player,
                         "That target is not in your direct line of sight, and "
                         "you cannot fire your IDF weapons underground!");
            return 0;
          }
        }
        if (btech_context_idf_requires_spotter(context) &&
            (MechWeapons[weaptype].special & IDF) &&
            (mech_spotter_dbref(mech) == -1))
          if (!LOS) {
            mecha_notify(btech_context_evaluation(context), player,
                         "That target is not in your direct line of sight"
                         " and you do not have a spotter set!!");
            return 0;
          }
      } else {

        /* default target is a hex */
        ishex = 1;
        if (!sight && !weapon_catalogue_is_artillery(weaptype) &&
            conditions.unit_target_lock) {

          /* look for enemies in the default hex cause they may have moved */
          if ((tempMech =
                   find_mech_in_hex(mech, mech_map, mech_target_hex_x(mech),
                                    mech_target_hex_y(mech), 0))) {

            enemyX = mech_position_real_x(tempMech);
            enemyY = mech_position_real_y(tempMech);
            enemyZ = mech_position_real_z(tempMech);
            mapx = mech_position_x(tempMech);
            mapy = mech_position_y(tempMech);
          }
        }

        if (!tempMech) {
          mapx = mech_target_hex_x(mech);
          mapy = mech_target_hex_y(mech);
          mech_target_hex_z_set(mech,
                                battle_map_hex_elevation(mech_map, mapx, mapy));
          const int target_hex_z = mech_target_hex_z(mech);
          enemyZ = ZSCALE * (float)target_hex_z;
          MapCoordToRealCoord(mapx, mapy, &enemyX, &enemyY);
        }

        /* don't check LOS for missile weapons firing at hex number */
        range =
            FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                      mech_position_real_z(mech), enemyX, enemyY, enemyZ);
        LOS = mech_los_check_unblocked(mech, tempMech, mapx, mapy, range);

        /* Check for Spotter here */
        if (btech_context_idf_requires_spotter(context) &&
            (MechWeapons[weaptype].special & IDF) &&
            (mech_spotter_dbref(mech) == -1))
          if (!LOS) {
            mecha_notify(btech_context_evaluation(context), player,
                         "That hex target is not in your direct line of sight"
                         " and you do not have a spotter set!!");
            return 0;
          }

        if (!(weapon_catalogue_is_artillery(weaptype) ||
              (MechWeapons[weaptype].special & IDF))) {
          if (!LOS) {
            mecha_notify(btech_context_evaluation(context), player,
                         "That hex target is not in your line of sight!");
            return 0;
          }
        } else if (battle_map_is_underground(mech_map)) {
          if (!LOS) {
            mecha_notify(btech_context_evaluation(context), player,
                         "That target is not in your direct line of sight, and "
                         "you cannot fire your IDF weapons underground!");
            return 0;
          }
        }
      }

      if (mech_class(mech) != CLASS_BSUIT) {
        MechWeaponArcCheck arc =
            mech_weapon_arc_check(mech, enemyX, enemyY, section, critical);
        if (arc == MECH_WEAPON_ARC_NOT_CONTROLLED) {
          mecha_notify(btech_context_evaluation(context), player,
                       "That arc's weapons aren't under your control!");
          return 0;
        }
        if (arc == MECH_WEAPON_ARC_OUTSIDE) {
          mecha_notify(btech_context_evaluation(context), player,
                       "Default target is not in your weapons arc!");
          return 0;
        }
      }
    }
    break;

  case 2:
    /* Fire at the numbered target */
    targetID[0] = args[1][0];
    targetID[1] = args[1][1];
    target = FindTargetDBREFFromMapNumber(mech, targetID);
    if (target == -1) {
      mecha_notify(btech_context_evaluation(context), player,
                   "That target is not in your line of sight!");
      return 0;
    }
    tempMech = btech_context_get_mech(context, target);
    if (!tempMech) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Error in FireWeaponNumber routine!");
      return 0;
    }
    enemyX = mech_position_real_x(tempMech);
    enemyY = mech_position_real_y(tempMech);
    enemyZ = mech_position_real_z(tempMech);
    mapx = mech_position_x(tempMech);
    mapy = mech_position_y(tempMech);

    range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                      mech_position_real_z(mech), enemyX, enemyY, enemyZ);
    LOS = mech_los_check_unblocked(mech, tempMech, mech_position_x(tempMech),
                                   mech_position_y(tempMech), range);

    if (!LOS) {
      mecha_notify(btech_context_evaluation(context), player,
                   "That target is not in your line of sight!");
      return 0;
    }

    if (mech_class(mech) != CLASS_BSUIT) {
      MechWeaponArcCheck arc =
          mech_weapon_arc_check(mech, enemyX, enemyY, section, critical);
      if (arc == MECH_WEAPON_ARC_NOT_CONTROLLED) {
        mecha_notify(btech_context_evaluation(context), player,
                     "That arc's weapons aren't under your control!");
        return 0;
      }
      if (arc == MECH_WEAPON_ARC_OUTSIDE) {
        mecha_notify(btech_context_evaluation(context), player,
                     "That target is not in your weapons arc!");
        return 0;
      }
    }
    break;

  case 3:

    /* Fire at the Map X Y */
    mapx = atoi(args[1]);
    mapy = atoi(args[2]);
    ishex = 1;
    if (!battle_map_coordinate_is_valid(mech_map, mapx, mapy)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Map coordinates out of range!");
      return 0;
    }

    if (!sight && !weapon_catalogue_is_artillery(weaptype))

      /* look for enemies in that hex... */
      if ((tempMech = find_mech_in_hex(mech, mech_map, mapx, mapy, 0))) {
        enemyX = mech_position_real_x(tempMech);
        enemyY = mech_position_real_y(tempMech);
        enemyZ = mech_position_real_z(tempMech);
      }

    if (!tempMech) {
      MapCoordToRealCoord(mapx, mapy, &enemyX, &enemyY);
      mech_target_hex_z_set(mech,
                            battle_map_hex_elevation(mech_map, mapx, mapy));
      const int target_hex_z = mech_target_hex_z(mech);
      enemyZ = ZSCALE * (float)target_hex_z;
    }

    if (mech_class(mech) != CLASS_BSUIT) {
      MechWeaponArcCheck arc =
          mech_weapon_arc_check(mech, enemyX, enemyY, section, critical);
      if (arc == MECH_WEAPON_ARC_NOT_CONTROLLED) {
        mecha_notify(btech_context_evaluation(context), player,
                     "That arc's weapons aren't under your control!");
        return 0;
      }
      if (arc == MECH_WEAPON_ARC_OUTSIDE) {
        mecha_notify(btech_context_evaluation(context), player,
                     "That hex target is not in your weapons arc!");
        return 0;
      }
    }

    /* Don't check LOS for missile weapons */
    range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                      mech_position_real_z(mech), enemyX, enemyY, enemyZ);
    LOS = mech_los_check_unblocked(mech, tempMech, mapx, mapy, range);

    if (!weapon_catalogue_is_artillery(weaptype))
      if (!LOS) {
        mecha_notify(btech_context_evaluation(context), player,
                     "That hex target is not in your line of sight!");
        return 0;
      }
    break;

  default:
    return 0;
  }

  if (tempMech) {
    if (weapon_catalogue_is_artillery(weaptype)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You can only target hexes with this kind of artillery.");
      return 0;
    }
    if (mech_swarm_target(tempMech) == mech_dbref(mech)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are unable to use your weapons against a 'swarmer!");
      return 0;
    }
    if (mech_condition_summary(tempMech).stealth_armor_active &&
        ((mech_target_dbref(mech) != mech_dbref(tempMech)) ||
         mech_event_count(mech, EVENT_LOCK))) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You need a stable lock to fire on that target!");
      return 0;
    }
    if (!weapon_catalogue_is_coolant(weaptype) &&
        mech_team(tempMech) == mech_team(mech) &&
        conditions.friendly_fire_safety) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You can't fire on a teammate with FFSafeties on!");
      return 0;
    }
    if (!weapon_catalogue_is_coolant(weaptype) &&
        mech_team(tempMech) == mech_team(mech) &&
        battle_map_blocks_friendly_fire(mech_map)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Friendly Fire? I don't think so...");
      return 0;
    }
    if (mech_class(tempMech) == CLASS_MW && mech_class(mech) != CLASS_MW &&
        !conditions.player_killer) {
      mecha_notify(
          btech_context_evaluation(context), player,
          "That's a living, breathing person! Switch off the safety first, "
          "if you really want to assassinate the target.");
      return 0;
    }
  }

  FireWeapon(mech, mech_map, tempMech, LOS, weaptype, weapnum, section,
             critical, enemyX, enemyY, mapx, mapy, range, 1000, sight, ishex);
  return (1);
}

int weapon_failure_stuff(Mech *mech, int *weapnum, int *weapindx, int *section,
                         int *critical, int *ammoLoc, int *ammoCrit,
                         int *ammoLoc1, int *ammoCrit1, int *modifier,
                         int *type, float range, int *range_ok,
                         int wGattlingShots) {
  mech_weapon_failure_check(mech, *weapnum, *weapindx, *section, *critical,
                            modifier, type);
  if (*type == POWER_SPIKE)
    return 1;
  if (*type == WEAPON_JAMMED || *type == WEAPON_DUD) {
    /* Just decrement ammunition */
    mech_ammunition_decrement(mech, *weapindx, *section, *critical, *ammoLoc,
                              *ammoCrit, *ammoLoc1, *ammoCrit1, wGattlingShots);
    return 1;
  }
  if (*type == RANGE) {
    int effective_range =
        mech_section_is_underwater(mech, *section)
            ? weapon_catalogue_effective_water_range(
                  *weapindx,
                  btech_context_uses_extended_weapon_ranges(mech_context(mech)))
            : weapon_catalogue_effective_range(
                  *weapindx, btech_context_uses_extended_weapon_ranges(
                                 mech_context(mech)));
    if ((float)(effective_range - *modifier) < range) {
      mech_notify(
          mech, MECHALL,
          "Due to weapons failure your shot falls short of its target!");
      *range_ok = 0;
    }
  }
  return 0;
}

void mech_c3_track_emit(Mech *mech, DbRef c3Ref, Mech *c3Mech) {
  if (c3Mech && mech_dbref(c3Mech) != mech_dbref(mech)) {
    mech_printf(mech, MECHALL, "Using range data from %s [%s]",
                btech_attribute_read(
                    btech_context_database(mech_context(c3Mech)),
                    mech_dbref(c3Mech), A_MECHNAME, (char[LBUF_SIZE]){0}),
                mech_id(c3Mech, true).text);
  }
}
