#include "autopilot_autogun_internal.h"

#include "autopilot.h"
#include "autopilot_autogun_api.h"
#include "autopilot_combat_policy_api.h"
#include "autopilot_weapon_profile_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
#include "mech_classification_api.h"
#include "mech_combat_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/red_black_tree.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

#include <math.h>
#include <stdio.h>

static AutopilotWeapon *
autopilot_weapon_profile_previous(RedBlackTree profile, AutopilotWeapon *weapon,
                                  int range) {
  return red_black_tree_search(profile, SEARCH_PREV,
                               autopilot_weapon_range_score_key(weapon, range));
}

void autopilot_autogun_fire(Autopilot *autopilot, Mech *mech, BattleMap *map,
                            Mech *target) {
  char buffer[LBUF_SIZE];
  float range;
  float accumulate_heat;
  float maximum_speed;
  int target_arc;
  int relative_bearing;
  AutopilotWeapon *weapon;

  /* Log It */
  autopilot_autogun_log(autopilot, "Autogun - Starting Weapon Attack Phase");

  /* Get range from mech to current target */
  range = map_real_range(&(MapRealSegment){
      .start = {.x = mech_position_real_x(mech),
                .y = mech_position_real_y(mech)},
      .end = {.x = mech_position_real_x(target),
              .y = mech_position_real_y(target)},
  });

  /* This probably unnecessary but since it doesn't
   * take much to calc range it should be ok for
   * testing for now */
  if ((range >= (float)AUTO_GUN_MAX_RANGE) &&
      !autopilot_has_assigned_target(autopilot)) {

    /* Target is to far - reset */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log it */
    autopilot_autogun_log(autopilot, "Autogun - Target out of range");
    autopilot_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  const int PROFILE_RANGE = (int)range;
  RedBlackTree weapon_profile =
      range >= 0.0F && range < (float)AUTO_GUN_MAX_RANGE
          ? autopilot_weapon_profile_get(autopilot, PROFILE_RANGE)
          : nullptr;

  /* Cycle through Guns while watching the heat */
  if (weapon_profile != nullptr) {

    /* Ok we got weapons lets use them */

    /* Reset heat counter to current heat */
    accumulate_heat = mech_weapon_heat(mech);

    /* If the unit is moving need to account for the heat of that as well */
    if ((mech_class(mech) == CLASS_MECH) &&
        (fabsf(mech_current_speed(mech)) > 0.0F)) {

      maximum_speed = mech_effective_maximum_speed(mech);
      if (mech_desired_speed(mech) > ((2.0F * maximum_speed / 3.0F) + 0.1F))
        accumulate_heat += 2;
      else
        accumulate_heat += 1;
    }

    /* Get first weapon */
    weapon = (AutopilotWeapon *)red_black_tree_search(weapon_profile,
                                                      SEARCH_LAST, nullptr);

    while (weapon) {

      const int WEAPON_HEAT = weapon_catalogue_heat(weapon->weapon_db_number);
      const bool STINGER_COMPATIBLE =
          !(mech_critical_ammo_mode(mech, weapon->section, weapon->critical) &
            STINGER_MODE) ||
          target == nullptr || mech_is_jumping(target) ||
          mech_is_out_of_control(target) ||
          (mech_is_flying_type(target) && !mech_is_landed(target));
      const bool AMMUNITION_REQUIRED =
          weapon_catalogue_ammunition_per_ton(weapon->weapon_db_number) > 0;
      const AutopilotWeaponDecision ELIGIBILITY =
          autopilot_weapon_evaluate(&(AutopilotWeaponSituation){
              .functional = !mech_weapon_is_nonfunctional_at(
                  mech, weapon->section, weapon->critical,
                  weapon_from_equipment_index(weapon->weapon_db_number)),
              .recycling = mech_weapon_is_recycling_at(mech, weapon->section,
                                                       weapon->critical),
              .defensive =
                  weapon_catalogue_is_anti_missile(weapon->weapon_db_number),
              .ammunition_required = AMMUNITION_REQUIRED,
              .ammunition =
                  AMMUNITION_REQUIRED
                      ? count_ammo_for_weapon(mech, weapon->weapon_db_number)
                      : 0,
              .ammunition_compatible = STINGER_COMPATIBLE,
              .in_arc = true,
              .heat_limited = mech_class(mech) == CLASS_MECH,
              .projected_heat = accumulate_heat,
              .heat_dissipation = mech_heat_dissipation(mech),
              .weapon_heat = WEAPON_HEAT,
              .maximum_heat = AUTO_GUN_MAX_HEAT});
      if (!ELIGIBILITY.fire) {
        weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                   PROFILE_RANGE);
        continue;
      }

      /* Ok passed the checks now setup the arcs and see if we can fire it */

      /* Ok the rest depends on what type of unit we driving */
      if ((mech_class(mech) == CLASS_MECH) &&
          (mech_movement_type(mech) == MOVE_BIPED)) {

        /* Center ourself and get target arc */
        mech_torso_twist_set(mech, MECH_TORSO_CENTER);
        if (mech_technology_flags(mech) & FLIPABLE_ARMS) {

          /* Center the arms if need be */
          mech_arms_center(mech);
        }

        /* Get Target Arc */
        target_arc = in_weapon_arc(mech, mech_position_real_x(target),
                                   mech_position_real_y(target));

        /* Now go through the various arcs and see if we
         * need to flip arm or rotorso or something */
        if (target_arc & REARARC) {

          if (weapon->section == LARM || weapon->section == RARM) {

            /* First see if we can flip arms */
            if (mech_technology_flags(mech) & FLIPABLE_ARMS) {

              /* Flip the arms */
              mech_arms_flip(mech);

            } else {

              /* Now see if we can rotatorso */

              /* Find out if it would be better to
               * rotate left or right */
              relative_bearing =
                  mech_heading_degrees(mech) -
                  map_bearing(&(MapRealSegment){
                      .start = {.x = mech_position_real_x(mech),
                                .y = mech_position_real_y(mech)},
                      .end = {.x = mech_position_real_x(target),
                              .y = mech_position_real_y(target)}});

              if (relative_bearing > 120 && relative_bearing < 180 &&
                  weapon->section == RARM) {

                /* Rotate Right */
                mech_torso_twist_set(mech, MECH_TORSO_RIGHT);

              } else if (relative_bearing > 180 && relative_bearing < 240 &&
                         weapon->section == LARM) {

                /* Rotate Left */
                mech_torso_twist_set(mech, MECH_TORSO_LEFT);

              } else {

                /* Can't do anything so go to next weapon */
                weapon = autopilot_weapon_profile_previous(
                    weapon_profile, weapon, PROFILE_RANGE);

                continue;
              }
            }

          } else if (!(mech_critical_fire_mode(mech, weapon->section,
                                               weapon->critical) &
                       REAR_MOUNT)) {

            /* Weapon is forward torso or leg mounted weapon
             * so no way to shoot with */
            weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                       PROFILE_RANGE);

            continue;
          }

          /* ELSE: Weapon is rear mounted so don't need to
           * do anything */

        } else if (target_arc & LSIDEARC) {

          if (weapon->section == RLEG || weapon->section == LLEG) {

            /* No way can we hit him with leg mounted
             * weapons so lets go to next one */
            weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                       PROFILE_RANGE);

            continue;
          }

          /* Rotate torso left */
          mech_torso_twist_set(mech, MECH_TORSO_LEFT);

        } else if (target_arc & RSIDEARC) {

          if (weapon->section == RLEG || weapon->section == LLEG) {

            /* No way can we hit him with leg mounted
             * weapons so lets go to next one */
            weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                       PROFILE_RANGE);

            continue;
          }

          /* Rotate torso right */
          mech_torso_twist_set(mech, MECH_TORSO_RIGHT);

        } else {

          if (mech_critical_fire_mode(mech, weapon->section, weapon->critical) &
              REAR_MOUNT) {

            /* No way can we hit the guy with a rear
             * gun so lets go to next one */
            weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                       PROFILE_RANGE);

            continue;
          }
        }

      } else if ((mech_class(mech) == CLASS_MECH) &&
                 (mech_movement_type(mech) == MOVE_QUAD)) {

        /* Get Target Arc */
        target_arc = in_weapon_arc(mech, mech_position_real_x(target),
                                   mech_position_real_y(target));

        if (target_arc & REARARC) {

          if (!(mech_critical_fire_mode(mech, weapon->section,
                                        weapon->critical) &
                REAR_MOUNT)) {

            /* Weapon is not rear mounted so skip it and
             * go to the next weapon */
            weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                       PROFILE_RANGE);

            continue;
          }

        } else if (target_arc & FORWARDARC) {

          if (mech_critical_fire_mode(mech, weapon->section, weapon->critical) &
              REAR_MOUNT) {

            /* Weapon is rear mounted so skip it and
             * go to the next weapon */
            weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                       PROFILE_RANGE);

            continue;
          }

        } else {

          /* The attacker is in a zone we can't possibly
           * shoot into, so just go to next weapon */
          weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                     PROFILE_RANGE);

          continue;
        }

      } else if ((mech_class(mech) == CLASS_VEH_GROUND) ||
                 (mech_class(mech) == CLASS_VEH_NAVAL)) {

        /* Check if turret exists and weapon is there */
        if (mech_section_internal(mech, TURRET) && weapon->section == TURRET) {

          /* Rotate Turret and nail the guy */
          MechConditionSummary condition = mech_condition_summary(mech);
          if (!condition.turret_jammed && !condition.turret_locked &&
              (acceptable_degree(mech_turret_heading_degrees(mech) +
                                 mech_heading_degrees(mech)) !=
               map_bearing(&(MapRealSegment){
                   .start = {.x = mech_position_real_x(mech),
                             .y = mech_position_real_y(mech)},
                   .end = {.x = mech_position_real_x(target),
                           .y = mech_position_real_y(target)}}))) {

            (void)snprintf(buffer, LBUF_SIZE, "%d",
                           map_bearing(&(MapRealSegment){
                               .start = {.x = mech_position_real_x(mech),
                                         .y = mech_position_real_y(mech)},
                               .end = {.x = mech_position_real_x(target),
                                       .y = mech_position_real_y(target)}}));
            mech_turret(autopilot->mynum, mech, buffer);
          }

        } else {

          /* Check if in arc of weapon */
          if (!is_in_weapon_arc(&(WeaponArcRequest){
                  .mech = mech,
                  .target = {.x = mech_position_real_x(target),
                             .y = mech_position_real_y(target)},
                  .section = weapon->section,
                  .critical = weapon->critical})) {

            /* Not in the arc so lets go to the next weapon */
            weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                       PROFILE_RANGE);

            continue;
          }
        }

      } else {

        /* We're either an aero, ds, bsuit, mechwarrior or vtol
         *
         * Still need to add code for them */
      }

      /* Done moving around, fire the weapon */
      (void)snprintf(buffer, LBUF_SIZE, "%d", weapon->weapon_number);
      mech_fireweapon(autopilot->mynum, mech, buffer);

      /* Log It */
      autopilot_autogun_log(autopilot,
                            "Autogun - Fired Weapon #%d "
                            "at target #%ld",
                            weapon->weapon_number, autopilot->target);

      /* Ok check to see if weapon was fired if so account for the
       * heat */
      if (mech_weapon_is_recycling_at(mech, weapon->section,
                                      weapon->critical)) {
        accumulate_heat += (float)WEAPON_HEAT;
      }

      /* Ok go to the next weapon */
      weapon = autopilot_weapon_profile_previous(weapon_profile, weapon,
                                                 PROFILE_RANGE);

    } /* End of cycling through weapons */
  }

  /* Log It */
  autopilot_autogun_log(autopilot, "Autogun - End Weapon Attack Phase");

  if (autogun_chase_target(autopilot, mech, map, target))
    return;
}
