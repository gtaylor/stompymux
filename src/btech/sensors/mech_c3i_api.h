
/*
   p.mech.c3i.h
*/

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct Mech Mech;

void mech_c3i_join_leave(DbRef player, void *data, char *buffer);
void mech_c3i_message(DbRef player, Mech *mech, char *buffer);
void mech_c3i_targets(DbRef player, Mech *mech, char *buffer);
void mech_c3i_network(DbRef player, Mech *mech, char *buffer);
int mech_c3i_free_network_position(const MechNetworkLink *link);
void mech_c3i_network_replicate(Mech *source, Mech *destination);
void mech_c3i_network_validate(Mech *mech);
void mech_c3i_network_clear(Mech *mech, int clear_from_others);
void mech_c3i_network_remove_reference(DbRef reference, Mech *mech);
void mech_c3i_network_add(Mech *mech, Mech *unit_to_add);
