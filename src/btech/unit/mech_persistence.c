#include "mech_persistence.h"

#include <stdlib.h>
#include <string.h>

#include "equipment_types.h"
#include "mech_internal.h"
#include "mech_state_types.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"

static struct MechSection *persistence_section(Mech *mech, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(mech->ud.sections, NUM_SECTIONS,
                            sizeof(*mech->ud.sections), (size_t)index);
}

typedef struct PersistenceCriticalAccess {
  Mech *mech;
  int section;
  int slot;
} PersistenceCriticalAccess;

static struct CriticalSlot *
persistence_critical(const PersistenceCriticalAccess *access) {
  if (access->slot < 0)
    abort();
  struct MechSection *section_data =
      persistence_section(access->mech, access->section);
  return checked_storage_at(section_data->criticals, NUM_CRITICALS,
                            sizeof(*section_data->criticals),
                            (size_t)access->slot);
}

static DbRef *persistence_dbref(DbRef *values, size_t count, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(values, count, sizeof(*values), (size_t)index);
}

static int *persistence_int(int *values, size_t count, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(values, count, sizeof(*values), (size_t)index);
}

void mech_persistence_snapshot_export(const Mech *mech,
                                      MechPersistenceSnapshot *snapshot) {
  snapshot->id[0] = mech->ID[0];
  snapshot->id[1] = mech->ID[1];
  snapshot->brief = mech->brief;
  snapshot->map_number = mech->mapnumber;
  snapshot->map_dbref = mech->mapindex;
  memcpy(snapshot->channel_titles, mech->chantitle,
         sizeof(snapshot->channel_titles));
  memcpy(snapshot->tics, mech->tic, sizeof(snapshot->tics));
  memcpy(snapshot->frequencies, mech->freq, sizeof(snapshot->frequencies));
  memcpy(snapshot->frequency_modes, mech->freqmodes,
         sizeof(snapshot->frequency_modes));
  snapshot->definition = mech->ud;
  snapshot->position = mech->pd;
  snapshot->runtime = mech->rd;
  snapshot->network = mech->sd;
}

void mech_persistence_identity_restore(
    Mech *mech, const MechPersistenceSnapshot *snapshot) {
  mech->ID[0] = snapshot->id[0];
  mech->ID[1] = snapshot->id[1];
  mech->brief = snapshot->brief;
  mech->mapnumber = snapshot->map_number;
  mech->mapindex = snapshot->map_dbref;
  mech->ud = snapshot->definition;
}

void mech_persistence_section_restore(Mech *mech, int section_index,
                                      const struct MechSection *section) {
  *persistence_section(mech, section_index) = *section;
}

void mech_persistence_critical_restore(Mech *mech, int section_index, int slot,
                                       const struct CriticalSlot *critical) {
  *persistence_critical(&(PersistenceCriticalAccess){
      .mech = mech, .section = section_index, .slot = slot}) = *critical;
}

void mech_persistence_position_restore(
    Mech *mech, const MechPersistenceSnapshot *snapshot) {
  mech->pd = snapshot->position;
  memcpy(mech->chantitle, snapshot->channel_titles, sizeof(mech->chantitle));
  memcpy(mech->tic, snapshot->tics, sizeof(mech->tic));
  memcpy(mech->freq, snapshot->frequencies, sizeof(mech->freq));
  memcpy(mech->freqmodes, snapshot->frequency_modes, sizeof(mech->freqmodes));
}

void mech_persistence_bay_restore(Mech *mech, int bay_index, DbRef bay_dbref) {
  *persistence_dbref(mech->pd.bay, NUM_BAYS, bay_index) = bay_dbref;
}

void mech_persistence_turret_restore(Mech *mech, int turret_index,
                                     DbRef turret_dbref) {
  *persistence_dbref(mech->pd.turret, NUM_TURRETS, turret_index) = turret_dbref;
}

void mech_persistence_network_restore(Mech *mech,
                                      const MechPersistenceSnapshot *snapshot) {
  mech->sd = snapshot->network;
}

void mech_persistence_network_node_restore(
    const MechNetworkNodeRestore *request) {
  if (request->network_type == 0)
    *persistence_dbref(request->mech->sd.C3iNetwork, C3I_NETWORK_SIZE,
                       request->node_index) = request->node_dbref;
  else
    *persistence_dbref(request->mech->sd.C3Network, C3_NETWORK_SIZE,
                       request->node_index) = request->node_dbref;
}

void mech_persistence_tic_restore(const MechTicWordRestore *request) {
  if (request->tic_index < 0 || request->word_index < 0)
    abort();
  unsigned long (*tic_row)[TICLONGS] = checked_storage_at(
      request->mech->tic, NUM_TICS, sizeof(*request->mech->tic),
      (size_t)request->tic_index);
  unsigned long *word = checked_storage_at(
      *tic_row, TICLONGS, sizeof(**tic_row), (size_t)request->word_index);
  *word = request->value;
}

void mech_persistence_frequency_restore(const MechFrequencyRestore *request) {
  *persistence_int(request->mech->freq, FREQS, request->index) =
      request->frequency;
  *persistence_int(request->mech->freqmodes, FREQS, request->index) =
      request->mode;
  if (request->index < 0)
    abort();
  char (*channel_title)[CHTITLELEN + 1] = checked_storage_at(
      request->mech->chantitle, FREQS, sizeof(*request->mech->chantitle),
      (size_t)request->index);
  memcpy(*channel_title, request->title, sizeof(*channel_title));
}

void mech_persistence_runtime_restore(Mech *mech,
                                      const MechPersistenceSnapshot *snapshot) {
  MechDamageRecord *damage_history = mech->rd.staggerDamageList;

  mech->rd = snapshot->runtime;
  mech->rd.staggerDamageList = damage_history;
}
