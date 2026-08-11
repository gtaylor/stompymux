#include "equipment_types.h"
#include "mech_network_api.h"

#include "mech_internal.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include <stddef.h>

static const DbRef *network_node(const DbRef *nodes, size_t count, int index) {
  return checked_storage_at_const(nodes, count, sizeof(*nodes), (size_t)index);
}

static DbRef *network_node_mutable(DbRef *nodes, size_t count, int index) {
  return checked_storage_at(nodes, count, sizeof(*nodes), (size_t)index);
}

int mech_c3_network_size(const Mech *mech) {
  return mech->sd.w_c3_network_size;
}

void mech_c3_network_size_set(Mech *mech, int size) {
  mech->sd.w_c3_network_size = size;
}

DbRef mech_c3_network_node(const Mech *mech, int index) {
  return *network_node(mech->sd.c3_network, C3_NETWORK_SIZE, index);
}

void mech_c3_network_node_set(Mech *mech, int index, DbRef node) {
  *network_node_mutable(mech->sd.c3_network, C3_NETWORK_SIZE, index) = node;
}

int mech_c3_working_masters(const Mech *mech) {
  return mech->sd.w_working_c3_masters;
}

void mech_c3_working_masters_set(Mech *mech, int count) {
  mech->sd.w_working_c3_masters = count;
}

int mech_c3_total_masters(const Mech *mech) {
  return mech->sd.w_total_c3_masters;
}

void mech_c3_total_masters_set(Mech *mech, int count) {
  mech->sd.w_total_c3_masters = count;
}

int mech_c3i_network_size(const Mech *mech) {
  return mech->sd.w_c3i_network_size;
}

void mech_c3i_network_size_set(Mech *mech, int size) {
  mech->sd.w_c3i_network_size = size;
}

DbRef mech_c3i_network_node(const Mech *mech, int index) {
  return *network_node(mech->sd.c3i_network, C3I_NETWORK_SIZE, index);
}

void mech_c3i_network_node_set(Mech *mech, int index, DbRef node) {
  *network_node_mutable(mech->sd.c3i_network, C3I_NETWORK_SIZE, index) = node;
}

DbRef mech_tag_target_dbref(const Mech *mech) { return mech->sd.tag_target; }

DbRef mech_tagged_by_dbref(const Mech *mech) { return mech->sd.tagged_by; }

void mech_tag_target_dbref_set(Mech *mech, DbRef target) {
  mech->sd.tag_target = target;
}

void mech_tagged_by_dbref_set(Mech *mech, DbRef tagger) {
  mech->sd.tagged_by = tagger;
}
