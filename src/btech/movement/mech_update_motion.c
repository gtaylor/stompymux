/* Implements BattleTech movement mechanics for unit update motion. */

#include "mech_update_api.h"

#include <math.h>
#include <stdlib.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
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
  int const turn_unit = short_to_float_simulation(1);
  float const turn_unit_float = (float)turn_unit;
  float maxspeed, omaxspeed;
  BattleMap *mech_map;
  BtechContext *context = mech_context(mech);

  if (mech_heading_degrees(mech) == mech_desired_heading_degrees(mech))
    return;
  maxspeed = mech_effective_maximum_speed(mech);
  if (mech_is_aerospace_unit(mech))
    maxspeed = maxspeed * ACCEL_MOD;
  if ((mech_excess_heat(mech) >= 9.0F) &&
      (mech_technology_flags(mech) & TRIPLE_MYOMER_TECH))
    maxspeed += 1.5F * MP1;
  omaxspeed = maxspeed;
  normangle = mech_heading_fixed_difference(mech);
  if (mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT)
    mw_mod = 60;
  else if (mech_movement_type(mech) == MOVE_QUAD)
    mw_mod = 2;
  if (btech_context_uses_fasa_turning(context)) {
    constexpr float FASA_TURN_MOD = 1.5F;
    if (mech_is_jumping(mech))
      offset = clamp_float_to_int(2.0F * turn_unit_float * 2.0F * 360.0F *
                                  FASA_TURN_MOD / 60.0F);
    else {
      float ts = mech_current_speed(mech);

      if (ts < 0) {
        maxspeed = maxspeed * 2.0F / 3.0F;
        ts = -ts;
      }
      if (ts > maxspeed || maxspeed < 0.1F) /* kludge */
        offset = 0;
      else {
        float const offset_float = turn_unit_float * 2.0F * 360.0F *
                                   FASA_TURN_MOD / 60.0F * (maxspeed - ts) *
                                   (omaxspeed / maxspeed) * (float)mw_mod *
                                   MP_PER_KPH / 6.0F;
        offset = clamp_float_to_int(offset_float); /* hmm. */
      }
    }
  } else {
    if (mech_is_jumping(mech)) {
      mech_map = btech_context_find_object(context, mech_map_dbref(mech));
      float jump_speed = mech_jump_speed(mech);
      if (mech_is_under_gravity(mech) && mech_map) {
        int gravity = battle_map_gravity(mech_map);
        int const effective_gravity = gravity > 50 ? gravity : 50;
        jump_speed = jump_speed * 100.0F / (float)effective_gravity;
      }
      offset = short_to_float_simulation(1) * 6 *
               clamp_float_to_int(jump_speed * MP_PER_KPH) * mw_mod;
    } else if (fabsf(mech_current_speed(mech)) < 1.0F) {
      float const offset_float =
          turn_unit_float * 3.0F * maxspeed * MP_PER_KPH * (float)mw_mod;
      offset = clamp_float_to_int(offset_float);
    } else {
      float const offset_float =
          turn_unit_float * 2.0F * maxspeed * MP_PER_KPH * (float)mw_mod;
      offset = clamp_float_to_int(offset_float);
      if ((short_to_float_simulation(abs(normangle)) > offset) &&
          mech_current_speed(mech) > 2.0F * maxspeed / 3.0F + 0.1F) {
        const int half_offset = offset / 2;
        if (mech_current_speed(mech) > maxspeed)
          offset =
              clamp_float_to_int((float)offset - (float)half_offset * maxspeed /
                                                     mech_current_speed(mech));
        else
          offset = clamp_float_to_int(
              (float)offset -
              (float)half_offset *
                  (3.0F * mech_current_speed(mech) / maxspeed - 2.0F));
      }
    }
  }
  /*   offset = offset * 2 * MOVE_MOD; - Twice as fast as this;dunno why - */
  offset = clamp_float_to_int((float)offset * (float)MOVE_MOD);
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
  if (mech_is_dropship(mech) && offset >= short_to_float_simulation(10))
    offset = short_to_float_simulation(10);
  mech_heading_rotate_toward_desired(mech, offset);
}
