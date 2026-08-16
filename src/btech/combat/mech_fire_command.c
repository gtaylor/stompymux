/* Implements BattleTech combat mechanics for unit fire command. */

#include <stdio.h>
#include <stdlib.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "failures.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat.h"
#include "mech_combat_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
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
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static const char *fire_argument(char *const *arguments, size_t index) {
  char *const *argument = (char *const *)checked_storage_at_const(
      (const void *)arguments, 5, sizeof(*arguments), index);
  return *argument;
}

void mech_fireweapon(DbRef player, Mech *mech, char *buffer) {
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
  if (!parse_int_checked(args[0], &weapnum)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid weapon number!");
    return;
  }
  mech_weapon_fire_command(&(WeaponFireCommandRequest){.actor = player,
                                                       .mech = mech,
                                                       .map = mech_map,
                                                       .weapon_number = weapnum,
                                                       .argument_count = argc,
                                                       .arguments = args});
}

typedef enum MechWeaponArcCheck : int {
  MECH_WEAPON_ARC_AVAILABLE,
  MECH_WEAPON_ARC_NOT_CONTROLLED,
  MECH_WEAPON_ARC_OUTSIDE,
} MechWeaponArcCheck;

static MechWeaponArcCheck mech_weapon_arc_check(Mech *mech, float x, float y,
                                                int section, int critical) {
  int unavailable = mech_unusable_weapon_arcs(mech);
  if (unavailable) {
    int arc = in_weapon_arc(mech, x, y);
    int override = btech_context_weapon_arc_override(mech_context(mech));
    if ((!override && (unavailable & arc)) || (override && !(override & arc)))
      return MECH_WEAPON_ARC_NOT_CONTROLLED;
  }
  return is_in_weapon_arc(&(WeaponArcRequest){.mech = mech,
                                              .target = {.x = x, .y = y},
                                              .section = section,
                                              .critical = critical})
             ? MECH_WEAPON_ARC_AVAILABLE
             : MECH_WEAPON_ARC_OUTSIDE;
}

/*
 * Main weapon firing routine
 */
int mech_weapon_fire_command(const WeaponFireCommandRequest *request) {
  const DbRef PLAYER = request->actor;
  Mech *mech = request->mech;
  BattleMap *mech_map = request->map;
  const int WEAPNUM = request->weapon_number;
  const int ARGC = request->argument_count;
  char **args = request->arguments;
  const bool SIGHT = request->sight;
  int weaptype;
  DbRef target;
  char target_id[2];
  int mapx = 0;
  int mapy = 0;
  int los = 0;
  Mech *temp_mech = nullptr;
  int section;
  int critical;
  float range = 0;
  float enemy_x = 0;
  float enemy_y = 0;
  float enemy_z = 0;
  int ishex = 0;
  int wc_dead_legs = 0;
  char location[UNIT_SECTION_NAME_CAPACITY];
  int mode;
  int i;
  BtechContext *context = mech_context(mech);
  MechConditionSummary conditions = mech_condition_summary(mech);

  if (mech_class(mech) == CLASS_BSUIT) {
    for (i = 0; i < NUM_BSUIT_MEMBERS; i++) {
      if (!mech_section_is_destroyed(mech, i) &&
          mech_section_recycle_ticks(mech, i)) {
        mecha_notifyf(btech_context_evaluation(context), PLAYER,
                      "Suit %d is still recovering from attack.", i + 1);
        return -1;
      }
    }
  }

  /* If they fire their weapon while hidden, they should appear */
  if (!SIGHT && conditions.hidden) {
    mech_notify(mech, MECHALL,
                "You break out of your cover to initiate weapons fire!");
    mech_los_broadcast(mech,
                       "breaks out of its cover and begins firing rabidly!");
    mech_hidden_set(mech, false);
  }

  /* If they fire a weapon while trying to hide stop them from hiding */
  if (!SIGHT) {
    mech_event_cancel(mech, EVENT_HIDE);
  }
  if (mech_spotter_dbref(mech) > 0 &&
      mech_spotter_dbref(mech) == mech_dbref(mech)) {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "You cannot fire while spotting.");
    return 0;
  }
  if (WEAPNUM < 0) {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return 0;
  }

  WeaponNumberLookupResult lookup =
      weapon_number_find(&(WeaponNumberLookupRequest){
          .mech = mech, .number = WEAPNUM, .sight = SIGHT});
  weaptype = lookup.value;
  section = lookup.slot.section;
  critical = lookup.slot.critical;

  if (weaptype == -1) {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return 0;
  }

  mode = mech_critical_fire_mode(mech, section, critical);

  if (!SIGHT) {
    /* Exile Stun Code Check */
    if (conditions.stunned) {
      mecha_notify(
          btech_context_evaluation(context), PLAYER,
          "You cannot take actions while stunned! That includes finding the "
          "trigger.");
      return 0;
    }

    if (mech_critical_temporary_failure(mech, section, critical)) {
      mecha_notify(
          btech_context_evaluation(context), PLAYER,
          "The weapons system chirps: 'That weapon is still unusable - "
          "please stand by.'");
      return 0;
    }
    if (weaptype == -3) {
      mecha_notify(
          btech_context_evaluation(context), PLAYER,
          "The weapons system chirps: 'That weapon is still reloading!'");
      return 0;
    }
    if (weaptype == -4) {
      mecha_notify(
          btech_context_evaluation(context), PLAYER,
          "The weapons system chirps: 'That weapon is still recharging!'");
      return 0;
    }

    /* New fancy message for when they try and fire a weapon and the section
     * is busy */
    if (weaptype == -5) {
      /* Get the section name and print the message */
      armor_string_from_index(section, location, mech_class(mech),
                              mech_movement_type(mech));
      notify_printf(btech_context_evaluation(context), PLAYER,
                    "%s%s is still recovering from a "
                    "previous action!",
                    mech_class(mech) == CLASS_BSUIT ? "" : "Your ", location);
      return 0;
    }

    if (mech_section_carries_club(mech, section)) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "You're carrying a club in that arm.");
      return 0;
    }

    if (conditions.fallen && mech_class(mech) == CLASS_MECH) {
      /* if a quad has 3 of 4 legs dead, it can't fire at all while prone */
      wc_dead_legs = count_destroyed_legs(mech);
      if (mech_is_quad(mech)) {
        if (wc_dead_legs > 2) {
          mecha_notify(btech_context_evaluation(context), PLAYER,
                       "Quads need at least 3 legs to fire while prone.");
          return 0;
        }
      }

      /* quads with all 4 legs can fire all weapons while prone. They do not
       * need to prop. */
      if (!mech_is_quad(mech) || (mech_is_quad(mech) && wc_dead_legs > 0)) {
        if (section == RLEG || section == LLEG) {
          mecha_notify(btech_context_evaluation(context), PLAYER,
                       "You cannot fire leg mounted weapons when prone.");
          return 0;
        }
        switch (section) {
        case RARM:
          if (mech_section_has_recycling_weapon(mech, LARM) ||
              mech_section_recycle_ticks(mech, LARM) ||
              mech_section_is_destroyed(mech, LARM)) {
            mecha_notify(
                btech_context_evaluation(context), PLAYER,
                "You currently can't use your Left Arm to prop yourself up.");
            return 0;
          }
          break;
        case LARM:
          if (mech_section_has_recycling_weapon(mech, RARM) ||
              mech_section_recycle_ticks(mech, RARM) ||
              mech_section_is_destroyed(mech, RARM)) {
            mecha_notify(
                btech_context_evaluation(context), PLAYER,
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
            mecha_notify(btech_context_evaluation(context), PLAYER,
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
          btech_context_evaluation(context), PLAYER,
          "You cannot fire torso-mounted weapons while you have battlesuits on "
          "you!");
      return 0;
    }
  }

  if (conditions.dug_in && section != TURRET) {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "Only turret weapons are available while in cover.");
    return 0;
  }
  if (weaptype == -2 || (mech_critical_temporary_failure(
                             mech, section, critical) == FAIL_DESTROYED)) {
    mecha_notify(
        btech_context_evaluation(context), PLAYER,
        "The weapons system chirps: 'That weapon has been destroyed!'");
    return 0;
  }
  if (weapon_catalogue_is_anti_missile(weaptype)) {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "That weapon is defensive only!");
    return 0;
  }
  if (ARGC > 3) {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "Invalid number of arguments!");
    return 0;
  }

  if (weapon_catalogue_supports_indirect_fire(weaptype) &&
      mech_spotter_dbref(mech) != -1 && mech_target_dbref(mech) == -1) {
    mech_spot_fire(PLAYER, mech, mech_map, WEAPNUM, weaptype, SIGHT, section,
                   critical);
    return 1;
  }

  /* We're set to look at a spotter, its a non-idf weapon. We should just not
   * fire */
  if ((mech_spotter_dbref(mech) != -1) &&
      !weapon_catalogue_supports_indirect_fire(weaptype)) {
    mecha_notify(
        btech_context_evaluation(context), PLAYER,
        "The weapon system chirps: 'Somone is spotting for you. Remove your "
        "spotter to fire non-IDF weapons'");
    return 0;
  }

  switch (ARGC) {
    /* Fire at default target */
  case 1:

    /* If its a coolant gun in heat mode we should shot our mech */
    if (weapon_catalogue_is_coolant(weaptype) && (mode & HEAT_MODE)) {
      /* Setting our mech as the target and the other parameters
       * as well */
      temp_mech = mech;
      if (!temp_mech) {
        mecha_notify(btech_context_evaluation(context), PLAYER,
                     "Error in FireWeaponNumber routine");
        return 0;
      }
      enemy_x = mech_position_real_x(temp_mech);
      enemy_y = mech_position_real_y(temp_mech);
      mapx = mech_position_x(temp_mech);
      mapy = mech_position_y(temp_mech);
      range = 0.2F;
      los = 1;

    } else {

      const MechTargetPositionResult TARGET_POSITION =
          mech_target_position(mech);
      if (!TARGET_POSITION.found) {
        mecha_notify(btech_context_evaluation(context), PLAYER,
                     "You do not have a default target set!");
        return 0;
      }
      enemy_x = TARGET_POSITION.position.x;
      enemy_y = TARGET_POSITION.position.y;
      enemy_z = TARGET_POSITION.position.z;

      if (mech_target_dbref(mech) != -1) {
        temp_mech = btech_context_get_mech(context, mech_target_dbref(mech));
        if (!temp_mech) {
          mecha_notify(btech_context_evaluation(context), PLAYER,
                       "Error in FireWeaponNumber routine");
          return 0;
        }
        mapx = mech_position_x(temp_mech);
        mapy = mech_position_y(temp_mech);
        range = mech_range_to(mech, temp_mech);
        los = mech_los_check_unblocked(mech, temp_mech, mapx, mapy, range);

        if (!weapon_catalogue_supports_indirect_fire(weaptype)) {
          if (!los) {
            mecha_notify(btech_context_evaluation(context), PLAYER,
                         "That target is not in your line of sight!");
            return 0;
          }
        } else if (battle_map_is_underground(mech_map)) {
          if (!los) {
            mecha_notify(btech_context_evaluation(context), PLAYER,
                         "That target is not in your direct line of sight, and "
                         "you cannot fire your IDF weapons underground!");
            return 0;
          }
        }
        if (btech_context_idf_requires_spotter(context) &&
            weapon_catalogue_supports_indirect_fire(weaptype) &&
            (mech_spotter_dbref(mech) == -1)) {
          if (!los) {
            mecha_notify(btech_context_evaluation(context), PLAYER,
                         "That target is not in your direct line of sight"
                         " and you do not have a spotter set!!");
            return 0;
          }
        }
      } else {

        /* default target is a hex */
        ishex = 1;
        if (!SIGHT && !weapon_catalogue_is_artillery(weaptype) &&
            conditions.unit_target_lock) {
          /* look for enemies in the default hex cause they may have moved */
          temp_mech = find_mech_in_hex(mech, mech_map, mech_target_hex_x(mech),
                                       mech_target_hex_y(mech), 0);
          if (temp_mech) {
            enemy_x = mech_position_real_x(temp_mech);
            enemy_y = mech_position_real_y(temp_mech);
            enemy_z = mech_position_real_z(temp_mech);
            mapx = mech_position_x(temp_mech);
            mapy = mech_position_y(temp_mech);
          }
        }

        if (!temp_mech) {
          mapx = mech_target_hex_x(mech);
          mapy = mech_target_hex_y(mech);
          mech_target_hex_z_set(mech,
                                battle_map_hex_elevation(mech_map, mapx, mapy));
          const int TARGET_HEX_Z = mech_target_hex_z(mech);
          enemy_z = ZSCALE * (float)TARGET_HEX_Z;
          map_coord_to_real_coord(mapx, mapy, &enemy_x, &enemy_y);
        }

        /* don't check LOS for missile weapons firing at hex number */
        range = map_spatial_range(&(MapSpatialSegment){
            .start = {.x = mech_position_real_x(mech),
                      .y = mech_position_real_y(mech),
                      .z = mech_position_real_z(mech)},
            .end = {.x = enemy_x, .y = enemy_y, .z = enemy_z},
        });
        los = mech_los_check_unblocked(mech, temp_mech, mapx, mapy, range);

        /* Check for Spotter here */
        if (btech_context_idf_requires_spotter(context) &&
            weapon_catalogue_supports_indirect_fire(weaptype) &&
            (mech_spotter_dbref(mech) == -1)) {
          if (!los) {
            mecha_notify(btech_context_evaluation(context), PLAYER,
                         "That hex target is not in your direct line of sight"
                         " and you do not have a spotter set!!");
            return 0;
          }
        }

        if (!(weapon_catalogue_is_artillery(weaptype) ||
              weapon_catalogue_supports_indirect_fire(weaptype))) {
          if (!los) {
            mecha_notify(btech_context_evaluation(context), PLAYER,
                         "That hex target is not in your line of sight!");
            return 0;
          }
        } else if (battle_map_is_underground(mech_map)) {
          if (!los) {
            mecha_notify(btech_context_evaluation(context), PLAYER,
                         "That target is not in your direct line of sight, and "
                         "you cannot fire your IDF weapons underground!");
            return 0;
          }
        }
      }

      if (mech_class(mech) != CLASS_BSUIT) {
        MechWeaponArcCheck arc =
            mech_weapon_arc_check(mech, enemy_x, enemy_y, section, critical);
        if (arc == MECH_WEAPON_ARC_NOT_CONTROLLED) {
          mecha_notify(btech_context_evaluation(context), PLAYER,
                       "That arc's weapons aren't under your control!");
          return 0;
        }
        if (arc == MECH_WEAPON_ARC_OUTSIDE) {
          mecha_notify(btech_context_evaluation(context), PLAYER,
                       "Default target is not in your weapons arc!");
          return 0;
        }
      }
    }
    break;

  case 2:
    /* Fire at the numbered target */
    target_id[0] = *checked_string_suffix(fire_argument(args, 1), 0);
    target_id[1] = *checked_string_suffix(fire_argument(args, 1), 1);
    target = find_target_dbref_from_map_number(mech, target_id);
    if (target == -1) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "That target is not in your line of sight!");
      return 0;
    }
    temp_mech = btech_context_get_mech(context, target);
    if (!temp_mech) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "Error in FireWeaponNumber routine!");
      return 0;
    }
    enemy_x = mech_position_real_x(temp_mech);
    enemy_y = mech_position_real_y(temp_mech);
    enemy_z = mech_position_real_z(temp_mech);
    mapx = mech_position_x(temp_mech);
    mapy = mech_position_y(temp_mech);

    range = map_spatial_range(&(MapSpatialSegment){
        .start = {.x = mech_position_real_x(mech),
                  .y = mech_position_real_y(mech),
                  .z = mech_position_real_z(mech)},
        .end = {.x = enemy_x, .y = enemy_y, .z = enemy_z},
    });
    los = mech_los_check_unblocked(mech, temp_mech, mech_position_x(temp_mech),
                                   mech_position_y(temp_mech), range);

    if (!los) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "That target is not in your line of sight!");
      return 0;
    }

    if (mech_class(mech) != CLASS_BSUIT) {
      MechWeaponArcCheck arc =
          mech_weapon_arc_check(mech, enemy_x, enemy_y, section, critical);
      if (arc == MECH_WEAPON_ARC_NOT_CONTROLLED) {
        mecha_notify(btech_context_evaluation(context), PLAYER,
                     "That arc's weapons aren't under your control!");
        return 0;
      }
      if (arc == MECH_WEAPON_ARC_OUTSIDE) {
        mecha_notify(btech_context_evaluation(context), PLAYER,
                     "That target is not in your weapons arc!");
        return 0;
      }
    }
    break;

  case 3:

    /* Fire at the Map X Y */
    if (!parse_int_checked(fire_argument(args, 1), &mapx) ||
        !parse_int_checked(fire_argument(args, 2), &mapy)) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "Invalid map coordinates!");
      return 0;
    }
    ishex = 1;
    if (!battle_map_coordinate_is_valid(mech_map, mapx, mapy)) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "Map coordinates out of range!");
      return 0;
    }

    if (!SIGHT && !weapon_catalogue_is_artillery(weaptype))

      /* look for enemies in that hex... */
      temp_mech = find_mech_in_hex(mech, mech_map, mapx, mapy, 0);
    if (temp_mech) {
      enemy_x = mech_position_real_x(temp_mech);
      enemy_y = mech_position_real_y(temp_mech);
      enemy_z = mech_position_real_z(temp_mech);
    }

    if (!temp_mech) {
      map_coord_to_real_coord(mapx, mapy, &enemy_x, &enemy_y);
      mech_target_hex_z_set(mech,
                            battle_map_hex_elevation(mech_map, mapx, mapy));
      const int TARGET_HEX_Z = mech_target_hex_z(mech);
      enemy_z = ZSCALE * (float)TARGET_HEX_Z;
    }

    if (mech_class(mech) != CLASS_BSUIT) {
      MechWeaponArcCheck arc =
          mech_weapon_arc_check(mech, enemy_x, enemy_y, section, critical);
      if (arc == MECH_WEAPON_ARC_NOT_CONTROLLED) {
        mecha_notify(btech_context_evaluation(context), PLAYER,
                     "That arc's weapons aren't under your control!");
        return 0;
      }
      if (arc == MECH_WEAPON_ARC_OUTSIDE) {
        mecha_notify(btech_context_evaluation(context), PLAYER,
                     "That hex target is not in your weapons arc!");
        return 0;
      }
    }

    /* Don't check LOS for missile weapons */
    range = map_spatial_range(&(MapSpatialSegment){
        .start = {.x = mech_position_real_x(mech),
                  .y = mech_position_real_y(mech),
                  .z = mech_position_real_z(mech)},
        .end = {.x = enemy_x, .y = enemy_y, .z = enemy_z},
    });
    los = mech_los_check_unblocked(mech, temp_mech, mapx, mapy, range);

    if (!weapon_catalogue_is_artillery(weaptype)) {
      if (!los) {
        mecha_notify(btech_context_evaluation(context), PLAYER,
                     "That hex target is not in your line of sight!");
        return 0;
      }
    }
    break;

  default:
    return 0;
  }

  if (temp_mech) {
    if (weapon_catalogue_is_artillery(weaptype)) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "You can only target hexes with this kind of artillery.");
      return 0;
    }
    if (mech_swarm_target(temp_mech) == mech_dbref(mech)) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "You are unable to use your weapons against a 'swarmer!");
      return 0;
    }
    if (mech_condition_summary(temp_mech).stealth_armor_active &&
        ((mech_target_dbref(mech) != mech_dbref(temp_mech)) ||
         mech_event_count(mech, EVENT_LOCK))) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "You need a stable lock to fire on that target!");
      return 0;
    }
    if (!weapon_catalogue_is_coolant(weaptype) &&
        mech_team(temp_mech) == mech_team(mech) &&
        conditions.friendly_fire_safety) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "You can't fire on a teammate with FFSafeties on!");
      return 0;
    }
    if (!weapon_catalogue_is_coolant(weaptype) &&
        mech_team(temp_mech) == mech_team(mech) &&
        battle_map_blocks_friendly_fire(mech_map)) {
      mecha_notify(btech_context_evaluation(context), PLAYER,
                   "Friendly Fire? I don't think so...");
      return 0;
    }
    if (mech_class(temp_mech) == CLASS_MW && mech_class(mech) != CLASS_MW &&
        !conditions.player_killer) {
      mecha_notify(
          btech_context_evaluation(context), PLAYER,
          "That's a living, breathing person! Switch off the safety first, "
          "if you really want to assassinate the target.");
      return 0;
    }
  }

  mech_weapon_fire(
      &(WeaponFireRequest){.mech = mech,
                           .map = mech_map,
                           .target = temp_mech,
                           .line_of_sight = los,
                           .weapon_index = weaptype,
                           .weapon_number = WEAPNUM,
                           .weapon = {.section = section, .critical = critical},
                           .target_hex = {.x = mapx, .y = mapy},
                           .range = range,
                           .indirect_fire = 1000,
                           .sight = SIGHT,
                           .target_kind = ishex});
  return (1);
}

void mech_c3_track_emit(Mech *mech, DbRef network_reference [[maybe_unused]],
                        Mech *c3_mech) {
  if (c3_mech && mech_dbref(c3_mech) != mech_dbref(mech)) {
    mech_printf(mech, MECHALL, "Using range data from %s [%s]",
                btech_attribute_read(
                    btech_context_database(mech_context(c3_mech)),
                    mech_dbref(c3_mech), A_MECHNAME, (char[LBUF_SIZE]){0}),
                mech_id(c3_mech, true).text);
  }
}
