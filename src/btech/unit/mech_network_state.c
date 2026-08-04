#include "mech_network_api.h"

#include "mech_internal.h"

int mech_c3_network_size(const Mech *mech) { return mech->sd.wC3NetworkSize; }

int mech_c3i_network_size(const Mech *mech) { return mech->sd.wC3iNetworkSize; }

DbRef mech_tag_target_dbref(const Mech *mech) { return mech->sd.tagTarget; }

DbRef mech_tagged_by_dbref(const Mech *mech) { return mech->sd.taggedBy; }
