/* State transitions for a BTech unit's lifecycle. */

#pragma once

#include <stddef.h>

typedef struct Mech Mech;
typedef struct BtechContext BtechContext;

size_t mech_storage_size(void);
Mech *mech_temporary_create(BtechContext *context);
void mech_temporary_destroy(Mech *mech);
void mech_power_up(Mech *mech);
void mech_power_down(Mech *mech);
void mech_mark_destroyed(Mech *mech);
void mech_destroy_and_place(Mech *mech);
bool mech_has_pilot(const Mech *mech);
bool mech_has_active_pilot(const Mech *mech);
bool mech_has_gunner(const Mech *mech);
bool mech_has_active_gunner(const Mech *mech);
void mech_max_speed_set(Mech *mech, float speed);
void mech_max_speed_lower(Mech *mech, float amount);
void mech_max_speed_divide(Mech *mech, float divisor);
bool mech_can_jump(const Mech *mech);
void mech_maybe_move(Mech *mech);
void mech_update_recycling(Mech *mech);
void mech_set_recycle_part(Mech *mech, int section, int critical, int value);
void mech_set_recycle_limb(Mech *mech, int section, int value);
void mech_make_fall(Mech *mech);
void mech_make_stand(Mech *mech);
void mech_start_seeing(Mech *mech);
void mech_continue_flying(Mech *mech);
void mech_drop_club(Mech *mech);
bool mech_aero_has_free_fuel(const Mech *mech);
void mech_template_state_reset(Mech *mech);
void mech_communications_clear(Mech *mech);
