/** @file
 * Typed persistent configuration owned by the BattleTech subsystem.
 */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <time.h>

#include "btech/ids.h"

typedef struct BtechContext BtechContext;

/** Building-contact inclusion policy. */
typedef enum BtechBuildingContactMode : int {
  BTECH_BUILDING_CONTACTS_FOLLOW_BRIEF,
  BTECH_BUILDING_CONTACTS_INCLUDE,
  BTECH_BUILDING_CONTACTS_EXCLUDE,
} BtechBuildingContactMode;

/** Player-owned display preferences. */
typedef struct BtechPlayerUiPreferences {
  int tactical_height;
  int tactical_width;
  int lrs_height;
  bool include_dead;
  bool include_shutdown;
  bool include_enemies;
  bool include_allies;
  bool include_target;
  BtechBuildingContactMode buildings;
} BtechPlayerUiPreferences;

/** Personal-combat equipment used when an ejected pilot is initialized. */
typedef struct BtechPersonalCombatLoadout {
  int armor_head;
  int armor_torso;
  int armor_hands;
  int armor_feet;
  char right_weapon[128];
  char left_weapon[128];
  bool has_right_ammunition;
  bool has_left_ammunition;
  int right_ammunition;
  int left_ammunition;
} BtechPersonalCombatLoadout;

/** Optional cargo-transfer point on a map. */
typedef struct BtechCargoTransferPoint {
  int x;
  int y;
  bool reveal_hint;
} BtechCargoTransferPoint;

/** One cardinal entrance into a child map. */
typedef enum BtechMapEntranceMode : int {
  BTECH_MAP_ENTRANCE_NONE,
  BTECH_MAP_ENTRANCE_OFFSET,
  BTECH_MAP_ENTRANCE_EXACT,
} BtechMapEntranceMode;

typedef struct BtechMapEntrance {
  BtechMapEntranceMode mode;
  int x;
  int y;
  int offset;
} BtechMapEntrance;

/** Parent placement and entrances for one child map. */
typedef struct BtechMapLink {
  BtechObjectId parent;
  int x;
  int y;
  BtechMapEntrance entrances[4];
} BtechMapLink;

/** Callback used to visit configured child-to-parent map links. */
typedef bool (*BtechMapLinkVisitor)(BtechObjectId child,
                                    const BtechMapLink *link, void *context);

/** Returns whether coordinates identify a hex on a registered map. */
bool btech_map_coordinate_is_valid(BtechContext *context, BtechObjectId map,
                                   int x, int y);

/** Returns a unit's preferred two-letter map identifier, or an empty string. */
const char *btech_unit_preferred_id(BtechContext *context, BtechObjectId unit);
/** Sets or clears a unit's validated preferred map identifier. */
bool btech_unit_preferred_id_set(BtechContext *context, BtechObjectId unit,
                                 const char *preferred_id);
/** Returns a unit display-name override, or an empty string. */
const char *btech_unit_display_name(BtechContext *context, BtechObjectId unit);
/** Sets or clears a unit display-name override. */
bool btech_unit_display_name_set(BtechContext *context, BtechObjectId unit,
                                 const char *name);
/** Returns unit markings, or an empty string. */
const char *btech_unit_markings(BtechContext *context, BtechObjectId unit);
/** Sets or clears unit markings. */
bool btech_unit_markings_set(BtechContext *context, BtechObjectId unit,
                             const char *markings);
/** Returns the assigned pilot, or -1 when none is assigned. */
BtechObjectId btech_unit_assigned_pilot(BtechContext *context,
                                        BtechObjectId unit);
/** Sets or clears the assigned pilot. */
bool btech_unit_assigned_pilot_set(BtechContext *context, BtechObjectId unit,
                                   BtechObjectId pilot);

/** Returns player UI preferences, filling documented defaults when absent. */
BtechPlayerUiPreferences btech_player_ui_preferences(BtechContext *context,
                                                     BtechObjectId player);
/** Returns whether player UI preferences are explicitly configured. */
bool btech_player_ui_preferences_configured(BtechContext *context,
                                            BtechObjectId player);
/** Atomically validates and stores player UI preferences. */
bool btech_player_ui_preferences_set(BtechContext *context,
                                     BtechObjectId player,
                                     BtechPlayerUiPreferences preferences);
/** Clears stored UI preferences so defaults apply. */
void btech_player_ui_preferences_clear(BtechContext *context,
                                       BtechObjectId player);

/** Returns the player's MechWarrior template, defaulting to MechWarrior. */
const char *btech_player_mechwarrior_template(BtechContext *context,
                                              BtechObjectId player);
/** Returns whether a MechWarrior template override is configured. */
bool btech_player_mechwarrior_template_configured(BtechContext *context,
                                                  BtechObjectId player);
/** Sets or clears a player's MechWarrior template reference. */
bool btech_player_mechwarrior_template_set(BtechContext *context,
                                           BtechObjectId player,
                                           const char *reference);
/** Returns whether a personal-combat loadout is configured. */
bool btech_player_loadout(BtechContext *context, BtechObjectId player,
                          BtechPersonalCombatLoadout *loadout);
/** Atomically validates and stores a personal-combat loadout. */
bool btech_player_loadout_set(BtechContext *context, BtechObjectId player,
                              const BtechPersonalCombatLoadout *loadout);
/** Clears a player's personal-combat loadout. */
void btech_player_loadout_clear(BtechContext *context, BtechObjectId player);

/** Returns a technician's UTC availability timestamp, or zero. */
time_t btech_repair_technician_available_at(BtechContext *context,
                                            BtechObjectId player);
/** Stores or clears a technician's UTC availability timestamp. */
bool btech_repair_technician_available_at_set(BtechContext *context,
                                              BtechObjectId player,
                                              time_t available_at);

/** Returns whether a map has a configured cargo-transfer point. */
bool btech_map_cargo_transfer_point(BtechContext *context, BtechObjectId map,
                                    BtechCargoTransferPoint *point);
/** Stores or clears a map cargo-transfer point. */
bool btech_map_cargo_transfer_point_set(BtechContext *context,
                                        BtechObjectId map,
                                        const BtechCargoTransferPoint *point);
/** Returns whether a child map has a parent link. */
bool btech_map_link(BtechContext *context, BtechObjectId child,
                    BtechMapLink *link);
/** Validates and stores a child map's parent link. */
bool btech_map_link_set(BtechContext *context, BtechObjectId child,
                        const BtechMapLink *link);
/** Removes a child map's parent link. */
void btech_map_link_clear(BtechContext *context, BtechObjectId child);
/** Visits configured map links in ascending child-dbref order. */
void btech_map_links_visit(BtechContext *context, BtechMapLinkVisitor visitor,
                           void *visitor_context);

/** Removes every typed BTech configuration reference to an object. */
void btech_configuration_forget(BtechContext *context, BtechObjectId object);
