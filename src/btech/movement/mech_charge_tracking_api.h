#pragma once

typedef struct Mech Mech;

void mech_charge_timeout_update(Mech *mech);
void mech_charge_distance_record(Mech *mech, float delta_x, float delta_y);
void mech_charge_impact_resolve(Mech *mech);
