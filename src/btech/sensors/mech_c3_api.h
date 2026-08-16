
/* p.mech.c3.h */

#pragma once

#include <stdbool.h>

#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct Mech Mech;

int mech_c3_master_slot_count(const Mech *mech);
bool mech_c3_master_slot_is_working(Mech *mech,
                                    CriticalSlotReference reference);
int mech_c3_working_master_count(Mech *mech);
int mech_c3_total_master_count(Mech *mech);
int mech_c3_maximum_network_size(Mech *mech, const DbRef *network,
                                 int network_size, Mech *targ_mech);
int mech_c3_network_trim(Mech *mech, DbRef *network, int network_size);
int mech_c3_free_network_position(const MechNetworkLink *link);
void mech_c3_network_replicate(Mech *mech_src, Mech *mech_dest);
void mech_c3_network_add(Mech *mech, Mech *mech_to_add);
void mech_c3_network_remove_reference(DbRef ref_to_clear, Mech *mech);
void mech_c3_network_clear(Mech *mech, bool clear_from_others);
void mech_c3_network_validate(Mech *mech);
void mech_c3_join_leave(DbRef player, Mech *mech, char *buffer);
void mech_c3_message(DbRef player, Mech *mech, char *buffer);
void mech_c3_targets(DbRef player, Mech *mech, char *buffer);
void mech_c3_network(DbRef player, Mech *mech, char *buffer);
