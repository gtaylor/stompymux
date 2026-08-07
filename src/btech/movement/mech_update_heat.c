/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "mech_update_api.h"

#include <math.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "registry_api.h"

static int mech_heat_sinks_enable(Mech *mech, int numsinks) {

  int maximum =
      (mech_technology_flags(mech) & (DOUBLE_HEAT_TECH | CLAN_TECH)) ? 4 : 2;
  numsinks = numsinks < maximum ? numsinks : maximum;
  int disabled = mech_disabled_heat_sink_count(mech);
  numsinks = numsinks < disabled ? numsinks : disabled;

  if (!numsinks)
    return 0;

  mech_disabled_heat_sinks_set(mech, disabled - numsinks);
  /* We don't check for water after enabling them, only the next tic. */
  mech_heat_dissipation_add(mech, numsinks);
#ifdef HEATCUTOFF_DEBUG
  mech_printf(mech, MECHALL,
              "[fg=green]%d heatsink%s kick%s into action.[reset]", numsinks,
              numsinks == 1 ? "" : "s", numsinks == 1 ? "s" : "");
#endif

  return numsinks;
}

static int mech_heat_sinks_disable(Mech *mech, int numsinks) {

  int maximum =
      (mech_technology_flags(mech) & (DOUBLE_HEAT_TECH | CLAN_TECH)) ? 4 : 2;
  numsinks = numsinks < maximum ? numsinks : maximum;
  int active = (int)mech_active_heat_sinks(mech);
  numsinks = numsinks < active ? numsinks : active;

  if (!numsinks)
    return 0;

  mech_disabled_heat_sinks_set(mech,
                               mech_disabled_heat_sink_count(mech) + numsinks);
  /* Submerged heatsinks silently still dissipate some heat. */
  mech_heat_dissipation_add(mech, -numsinks);
#ifdef HEATCUTOFF_DEBUG
  mech_printf(mech, MECHALL,
              "[fg=yellow]%d heatsink%s hum%s into silence.[reset]", numsinks,
              numsinks == 1 ? "" : "s", numsinks == 1 ? "s" : "");
#endif

  return numsinks;
}

/* Update the Unit's current heat values as well as
 * send messages to the pilot based on heat level */
void mech_heat_update(Mech *mech) {

  int legsinks;
  float maxspeed;
  float intheat;
  float inheat;
  BattleMap *map;

  // These guys don't get heat updates.
  if (!mech_uses_heat(mech))
    return;

  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);

  inheat = mech_excess_heat(mech);
  maxspeed = mech_effective_maximum_speed(mech);
  mech_heat_production_set(mech, 0.0F);

  if (mech_position_terrain(mech) == BATTLE_TERRAIN_FIRE &&
      mech_class(mech) == CLASS_MECH)
    mech_heat_production_add(mech, 5.0F);

  /* We do a trick here.  We look at the previous heat level to determine
   * if TSM is/was on.  If it is/was, we recalc what running and walk speeds are
   * to better set how much heat the unit is putting out */
  if (mech_technology_flags(mech) & TRIPLE_MYOMER_TECH) {
    if (inheat >= 9.)
      maxspeed =
          ceil((rint((mech_effective_maximum_speed(mech) / 1.5) / MP1) + 1) *
               1.5) *
          MP1;
  }

  if (fabs(mech_current_speed(mech)) > 0.0) {
#ifndef BT_MOVEMENT_MODES
    if (mech_desired_speed(mech) > 2.0F * maxspeed / 3.0F + 0.1F)
      mech_heat_production_add(mech, 2.0F);
#else
    if (condition.sprinting || condition.evading)
      mech_heat_production_add(mech, 3.0F);
    else if (mech_desired_speed(mech) > 2.0F * maxspeed / 3.0F + 0.1F)
      mech_heat_production_add(mech, 2.0F);
#endif
    else
      mech_heat_production_add(mech, 1.0F);
  }

  if (mech_is_jumping(mech))
    mech_heat_production_add(mech, mech_jump_speed(mech) * MP_PER_KPH > 3.0F
                                       ? mech_jump_speed(mech) * MP_PER_KPH
                                       : 3.0F);

  if (mech_is_started(mech))
    mech_heat_production_add(mech, (float)mech_engine_heat(mech));

  if (condition.stealth_armor_active)
    mech_heat_production_add(mech, 10.0F);

  if (condition.null_signature_active)
    mech_heat_production_add(mech, 10.0F);

  intheat = mech_heat_production(mech);

  mech_heat_production_add(mech, mech_weapon_heat(mech));

  /* ADD Water effects here */
  if (battle_terrain_is_water(mech_real_terrain_get(mech)) &&
      mech_position_z(mech) <= -1) {
    legsinks = FindLegHeatSinks(mech);
    legsinks = (legsinks > 4) ? 4 : legsinks;
    float active_sinks = mech_active_heat_sinks(mech);
    if (mech_position_z(mech) == -1 && !mech_is_fallen(mech)) {
      float immersed_sinks = legsinks + active_sinks;
      mech_heat_dissipation_set(mech, 2 * active_sinks < immersed_sinks
                                          ? 2 * active_sinks
                                          : immersed_sinks);
    } else {
      float immersed_sinks = 6 + active_sinks;
      mech_heat_dissipation_set(mech, 2 * active_sinks < immersed_sinks
                                          ? 2 * active_sinks
                                          : immersed_sinks);
    }
  } else {
    mech_heat_dissipation_set(mech, mech_active_heat_sinks(mech));
  }

  /* Infernoed */
  if (mech_is_jellied(mech)) {
    mech_heat_dissipation_add(mech, -6.0F);
    if (mech_heat_dissipation(mech) < 0.0F)
      mech_heat_dissipation_set(mech, 0.0F);
  }

  if (mech_is_under_special_conditions(mech))
    if ((map = btech_context_find_object(context, mech_map_dbref(mech))))
      if (battle_map_uses_special_rules(map))
        if (battle_map_temperature(map) < -30 ||
            battle_map_temperature(map) > 50) {
          if (battle_map_temperature(map) < -30)
            mech_heat_dissipation_add(
                mech, (-30 - battle_map_temperature(map) + 9) / 10);
          else
            mech_heat_dissipation_add(
                mech, -(battle_map_temperature(map) - 50 + 9) / 10);
        }

  /* Handle heat cutoff now */
  /* Sink enable/disable helpers also adjust heat dissipation. */
  /* Re-Written to use Exile's code - Dany 12/05 */
  if (mech_heat_cutoff_is_enabled(mech)) {
    float overheat = mech_heat_production(mech) - mech_heat_dissipation(mech);

    if (overheat >= 10.)
      mech_heat_sinks_enable(mech, floor(overheat - 10.) + 1);
    else if (overheat < 9.)
      mech_heat_sinks_disable(mech, floor(9. - overheat) + 1);

  } else if (mech_disabled_heat_sink_count(mech)) {
    mech_heat_sinks_enable(mech, 100);
  }

  mech_excess_heat_set(mech, mech_heat_production(mech) -
                                 mech_heat_dissipation(mech));

  /* No lowering of heat if heat is under 9 */
  mech_weapon_heat_add(mech, -(mech_heat_dissipation(mech) - intheat) /
                                 WEAPON_RECYCLE_TIME);

  if (mech_weapon_heat(mech) < 0.0)
    mech_weapon_heat_set(mech, 0.0F);

  if (mech_excess_heat(mech) < 0.0)
    mech_excess_heat_set(mech, 0.0F);

  /* Rule Reference: BMR Revised, Page 17 (Heat=>26 +2 Bruise, Heat=>15 +1
   * Bruise, w/o Lifesupport) */
  /* Rule Reference: Total Warfare, Page 42 (Heat=>26 +2 Bruise, Heat=>15 +1
   * Bruise, w/o Lifesupport) */
  /* Custom Rule: Give bruise if heat > 30 and Random 0 or 1 */

  if ((btech_context_event_tick(context) % TURN) == 0)
    if (mech_life_support_is_destroyed(mech) ||
        (mech_excess_heat(mech) > 30. &&
         btech_random_range(context, 0, 1) == 0)) {
      if (mech_excess_heat(mech) > 25.) {
        mech_notify(mech, MECHPILOT, "You take personal injury from heat!");
        headhitmwdamage(mech, mech,
                        mech_life_support_is_destroyed(mech) ? 2 : 1);
      } else if (mech_excess_heat(mech) >= 15.) {
        mech_notify(mech, MECHPILOT, "You take personal injury from heat!");
        headhitmwdamage(mech, mech, 1);
      }
    }

  if (mech_excess_heat(mech) >= 19.) {
    if (inheat < 19.) {
      mech_notify(mech, MECHALL,
                  "[fg=red bold]=====================================\n"
                  "Your Excess Heat indicator turns RED!\n"
                  "=====================================[reset]");
    }
  } else if (mech_excess_heat(mech) >= 14.) {
    if (inheat >= 19. || inheat < 14.) {
      mech_notify(mech, MECHALL,
                  "[fg=yellow bold]=======================================\n"
                  "Your Excess Heat indicator turns YELLOW\n"
                  "=======================================[reset]");
    }
  } else {
    if (inheat >= 14.) {
      mech_notify(mech, MECHALL,
                  "[fg=green]======================================\n"
                  "Your Excess Heat indicator turns GREEN\n"
                  "======================================[reset]");
    }
  }
  mech_overheat_handle(mech);
}
