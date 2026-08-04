#include "command_handlers_api.h"

#include "btech/context.h"
#include "legacy_macros.h"
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
    DOCHECK0_CONTEXT(context, mech_is_destroyed(mech), "You are destroyed!");
    DOCHECK0_CONTEXT(context, !mech_is_started(mech), "Reactor is not online!");
  }

  if (flags & MECH_PILOT)
    DOCHECK0_CONTEXT(context, mech_is_blinded(mech),
                     "You are momentarily blinded!");

  if (flags & MECH_PILOT_CON)
    DOCHECK0_CONTEXT(
        context,
        mech_pilot_is_unconscious(mech) &&
            (!mech_is_started(mech) || player == mech_pilot_dbref(mech)),
        "You are unconscious....zzzzzzz");

  if (flags & MECH_PILOTONLY)
    DOCHECK0_CONTEXT(context,
                     !is_wizard(btech_context_database(context), player) &&
                         is_in_character(btech_context_database(context),
                                         mech_dbref(mech)) &&
                         mech_pilot_dbref(mech) != player,
                     "Now now, only the pilot can push that button.");

  if (flags & MECH_MAP) {
    DOCHECK0_CONTEXT(context, mech_map_dbref(mech) < 0, "You are on no map!");
    if (btech_context_get_map(context, mech_map_dbref(mech)) == nullptr) {
      notify(evaluation, player, "You are on an invalid map! Map index reset!");
      mech_shutdown(player, mech, "");
      mech_map_dbref_set(mech, -1);
      return 0;
    }
  }
  return 1;
}
