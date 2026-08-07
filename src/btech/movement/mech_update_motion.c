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
#include <stdlib.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "equipment_types.h"
#include "floatsim.h"
#include "map_conditions_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "registry_api.h"
#include "section_types.h"

void mech_heading_update(Mech *mech) {
  int offset;
  int normangle;
  int mw_mod = 1;
  float maxspeed, omaxspeed;
  BattleMap *mech_map;
  BtechContext *context = mech_context(mech);

  if (mech_heading_degrees(mech) == mech_desired_heading_degrees(mech))
    return;
  maxspeed = mech_effective_maximum_speed(mech);
  if (mech_is_aerospace_unit(mech))
    maxspeed = maxspeed * ACCEL_MOD;
  if ((mech_excess_heat(mech) >= 9.) &&
      (mech_technology_flags(mech) & TRIPLE_MYOMER_TECH))
    maxspeed += 1.5 * MP1;
  omaxspeed = maxspeed;
  normangle = mech_heading_fixed_difference(mech);
  if (mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT)
    mw_mod = 60;
  else if (mech_movement_type(mech) == MOVE_QUAD)
    mw_mod = 2;
  if (btech_context_uses_fasa_turning(context)) {
    constexpr int FASA_TURN_MOD = 3 / 2;
    if (mech_is_jumping(mech))
      offset = 2 * short_to_float_simulation(1) * 2 * 360 * FASA_TURN_MOD / 60;
    else {
      float ts = mech_current_speed(mech);

      if (ts < 0) {
        maxspeed = maxspeed * 2.0 / 3.0;
        ts = -ts;
      }
      if (ts > maxspeed || maxspeed < 0.1) /* kludge */
        offset = 0;
      else {
        offset = short_to_float_simulation(1) * 2 * 360 * FASA_TURN_MOD / 60 *
                 (maxspeed - ts) * (omaxspeed / maxspeed) * mw_mod *
                 MP_PER_KPH / 6; /* hmm. */
      }
    }
  } else {
    if (mech_is_jumping(mech)) {
      mech_map = btech_context_find_object(context, mech_map_dbref(mech));
      float jump_speed = mech_jump_speed(mech);
      if (mech_is_under_gravity(mech) && mech_map) {
        int gravity = battle_map_gravity(mech_map);
        jump_speed = jump_speed * 100 / (gravity > 50 ? gravity : 50);
      }
      offset = short_to_float_simulation(1) * 6 *
               (int)(jump_speed * MP_PER_KPH) * mw_mod;
    } else if (fabs(mech_current_speed(mech)) < 1.0)
      offset =
          short_to_float_simulation(1) * 3 * maxspeed * MP_PER_KPH * mw_mod;
    else {
      offset =
          short_to_float_simulation(1) * 2 * maxspeed * MP_PER_KPH * mw_mod;
      if ((short_to_float_simulation(abs(normangle)) > offset) &&
          mech_current_speed(mech) > 2.0 * maxspeed / 3.0 + 0.1) {
        if (mech_current_speed(mech) > maxspeed)
          offset -= offset / 2 * maxspeed / mech_current_speed(mech);
        else
          offset -=
              offset / 2 * (3.0 * mech_current_speed(mech) / maxspeed - 2.0);
      }
    }
  }
  /*   offset = offset * 2 * MOVE_MOD; - Twice as fast as this;dunno why - */
  offset = offset * MOVE_MOD;
#ifdef BT_MOVEMENT_MODES
  MechConditionSummary conditions = mech_condition_summary(mech);
  if (conditions.tight_turn_mode &&
      HasBoolAdvantage(context, mech_pilot_dbref(mech), "maneuvering_ace"))
    offset = (offset * 3) / 2;
  if ((conditions.sprinting || conditions.evading) &&
      !HasBoolAdvantage(context, mech_pilot_dbref(mech), "maneuvering_ace")) {
    if (HasBoolAdvantage(context, mech_pilot_dbref(mech), "speed_demon"))
      offset = (offset * 2) / 3;
    else
      offset = (offset / 2);
  }
#endif
  if (normangle < 0)
    normangle += short_to_float_simulation(360);
  if (mech_is_dropship(mech) && offset >= short_to_float_simulation(10))
    offset = short_to_float_simulation(10);
  mech_heading_rotate_toward_desired(mech, offset);
}
