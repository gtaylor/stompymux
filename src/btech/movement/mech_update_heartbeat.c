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

#include "aero_move_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_ecm_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lite_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_tag_api.h"
#include "mech_utils_api.h"

static int mech_visibility_clamp(int visibility) {
  if (visibility < 0)
    return 0;
  if (visibility > 100)
    return 100;
  return visibility;
}

void mech_update(DbRef key, void *data) {
  Mech *mech = data;

  if (!mech)
    return;
  mech_fired_recently_set(mech, false);
  if (mech_is_aerospace_unit(mech)) {
    aero_update(mech);
    return;
  }
  if (mech_is_started(mech) || mech_pilot_is_unconscious(mech))
    mech_piloting_update(mech);
  if (mech_is_started(mech) || mech_added_heat(mech) > 0.1F)
    mech_heat_update(mech);
  if (mech_is_started(mech)) {
    int visibility = mech_sensor_visibility_modifier(mech) +
                     btech_random_range(mech_context(mech), -40, 40);
    mech_sensor_visibility_modifier_set(mech,
                                        mech_visibility_clamp(visibility));
  }
  mech_ecm_check(mech);
  mech_tag_check(mech);
  end_lite_check(mech);

  if (mech_condition_summary(mech).turret_auto_turn)
    mech_turret_autoturn_update(mech);
}
