/* State transitions for a BTech unit's lifecycle. */

#pragma once

typedef struct mech_data MECH;

void mech_power_up(MECH *mech);
void mech_power_down(MECH *mech);
void mech_mark_destroyed(MECH *mech);
void mech_destroy_and_place(MECH *mech);
bool mech_has_pilot(const MECH *mech);
bool mech_has_active_pilot(const MECH *mech);
bool mech_has_gunner(const MECH *mech);
bool mech_has_active_gunner(const MECH *mech);
void mech_max_speed_set(MECH *mech, float speed);
void mech_max_speed_lower(MECH *mech, float amount);
void mech_max_speed_divide(MECH *mech, float divisor);
bool mech_can_jump(const MECH *mech);
void mech_maybe_move(MECH *mech);
void mech_update_recycling(MECH *mech);
void mech_set_recycle_part(MECH *mech, int section, int critical, int value);
void mech_set_recycle_limb(MECH *mech, int section, int value);
void mech_make_fall(MECH *mech);
void mech_make_stand(MECH *mech);
void mech_start_seeing(MECH *mech);
void mech_continue_flying(MECH *mech);
void mech_drop_club(MECH *mech);
bool mech_aero_has_free_fuel(const MECH *mech);
