#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

bool mech_is_started(const Mech *mech);
bool mech_is_destroyed(const Mech *mech);
bool mech_is_landed(const Mech *mech);
bool mech_is_jumping(const Mech *mech);
bool mech_is_out_of_control(const Mech *mech);
bool mech_is_blinded(const Mech *mech);
bool mech_has_fired_recently(const Mech *mech);
bool mech_is_invisible(const Mech *mech);
bool mech_autocon_when_shutdown(const Mech *mech);
bool mech_autocon_include_shutdown_targets(const Mech *mech);
bool mech_suppresses_gunnery_experience(const Mech *mech);
bool mech_player_character_initialization_begin(Mech *mech);
bool mech_pilot_is_unconscious(const Mech *mech);
bool mech_is_observer(const Mech *mech);
bool mech_is_under_gravity(const Mech *mech);
bool mech_is_under_special_conditions(const Mech *mech);
bool mech_has_destroyed_gyro(const Mech *mech);
bool mech_has_damaged_gyro(const Mech *mech);
int mech_cocoon_integrity(const Mech *mech);
int mech_seen_count(const Mech *mech);
int mech_last_dropship_message_tick(const Mech *mech);
void mech_cocoon_integrity_set(Mech *mech, int integrity);
void mech_landed_set(Mech *mech, bool landed);
void mech_last_startup_set(Mech *mech, int timestamp);
void mech_movement_stop(Mech *mech);
void mech_last_use_reset(Mech *mech);
DbRef mech_autopilot_dbref(const Mech *mech);
void mech_autopilot_dbref_set(Mech *mech, DbRef autopilot);
void mech_seen_count_decrement(Mech *mech);
void mech_seen_count_increment(Mech *mech);
void mech_seen_count_reset(Mech *mech);
void mech_last_dropship_message_tick_set(Mech *mech, int tick);
void mech_possible_contact_count_increment(Mech *mech);
void mech_blinded_set(Mech *mech, bool blinded);
void mech_fired_recently_set(Mech *mech, bool fired);
DbRef mech_carried_dbref(const Mech *mech);
void mech_carried_dbref_set(Mech *mech, DbRef carried);
bool mech_is_towed(const Mech *mech);
bool mech_is_towable(const Mech *mech);
void mech_towed_set(Mech *mech, bool towed);
void mech_towed_clear(Mech *mech);
void mech_environment_conditions_set(Mech *mech, bool special, bool temperature,
                                     bool gravity, bool vacuum);
void mech_jump_complete(Mech *mech);
void mech_jump_abort(Mech *mech);
int mech_spin_start_tick(const Mech *mech);
void mech_spin_start_tick_set(Mech *mech, int tick);
