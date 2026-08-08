#include "command_handlers_api.h"
#include "mech_notify_api.h"

#include "btech/context.h"
#include "mech_crew_api.h"
#include "mech_identity_api.h"
#include "mech_runtime_api.h"
#include "mech_startup_api.h"
#include "mech_status_types.h"
#include "mux/objects/flags.h"
#include "registry_api.h"

int common_checks(DbRef player, Mech *mech, int flags) {
  if (mech == nullptr)
    return 0;

  BtechContext *context = mech_context(mech);
  EvaluationContext *evaluation = btech_context_evaluation(context);
  mech_last_use_reset(mech);

  if (flags & MECH_STARTED) {
    if (mech_is_destroyed(mech)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are destroyed!");
      return 0;
    }
    if (!mech_is_started(mech)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Reactor is not online!");
      return 0;
    }
  }

  if (flags & MECH_PILOT)
    if (mech_is_blinded(mech)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are momentarily blinded!");
      return 0;
    }

  if (flags & MECH_PILOT_CON)
    if (mech_pilot_is_unconscious(mech) &&
        (!mech_is_started(mech) || player == mech_pilot_dbref(mech))) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are unconscious....zzzzzzz");
      return 0;
    }

  if (flags & MECH_PILOTONLY)
    if (!is_wizard(btech_context_database(context), player) &&
        is_in_character(btech_context_database(context), mech_dbref(mech)) &&
        mech_pilot_dbref(mech) != player) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Now now, only the pilot can push that button.");
      return 0;
    }

  if (flags & MECH_MAP) {
    if (mech_map_dbref(mech) < 0) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You are on no map!");
      return 0;
    }
    if (btech_context_get_map(context, mech_map_dbref(mech)) == nullptr) {
      mecha_notify(evaluation, player,
                   "You are on an invalid map! Map index reset!");
      mech_shutdown(player, mech, "");
      mech_map_dbref_set(mech, -1);
      return 0;
    }
  }
  return 1;
}
