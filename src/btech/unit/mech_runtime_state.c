#include "mech_runtime_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

bool mech_is_started(const Mech *mech) { return mech->rd.status & STARTED; }

DbRef mech_autopilot_dbref(const Mech *mech) { return mech->rd.autopilot_num; }

void mech_autopilot_dbref_set(Mech *mech, DbRef autopilot) {
  mech->rd.autopilot_num = autopilot;
}

void mech_seen_count_decrement(Mech *mech) {
  if (mech->rd.num_seen > 0)
    mech->rd.num_seen--;
}

void mech_seen_count_reset(Mech *mech) { mech->rd.num_seen = 0; }

DbRef mech_carried_dbref(const Mech *mech) { return mech->rd.carrying; }

void mech_carried_dbref_set(Mech *mech, DbRef carried) {
  mech->rd.carrying = carried;
  mech->rd.critstatus &= ~LOAD_OK;
}

bool mech_is_towed(const Mech *mech) { return mech->rd.status & TOWED; }

void mech_towed_clear(Mech *mech) { mech->rd.status &= ~TOWED; }

void mech_environment_conditions_set(Mech *mech, bool special, bool temperature,
                                     bool gravity, bool vacuum) {
  mech->rd.status &= ~CONDITIONS;
  if (!special)
    return;
  mech->rd.status |= UNDERSPECIAL;
  if (temperature)
    mech->rd.status |= UNDERTEMPERATURE;
  if (gravity)
    mech->rd.status |= UNDERGRAVITY;
  if (vacuum)
    mech->rd.status |= UNDERVACUUM;
}
