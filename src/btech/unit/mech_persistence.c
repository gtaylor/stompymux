#include "mech_persistence.h"

#include <stdlib.h>
#include <string.h>

#include "mech_internal.h"

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
  mech->ud.sections[section_index] = *section;
}

void mech_persistence_critical_restore(Mech *mech, int section_index, int slot,
                                       const struct CriticalSlot *critical) {
  mech->ud.sections[section_index].criticals[slot] = *critical;
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
  mech->pd.bay[bay_index] = bay_dbref;
}

void mech_persistence_turret_restore(Mech *mech, int turret_index,
                                     DbRef turret_dbref) {
  mech->pd.turret[turret_index] = turret_dbref;
}

void mech_persistence_network_restore(Mech *mech,
                                      const MechPersistenceSnapshot *snapshot) {
  mech->sd = snapshot->network;
}

void mech_persistence_network_node_restore(Mech *mech, int network_type,
                                           int node_index, DbRef node_dbref) {
  if (network_type == 0)
    mech->sd.C3iNetwork[node_index] = node_dbref;
  else
    mech->sd.C3Network[node_index] = node_dbref;
}

void mech_persistence_tic_restore(Mech *mech, int tic_index, int word_index,
                                  unsigned long value) {
  mech->tic[tic_index][word_index] = value;
}

void mech_persistence_frequency_restore(Mech *mech, int frequency_index,
                                        int frequency, int mode,
                                        const char *title) {
  mech->freq[frequency_index] = frequency;
  mech->freqmodes[frequency_index] = mode;
  memcpy(mech->chantitle[frequency_index], title,
         sizeof(mech->chantitle[frequency_index]));
}

void mech_persistence_runtime_restore(Mech *mech,
                                      const MechPersistenceSnapshot *snapshot) {
  MechDamageRecord *damage_history = mech->rd.staggerDamageList;

  mech->rd = snapshot->runtime;
  mech->rd.staggerDamageList = damage_history;
}

bool mech_persistence_damage_history_is_empty(const Mech *mech) {
  return mech->rd.staggerDamageList == nullptr;
}

bool mech_persistence_damage_append(Mech *mech, int amount, time_t occurred_at,
                                    DbRef attacker, bool counted) {
  MechDamageRecord **link = &mech->rd.staggerDamageList;
  MechDamageRecord *record;

  while (*link)
    link = &(*link)->next;
  record = calloc(1, sizeof(*record));
  if (!record)
    return false;
  record->amount = amount;
  record->occuredAt = occurred_at;
  record->attackerNum = attacker;
  record->counted = counted;
  *link = record;
  return true;
}
