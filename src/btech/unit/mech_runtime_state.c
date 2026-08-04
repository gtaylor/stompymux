#include "mech_runtime_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

bool mech_is_started(const Mech *mech) { return mech->rd.status & STARTED; }

bool mech_is_destroyed(const Mech *mech) { return mech->rd.status & DESTROYED; }

bool mech_suppresses_gunnery_experience(const Mech *mech) {
  return mech->rd.status2 & NO_GUN_XP;
}

bool mech_player_character_initialization_begin(Mech *mech) {
  if (mech->ud.type != CLASS_MW || (mech->rd.critstatus & PC_INITIALIZED))
    return false;
  mech->rd.critstatus |= PC_INITIALIZED;
  return true;
}

bool mech_pilot_is_unconscious(const Mech *mech) {
  return mech->rd.status & UNCONSCIOUS;
}

void mech_movement_stop(Mech *mech) {
  mech->rd.speed = 0.0F;
  mech->rd.desired_speed = 0.0F;
}

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
