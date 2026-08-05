#include "mech_runtime_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

bool mech_is_started(const Mech *mech) { return mech->rd.status & STARTED; }

bool mech_is_destroyed(const Mech *mech) { return mech->rd.status & DESTROYED; }

bool mech_is_landed(const Mech *mech) { return mech->rd.status & LANDED; }

bool mech_is_jumping(const Mech *mech) { return mech->rd.status & JUMPING; }

bool mech_is_out_of_control(const Mech *mech) { return mech->rd.cocoon; }

bool mech_is_blinded(const Mech *mech) { return mech->rd.status & BLINDED; }

void mech_blinded_set(Mech *mech, bool blinded) {
  if (blinded)
    mech->rd.status |= BLINDED;
  else
    mech->rd.status &= ~BLINDED;
}

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

bool mech_is_observer(const Mech *mech) {
  return mech->rd.critstatus & OBSERVATORIC;
}

bool mech_is_under_gravity(const Mech *mech) {
  return mech->rd.status & UNDERGRAVITY;
}

bool mech_has_destroyed_gyro(const Mech *mech) {
  return mech->rd.critstatus & GYRO_DESTROYED;
}

int mech_seen_count(const Mech *mech) { return mech->rd.num_seen; }

void mech_movement_stop(Mech *mech) {
  mech->rd.speed = 0.0F;
  mech->rd.desired_speed = 0.0F;
}

void mech_last_use_reset(Mech *mech) { mech->rd.lastused = 0; }

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
