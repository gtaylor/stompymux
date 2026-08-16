#include "mech_runtime_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"
#include "mux/server/platform.h"
#include "section_types.h"
#include <time.h>

bool mech_is_started(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_STARTED);
}

bool mech_is_destroyed(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_DESTROYED);
}

void mech_destroyed_set(Mech *mech, bool destroyed) {
  if (destroyed)
    mech_status_set(&mech->rd.status, MECH_STATUS_DESTROYED);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_DESTROYED);
}

bool mech_is_landed(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_LANDED);
}

bool mech_is_jumping(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_JUMPING);
}

bool mech_is_out_of_control(const Mech *mech) { return mech->rd.cocoon != 0; }

bool mech_is_blinded(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_BLINDED);
}

bool mech_has_fired_recently(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_FIRED);
}

bool mech_is_invisible(const Mech *mech) {
  return mech_crit_status_has(mech->rd.critstatus, MECH_CRIT_STATUS_INVISIBLE);
}

bool mech_autocon_when_shutdown(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_AUTOCON_WHEN_SHUTDOWN);
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
    mech_status_set(&mech->rd.status, MECH_STATUS_BLINDED);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_BLINDED);
}

void mech_fired_recently_set(Mech *mech, bool fired) {
  if (fired)
    mech_status_set(&mech->rd.status, MECH_STATUS_FIRED);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_FIRED);
}

bool mech_suppresses_gunnery_experience(const Mech *mech) {
  return mech_status2_has(mech->rd.status2, MECH_STATUS2_NO_GUN_XP);
}

bool mech_player_character_initialization_begin(Mech *mech) {
  if (mech->ud.type != CLASS_MW ||
      mech_crit_status_has(mech->rd.critstatus,
                           MECH_CRIT_STATUS_PC_INITIALIZED))
    return false;
  mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_PC_INITIALIZED);
  return true;
}

bool mech_pilot_is_unconscious(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_UNCONSCIOUS);
}

bool mech_is_observer(const Mech *mech) {
  return mech_crit_status_has(mech->rd.critstatus,
                              MECH_CRIT_STATUS_OBSERVATORIC);
}

bool mech_is_under_gravity(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_UNDERGRAVITY);
}

bool mech_is_under_special_conditions(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_UNDERSPECIAL);
}

bool mech_is_under_vacuum(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_UNDERVACUUM);
}

bool mech_is_immobile(const Mech *mech) {
  return (!mech_status_has(mech->rd.status, MECH_STATUS_STARTED) ||
          mech_status_has(mech->rd.status, MECH_STATUS_UNCONSCIOUS) ||
          mech_status2_has(mech->rd.status2, MECH_STATUS2_FORTIFIED) ||
          mech_status_has(mech->rd.status, MECH_STATUS_BLINDED) ||
          mech->ud.move == MOVE_NONE ||
          (mech_status_has(mech->rd.status, MECH_STATUS_FALLEN) &&
           mech->ud.type != CLASS_MECH && mech->ud.type != CLASS_MW)) != 0;
}

bool mech_has_destroyed_gyro(const Mech *mech) {
  return mech_crit_status_has(mech->rd.critstatus,
                              MECH_CRIT_STATUS_GYRO_DESTROYED);
}

bool mech_has_damaged_gyro(const Mech *mech) {
  return mech_crit_status_has(
      mech->rd.critstatus, (MechCritStatus)(MECH_CRIT_STATUS_GYRO_DAMAGED |
                                            MECH_CRIT_STATUS_GYRO_DESTROYED));
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
    mech_status_set(&mech->rd.status, MECH_STATUS_LANDED);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_LANDED);
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
  mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_LOAD_OK);
}

bool mech_is_towed(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_TOWED);
}

bool mech_is_towable(const Mech *mech) {
  return mech_crit_status_has(mech->rd.critstatus, MECH_CRIT_STATUS_TOWABLE);
}

void mech_towed_set(Mech *mech, bool towed) {
  if (towed)
    mech_status_set(&mech->rd.status, MECH_STATUS_TOWED);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_TOWED);
}

void mech_towed_clear(Mech *mech) {
  mech_status_clear(&mech->rd.status, MECH_STATUS_TOWED);
}

void mech_environment_conditions_set(Mech *mech, bool special, bool temperature,
                                     bool gravity, bool vacuum) {
  mech_status_clear(&mech->rd.status, MECH_STATUS_CONDITIONS);
  if (!special)
    return;
  mech_status_set(&mech->rd.status, MECH_STATUS_UNDERSPECIAL);
  if (temperature)
    mech_status_set(&mech->rd.status, MECH_STATUS_UNDERTEMPERATURE);
  if (gravity)
    mech_status_set(&mech->rd.status, MECH_STATUS_UNDERGRAVITY);
  if (vacuum)
    mech_status_set(&mech->rd.status, MECH_STATUS_UNDERVACUUM);
}

void mech_jump_complete(Mech *mech) {
  mech_status_clear(&mech->rd.status,
                    (MechStatus)(MECH_STATUS_JUMPING | MECH_STATUS_DFA_ATTACK));
  mech->rd.dfatarget = -1;
  mech->rd.goingx = 0;
  mech->rd.goingy = 0;
  mech->rd.speed = 0.0F;
}

void mech_jump_abort(Mech *mech) {
  mech_status_clear(&mech->rd.status,
                    (MechStatus)(MECH_STATUS_JUMPING | MECH_STATUS_DFA_ATTACK));
}

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
