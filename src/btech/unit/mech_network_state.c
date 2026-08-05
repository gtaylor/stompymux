#include "mech_network_api.h"

#include "mech_internal.h"

int mech_c3_network_size(const Mech *mech) { return mech->sd.wC3NetworkSize; }

DbRef mech_c3_network_node(const Mech *mech, int index) {
  return mech->sd.C3Network[index];
}

int mech_c3i_network_size(const Mech *mech) { return mech->sd.wC3iNetworkSize; }

void mech_c3i_network_size_set(Mech *mech, int size) {
  mech->sd.wC3iNetworkSize = size;
}

DbRef mech_c3i_network_node(const Mech *mech, int index) {
  return mech->sd.C3iNetwork[index];
}

void mech_c3i_network_node_set(Mech *mech, int index, DbRef node) {
  mech->sd.C3iNetwork[index] = node;
}

DbRef mech_tag_target_dbref(const Mech *mech) { return mech->sd.tagTarget; }

DbRef mech_tagged_by_dbref(const Mech *mech) { return mech->sd.taggedBy; }

void mech_tag_target_dbref_set(Mech *mech, DbRef target) {
  mech->sd.tagTarget = target;
}

void mech_tagged_by_dbref_set(Mech *mech, DbRef tagger) {
  mech->sd.taggedBy = tagger;
}
