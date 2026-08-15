#include "mech_runtime_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"
#include "mux/server/platform.h"
#include "section_types.h"
#include <time.h>

bool mech_is_started(const Mech *mech) {
  return (mech->rd.status & STARTED) != 0;
}

bool mech_is_destroyed(const Mech *mech) {
  return (mech->rd.status & DESTROYED) != 0;
}

void mech_destroyed_set(Mech *mech, bool destroyed) {
  if (destroyed)
    mech->rd.status |= DESTROYED;
  else
    mech->rd.status &= ~DESTROYED;
}

bool mech_is_landed(const Mech *mech) {
  return (mech->rd.status & LANDED) != 0;
}

bool mech_is_jumping(const Mech *mech) {
  return (mech->rd.status & JUMPING) != 0;
}

bool mech_is_out_of_control(const Mech *mech) { return mech->rd.cocoon != 0; }

bool mech_is_blinded(const Mech *mech) {
  return (mech->rd.status & BLINDED) != 0;
}

bool mech_has_fired_recently(const Mech *mech) {
  return (mech->rd.status & FIRED) != 0;
}

bool mech_is_invisible(const Mech *mech) {
  return (mech->rd.critstatus & INVISIBLE) != 0;
}

bool mech_autocon_when_shutdown(const Mech *mech) {
  return (mech->rd.status & AUTOCON_WHEN_SHUTDOWN) != 0;
}

bool mech_autocon_include_shutdown_targets(const Mech *mech) {
  return (mech->rd.mech_prefs & MECHPREF_AUTOCON_SD) != 0;
}

bool mech_armor_warning_enabled(const Mech *mech) {
  return (!(mech->rd.mech_prefs & MECHPREF_NOARMORWARN)) != 0;
}

bool mech_ammunition_warning_enabled(const Mech *mech) {
  return (!(mech->rd.mech_prefs & MECHPREF_NOAMMOWARN)) != 0;
}

void mech_blinded_set(Mech *mech, bool blinded) {
  if (blinded)
    mech->rd.status |= BLINDED;
  else
    mech->rd.status &= ~BLINDED;
}

void mech_fired_recently_set(Mech *mech, bool fired) {
  if (fired)
    mech->rd.status |= FIRED;
  else
    mech->rd.status &= ~FIRED;
}

bool mech_suppresses_gunnery_experience(const Mech *mech) {
  return (mech->rd.status2 & NO_GUN_XP) != 0;
}

bool mech_player_character_initialization_begin(Mech *mech) {
  if (mech->ud.type != CLASS_MW || (mech->rd.critstatus & PC_INITIALIZED))
    return false;
  mech->rd.critstatus |= PC_INITIALIZED;
  return true;
}

bool mech_pilot_is_unconscious(const Mech *mech) {
  return (mech->rd.status & UNCONSCIOUS) != 0;
}

bool mech_is_observer(const Mech *mech) {
  return (mech->rd.critstatus & OBSERVATORIC) != 0;
}

bool mech_is_under_gravity(const Mech *mech) {
  return (mech->rd.status & UNDERGRAVITY) != 0;
}

bool mech_is_under_special_conditions(const Mech *mech) {
  return (mech->rd.status & UNDERSPECIAL) != 0;
}

bool mech_is_under_vacuum(const Mech *mech) {
  return (mech->rd.status & UNDERVACUUM) != 0;
}

bool mech_is_immobile(const Mech *mech) {
  return (!(mech->rd.status & STARTED) || (mech->rd.status & UNCONSCIOUS) ||
          (mech->rd.status2 & FORTIFIED) || (mech->rd.status & BLINDED) ||
          mech->ud.move == MOVE_NONE ||
          ((mech->rd.status & FALLEN) && mech->ud.type != CLASS_MECH &&
           mech->ud.type != CLASS_MW)) != 0;
}

bool mech_has_destroyed_gyro(const Mech *mech) {
  return (mech->rd.critstatus & GYRO_DESTROYED) != 0;
}

bool mech_has_damaged_gyro(const Mech *mech) {
  return (mech->rd.critstatus & (GYRO_DAMAGED | GYRO_DESTROYED)) != 0;
}

int mech_cocoon_integrity(const Mech *mech) { return mech->rd.cocoon; }

int mech_seen_count(const Mech *mech) { return mech->rd.num_seen; }

int mech_last_dropship_message_tick(const Mech *mech) {
  return mech->rd.last_ds_msg;
}

void mech_cocoon_integrity_set(Mech *mech, int integrity) {
  mech->rd.cocoon = integrity;
}

void mech_landed_set(Mech *mech, bool landed) {
  if (landed)
    mech->rd.status |= LANDED;
  else
    mech->rd.status &= ~LANDED;
}

void mech_last_startup_set(Mech *mech, int timestamp) {
  mech->rd.last_startup = timestamp;
}

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

void mech_seen_count_increment(Mech *mech) { mech->rd.num_seen++; }

void mech_seen_count_reset(Mech *mech) { mech->rd.num_seen = 0; }

void mech_last_dropship_message_tick_set(Mech *mech, int tick) {
  mech->rd.last_ds_msg = tick;
}

void mech_possible_contact_count_increment(Mech *mech) { mech->rd.can_see++; }

DbRef mech_carried_dbref(const Mech *mech) { return mech->rd.carrying; }

void mech_carried_dbref_set(Mech *mech, DbRef carried) {
  mech->rd.carrying = carried;
  mech->rd.critstatus &= ~LOAD_OK;
}

bool mech_is_towed(const Mech *mech) { return (mech->rd.status & TOWED) != 0; }

bool mech_is_towable(const Mech *mech) {
  return (mech->rd.critstatus & TOWABLE) != 0;
}

void mech_towed_set(Mech *mech, bool towed) {
  if (towed)
    mech->rd.status |= TOWED;
  else
    mech->rd.status &= ~TOWED;
}

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

void mech_jump_complete(Mech *mech) {
  mech->rd.status &= ~(JUMPING | DFA_ATTACK);
  mech->rd.dfatarget = -1;
  mech->rd.goingx = 0;
  mech->rd.goingy = 0;
  mech->rd.speed = 0.0F;
}

void mech_jump_abort(Mech *mech) { mech->rd.status &= ~(JUMPING | DFA_ATTACK); }

time_t mech_spin_start_tick(const Mech *mech) { return mech->rd.sspin; }

int mech_reactor_instability_start_tick(const Mech *mech) {
  return mech->rd.boom_start;
}

void mech_reactor_instability_start_tick_set(Mech *mech, int tick) {
  mech->rd.boom_start = tick;
}

void mech_spin_start_tick_set(Mech *mech, time_t tick) {
  mech->rd.sspin = tick;
}
