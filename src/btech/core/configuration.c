/* configuration.c - Typed sparse BTech configuration ownership. */

#include "btech/configuration.h"
#include "btech/ids.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "configuration_internal.h"
#include "context_internal.h" // IWYU pragma: keep
#include "map.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

static int compare_dbrefs(const RedBlackTreeCompareCall *call) {
  const intptr_t LEFT = *(const intptr_t *)call->lhs;
  const intptr_t RIGHT = *(const intptr_t *)call->rhs;
  if (LEFT < RIGHT)
    return -1;
  return LEFT > RIGHT ? 1 : 0;
}

static void configuration_entry_free(BtechConfigurationEntry *entry) {
  free(entry->display_name);
  free(entry->markings);
  free(entry->mechwarrior_template);
  free(entry);
}

static void configuration_entry_release(const RedBlackTreeReleaseCall *call) {
  configuration_entry_free(call->data);
}

void btech_configuration_initialize(BtechContext *context) {
  context->configurations = red_black_tree_init(compare_dbrefs, nullptr);
  if (context->configurations == nullptr)
    abort();
}

void btech_configuration_destroy(BtechContext *context) {
  if (context->configurations == nullptr)
    return;
  red_black_tree_release(context->configurations, configuration_entry_release,
                         nullptr);
  context->configurations = nullptr;
}

BtechConfigurationEntry *btech_configuration_entry(BtechContext *context,
                                                   DbRef object, bool create) {
  if (context == nullptr || context->configurations == nullptr)
    return nullptr;
  BtechConfigurationEntry *entry =
      red_black_tree_find_integer(context->configurations, object);
  if (entry != nullptr || !create)
    return entry;
  entry = checked_storage_try_allocate_array(1, sizeof(*entry));
  if (entry == nullptr)
    return nullptr;
  *entry =
      (BtechConfigurationEntry){.object = object, .assigned_pilot = NOTHING};
  red_black_tree_insert_integer(context->configurations, object, entry);
  return entry;
}

bool btech_configuration_entry_is_empty(const BtechConfigurationEntry *entry) {
  return (entry->preferred_id[0] == '\0' && entry->display_name == nullptr &&
          entry->markings == nullptr && entry->assigned_pilot == NOTHING &&
          !entry->has_ui_preferences &&
          entry->mechwarrior_template == nullptr && !entry->has_loadout &&
          entry->technician_available_at == 0 &&
          !entry->has_cargo_transfer_point && !entry->has_map_link) != 0;
}

static void configuration_entry_prune(BtechContext *context,
                                      BtechConfigurationEntry *entry) {
  if (!btech_configuration_entry_is_empty(entry))
    return;
  BtechConfigurationEntry *removed =
      red_black_tree_delete_integer(context->configurations, entry->object);
  if (removed != nullptr)
    configuration_entry_free(removed);
}

static bool replace_text(char **destination, const char *source,
                         size_t maximum) {
  char *copy = nullptr;
  if (source != nullptr && *source != '\0') {
    const size_t LENGTH = strlen(source);
    if (LENGTH > maximum)
      return false;
    copy = strdup(source);
    if (copy == nullptr)
      return false;
  }
  free(*destination);
  *destination = copy;
  return true;
}

const char *btech_unit_preferred_id(BtechContext *context, BtechObjectId unit) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)unit, false);
  return entry == nullptr ? "" : entry->preferred_id;
}

bool btech_unit_preferred_id_set(BtechContext *context, BtechObjectId unit,
                                 const char *preferred_id) {
  char first = '\0';
  char second = '\0';
  if (preferred_id != nullptr && *preferred_id != '\0') {
    if (strlen(preferred_id) != 2)
      return false;
    first = *checked_string_suffix(preferred_id, 0);
    second = *checked_string_suffix(preferred_id, 1);
    if (!((first >= 'A' && first <= 'Z') || (first >= 'a' && first <= 'z')) ||
        !((second >= 'A' && second <= 'Z') || (second >= 'a' && second <= 'z')))
      return false;
  }
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)unit, true);
  if (entry == nullptr)
    return false;
  if (preferred_id == nullptr || *preferred_id == '\0') {
    entry->preferred_id[0] = '\0';
  } else {
    entry->preferred_id[0] = ascii_to_upper(first);
    entry->preferred_id[1] = ascii_to_upper(second);
    entry->preferred_id[2] = '\0';
  }
  configuration_entry_prune(context, entry);
  return true;
}

const char *btech_unit_display_name(BtechContext *context, BtechObjectId unit) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)unit, false);
  return entry == nullptr || entry->display_name == nullptr
             ? ""
             : entry->display_name;
}

bool btech_unit_display_name_set(BtechContext *context, BtechObjectId unit,
                                 const char *name) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)unit, true);
  if (entry == nullptr || !replace_text(&entry->display_name, name, 120))
    return false;
  configuration_entry_prune(context, entry);
  return true;
}

const char *btech_unit_markings(BtechContext *context, BtechObjectId unit) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)unit, false);
  return entry == nullptr || entry->markings == nullptr ? "" : entry->markings;
}

bool btech_unit_markings_set(BtechContext *context, BtechObjectId unit,
                             const char *markings) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)unit, true);
  if (entry == nullptr ||
      !replace_text(&entry->markings, markings, LBUF_SIZE - 1))
    return false;
  configuration_entry_prune(context, entry);
  return true;
}

BtechObjectId btech_unit_assigned_pilot(BtechContext *context,
                                        BtechObjectId unit) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)unit, false);
  return entry == nullptr ? NOTHING : entry->assigned_pilot;
}

// NOLINTNEXTLINE(bugprone-easily-swappable-parameters)
bool btech_unit_assigned_pilot_set(BtechContext *context, BtechObjectId unit,
                                   BtechObjectId pilot) {
  if (pilot != NOTHING && (!is_good_obj(context->database, (DbRef)pilot) ||
                           !is_player(context->database, (DbRef)pilot) ||
                           is_going(context->database, (DbRef)pilot)))
    return false;
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)unit, true);
  if (entry == nullptr)
    return false;
  entry->assigned_pilot = (DbRef)pilot;
  configuration_entry_prune(context, entry);
  return true;
}

static BtechPlayerUiPreferences default_ui_preferences(void) {
  return (BtechPlayerUiPreferences){
      .tactical_height = 14,
      .tactical_width = 21,
      .lrs_height = 11,
      .include_shutdown = true,
      .include_enemies = true,
      .include_allies = true,
      .include_target = true,
      .buildings = BTECH_BUILDING_CONTACTS_EXCLUDE,
  };
}

BtechPlayerUiPreferences btech_player_ui_preferences(BtechContext *context,
                                                     BtechObjectId player) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, false);
  return entry == nullptr || !entry->has_ui_preferences
             ? default_ui_preferences()
             : entry->ui_preferences;
}

bool btech_player_ui_preferences_configured(BtechContext *context,
                                            BtechObjectId player) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, false);
  return (entry != nullptr && entry->has_ui_preferences) != 0;
}

bool btech_player_ui_preferences_set(BtechContext *context,
                                     BtechObjectId player,
                                     BtechPlayerUiPreferences preferences) {
  if (preferences.tactical_height < 5 || preferences.tactical_height > 24 ||
      preferences.tactical_width < 5 || preferences.tactical_width > 40 ||
      preferences.lrs_height < 10 || preferences.lrs_height > 40 ||
      preferences.buildings < BTECH_BUILDING_CONTACTS_FOLLOW_BRIEF ||
      preferences.buildings > BTECH_BUILDING_CONTACTS_EXCLUDE)
    return false;
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, true);
  if (entry == nullptr)
    return false;
  entry->has_ui_preferences = true;
  entry->ui_preferences = preferences;
  return true;
}

void btech_player_ui_preferences_clear(BtechContext *context,
                                       BtechObjectId player) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, false);
  if (entry == nullptr)
    return;
  entry->has_ui_preferences = false;
  entry->ui_preferences = (BtechPlayerUiPreferences){};
  configuration_entry_prune(context, entry);
}

const char *btech_player_mechwarrior_template(BtechContext *context,
                                              BtechObjectId player) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, false);
  return entry == nullptr || entry->mechwarrior_template == nullptr
             ? "MechWarrior"
             : entry->mechwarrior_template;
}

bool btech_player_mechwarrior_template_configured(BtechContext *context,
                                                  BtechObjectId player) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, false);
  return (entry != nullptr && entry->mechwarrior_template != nullptr) != 0;
}

bool btech_player_mechwarrior_template_set(BtechContext *context,
                                           BtechObjectId player,
                                           const char *reference) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, true);
  if (entry == nullptr ||
      !replace_text(&entry->mechwarrior_template, reference, 24))
    return false;
  configuration_entry_prune(context, entry);
  return true;
}

bool btech_player_loadout(BtechContext *context, BtechObjectId player,
                          BtechPersonalCombatLoadout *loadout) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, false);
  if (entry == nullptr || !entry->has_loadout)
    return false;
  if (loadout != nullptr)
    *loadout = entry->loadout;
  return true;
}

bool btech_player_loadout_set(BtechContext *context, BtechObjectId player,
                              const BtechPersonalCombatLoadout *loadout) {
  if (loadout == nullptr || loadout->armor_head < 0 ||
      loadout->armor_head > 2 || loadout->armor_torso < 0 ||
      loadout->armor_torso > 8 || loadout->armor_hands < 0 ||
      loadout->armor_hands > 2 || loadout->armor_feet < 0 ||
      loadout->armor_feet > 2 || loadout->right_ammunition < 0 ||
      loadout->right_ammunition > 255 || loadout->left_ammunition < 0 ||
      loadout->left_ammunition > 255)
    return false;
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, true);
  if (entry == nullptr)
    return false;
  entry->loadout = *loadout;
  entry->has_loadout = true;
  return true;
}

void btech_player_loadout_clear(BtechContext *context, BtechObjectId player) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, false);
  if (entry == nullptr)
    return;
  entry->has_loadout = false;
  entry->loadout = (BtechPersonalCombatLoadout){};
  configuration_entry_prune(context, entry);
}

time_t btech_repair_technician_available_at(BtechContext *context,
                                            BtechObjectId player) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, false);
  return entry == nullptr ? 0 : entry->technician_available_at;
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters)
bool btech_repair_technician_available_at_set(BtechContext *context,
                                              BtechObjectId player,
                                              time_t available_at) {
  if (available_at < 0)
    return false;
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)player, true);
  if (entry == nullptr)
    return false;
  entry->technician_available_at = available_at;
  configuration_entry_prune(context, entry);
  return true;
}
// NOLINTEND(bugprone-easily-swappable-parameters)

bool btech_map_cargo_transfer_point(BtechContext *context, BtechObjectId map,
                                    BtechCargoTransferPoint *point) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)map, false);
  if (entry == nullptr || !entry->has_cargo_transfer_point)
    return false;
  if (point != nullptr)
    *point = entry->cargo_transfer_point;
  return true;
}

// NOLINTNEXTLINE(bugprone-easily-swappable-parameters): Cartesian coordinates.
bool btech_map_coordinate_is_valid(BtechContext *context, BtechObjectId map,
                                   int x, int y) {
  BattleMap *battle_map = btech_context_get_map(context, (DbRef)map);
  return (battle_map != nullptr && x >= 0 && y >= 0 &&
          x < battle_map->map_width && y < battle_map->map_height) != 0;
}

bool btech_map_cargo_transfer_point_set(BtechContext *context,
                                        BtechObjectId map,
                                        const BtechCargoTransferPoint *point) {
  if (point != nullptr &&
      !btech_map_coordinate_is_valid(context, map, point->x, point->y))
    return false;
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)map, point != nullptr);
  if (entry == nullptr)
    return point == nullptr;
  entry->has_cargo_transfer_point = point != nullptr;
  if (point != nullptr)
    entry->cargo_transfer_point = *point;
  else
    entry->cargo_transfer_point = (BtechCargoTransferPoint){};
  configuration_entry_prune(context, entry);
  return true;
}

bool btech_map_link(BtechContext *context, BtechObjectId child,
                    BtechMapLink *link) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)child, false);
  if (entry == nullptr || !entry->has_map_link)
    return false;
  if (link != nullptr)
    *link = entry->map_link;
  return true;
}

bool btech_map_link_set(BtechContext *context, BtechObjectId child,
                        const BtechMapLink *link) {
  BattleMap *child_map = btech_context_get_map(context, (DbRef)child);
  BattleMap *parent_map =
      link == nullptr ? nullptr
                      : btech_context_get_map(context, (DbRef)link->parent);
  if (link == nullptr || child == link->parent || child_map == nullptr ||
      parent_map == nullptr || link->x < 0 || link->y < 0 ||
      link->x >= parent_map->map_width || link->y >= parent_map->map_height)
    return false;
  for (size_t index = 0; index < 4; index++) {
    const BtechMapEntrance *entrance = checked_storage_at_const(
        link->entrances, 4, sizeof(*link->entrances), index);
    if (entrance->mode < BTECH_MAP_ENTRANCE_NONE ||
        entrance->mode > BTECH_MAP_ENTRANCE_EXACT ||
        (entrance->mode == BTECH_MAP_ENTRANCE_OFFSET && entrance->offset < 0) ||
        (entrance->mode == BTECH_MAP_ENTRANCE_EXACT &&
         (entrance->x < 0 || entrance->y < 0 ||
          entrance->x >= child_map->map_width ||
          entrance->y >= child_map->map_height)))
      return false;
  }
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)child, true);
  if (entry == nullptr)
    return false;
  entry->has_map_link = true;
  entry->map_link = *link;
  return true;
}

void btech_map_link_clear(BtechContext *context, BtechObjectId child) {
  BtechConfigurationEntry *entry =
      btech_configuration_entry(context, (DbRef)child, false);
  if (entry == nullptr)
    return;
  entry->has_map_link = false;
  entry->map_link = (BtechMapLink){};
  configuration_entry_prune(context, entry);
}

typedef struct MapLinkVisit {
  BtechMapLinkVisitor visitor;
  void *context;
} MapLinkVisit;

static bool visit_map_link(const RedBlackTreeVisitCall *call) {
  BtechConfigurationEntry *entry = call->data;
  MapLinkVisit *visit = call->context;
  return (!entry->has_map_link ||
          visit->visitor(entry->object, &entry->map_link, visit->context)) != 0;
}

void btech_map_links_visit(BtechContext *context, BtechMapLinkVisitor visitor,
                           void *visitor_context) {
  if (context == nullptr || context->configurations == nullptr ||
      visitor == nullptr)
    return;
  MapLinkVisit visit = {.visitor = visitor, .context = visitor_context};
  red_black_tree_walk(context->configurations, WALK_INORDER, visit_map_link,
                      &visit);
}

typedef struct ForgetReferences {
  DbRef object;
  DbRef *empty_objects;
  size_t empty_count;
  size_t capacity;
} ForgetReferences;

static bool forget_references(const RedBlackTreeVisitCall *call) {
  BtechConfigurationEntry *entry = call->data;
  ForgetReferences *forget = call->context;
  if (entry->assigned_pilot == forget->object)
    entry->assigned_pilot = NOTHING;
  if (entry->has_map_link && entry->map_link.parent == forget->object) {
    entry->has_map_link = false;
    entry->map_link = (BtechMapLink){};
  }
  if (entry->object != forget->object &&
      btech_configuration_entry_is_empty(entry)) {
    DbRef *empty = checked_storage_at(forget->empty_objects, forget->capacity,
                                      sizeof(*forget->empty_objects),
                                      forget->empty_count++);
    *empty = entry->object;
  }
  return true;
}

void btech_configuration_forget(BtechContext *context, BtechObjectId object) {
  if (context == nullptr || context->configurations == nullptr)
    return;
  const size_t CAPACITY = red_black_tree_size(context->configurations);
  DbRef *empty_objects =
      checked_storage_allocate_array(CAPACITY, sizeof(*empty_objects));
  ForgetReferences forget = {.object = (DbRef)object,
                             .empty_objects = empty_objects,
                             .capacity = CAPACITY};
  red_black_tree_walk(context->configurations, WALK_INORDER, forget_references,
                      &forget);
  for (size_t index = 0; index < forget.empty_count; index++) {
    const DbRef EMPTY = *(const DbRef *)checked_storage_at_const(
        empty_objects, CAPACITY, sizeof(*empty_objects), index);
    BtechConfigurationEntry *empty =
        red_black_tree_delete_integer(context->configurations, EMPTY);
    if (empty != nullptr)
      configuration_entry_free(empty);
  }
  free(empty_objects);
  BtechConfigurationEntry *entry =
      red_black_tree_delete_integer(context->configurations, (DbRef)object);
  if (entry != nullptr)
    configuration_entry_free(entry);
}
