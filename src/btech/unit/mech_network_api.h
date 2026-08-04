#pragma once

#include "mech_api_types.h"

int mech_c3_network_size(const Mech *mech);
int mech_c3i_network_size(const Mech *mech);
DbRef mech_tag_target_dbref(const Mech *mech);
DbRef mech_tagged_by_dbref(const Mech *mech);
void mech_tag_target_dbref_set(Mech *mech, DbRef target);
void mech_tagged_by_dbref_set(Mech *mech, DbRef tagger);
