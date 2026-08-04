#pragma once

#include "mech_api_types.h"
#include "mech_state_types.h"

typedef struct {
  char id[2];
  char brief;
  int map_number;
  DbRef map_dbref;
  char channel_titles[FREQS][CHTITLELEN + 1];
  unsigned long tics[NUM_TICS][TICLONGS];
  int frequencies[FREQS];
  int frequency_modes[FREQS];
  MechDefinitionState definition;
  MechPositionState position;
  MechRuntimeState runtime;
  MechNetworkState network;
} MechPersistenceSnapshot;

void mech_persistence_snapshot_export(const Mech *mech,
                                      MechPersistenceSnapshot *snapshot);
void mech_persistence_identity_restore(Mech *mech,
                                       const MechPersistenceSnapshot *snapshot);
void mech_persistence_section_restore(Mech *mech, int section_index,
                                      const struct MechSection *section);
void mech_persistence_critical_restore(Mech *mech, int section_index, int slot,
                                       const struct CriticalSlot *critical);
void mech_persistence_position_restore(Mech *mech,
                                       const MechPersistenceSnapshot *snapshot);
void mech_persistence_bay_restore(Mech *mech, int bay_index, DbRef bay_dbref);
void mech_persistence_turret_restore(Mech *mech, int turret_index,
                                     DbRef turret_dbref);
void mech_persistence_network_restore(Mech *mech,
                                      const MechPersistenceSnapshot *snapshot);
void mech_persistence_network_node_restore(Mech *mech, int network_type,
                                           int node_index, DbRef node_dbref);
void mech_persistence_tic_restore(Mech *mech, int tic_index, int word_index,
                                  unsigned long value);
void mech_persistence_frequency_restore(Mech *mech, int frequency_index,
                                        int frequency, int mode,
                                        const char *title);
void mech_persistence_runtime_restore(Mech *mech,
                                      const MechPersistenceSnapshot *snapshot);
