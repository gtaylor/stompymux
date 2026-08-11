#include "equipment_types.h"
#include "mech_update_api.h"

#include "btech/context.h"
#include "btechstats_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_damage_history_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_utils_api.h"

void mech_damage_stagger_check(Mech *wounded) {
  BtechContext *context = mech_context(wounded);
  MechDamageHistory history = mech_damage_history(wounded);
  int now = btech_context_event_tick(context) % TURN;

  if (btech_context_stagger_mode(context))
    return;

  if (!mech_is_dropship(wounded) && history.turn_damage >= 20 &&
      (!history.staggered_last_turn || history.stagger_stamp == now)) {
    if (!mech_is_jumping(wounded) && !mech_is_fallen(wounded) &&
        !mech_is_out_of_control(wounded)) {
      mech_notify(wounded, MECHALL, "You stagger from the damage!");
      if (!made_pilot_skill_roll(wounded, 1)) {
        mech_notify(wounded, MECHALL, "You fall over from all the damage!");
        mech_los_broadcast(wounded, "falls down, staggered by the damage!");
        mech_fall(wounded, 1, 0);
      }
    }
    mech_turn_damage_clear(wounded);
    mech_stagger_stamp_set(wounded, now);
    return;
  }
  if ((history.staggered_last_turn && history.stagger_stamp == now) ||
      (!history.staggered_last_turn && !now)) {
    mech_turn_damage_clear(wounded);
    mech_stagger_stamp_set(wounded, -1);
  }
}
