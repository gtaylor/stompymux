/* configuration_internal.h - Private typed BTech configuration storage. */

#pragma once

#include "btech/configuration.h"
#include "mux/server/platform.h"
#include "mux/support/red_black_tree.h"

typedef struct BtechConfigurationEntry {
  DbRef object;
  char preferred_id[3];
  char *display_name;
  char *markings;
  DbRef assigned_pilot;

  bool has_ui_preferences;
  BtechPlayerUiPreferences ui_preferences;
  char *mechwarrior_template;
  bool has_loadout;
  BtechPersonalCombatLoadout loadout;
  time_t technician_available_at;

  bool has_cargo_transfer_point;
  BtechCargoTransferPoint cargo_transfer_point;
  bool has_map_link;
  BtechMapLink map_link;
} BtechConfigurationEntry;

void btech_configuration_initialize(BtechContext *context);
void btech_configuration_destroy(BtechContext *context);
BtechConfigurationEntry *btech_configuration_entry(BtechContext *context,
                                                   DbRef object, bool create);
bool btech_configuration_entry_is_empty(const BtechConfigurationEntry *entry);
