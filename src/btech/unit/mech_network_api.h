#pragma once

#include "mech_api_types.h"

int mech_c3_network_size(const Mech *mech);
DbRef mech_c3_network_node(const Mech *mech, int index);
int mech_c3i_network_size(const Mech *mech);
void mech_c3i_network_size_set(Mech *mech, int size);
DbRef mech_c3i_network_node(const Mech *mech, int index);
void mech_c3i_network_node_set(Mech *mech, int index, DbRef node);
DbRef mech_tag_target_dbref(const Mech *mech);
DbRef mech_tagged_by_dbref(const Mech *mech);
void mech_tag_target_dbref_set(Mech *mech, DbRef target);
void mech_tagged_by_dbref_set(Mech *mech, DbRef tagger);
