#include "mux/commands/command_context.h"
#include "mux/commands/macro.h"

#include <limits.h>
#include <stdlib.h>
#include <string.h>

#include "mux/communication/commac.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

static int macro_player_storage_value(DbRef player) {
  if (player < INT_MIN)
    return INT_MIN;
  if (player > INT_MAX)
    return INT_MAX;
  return (int)player;
}

MacroSet *macro_registry_item(const MacroRegistry *registry, size_t index) {
  return *(MacroSet *const *)checked_storage_at_const(
      (const void *)registry->sets, (size_t)registry->count,
      sizeof(*registry->sets), index);
}

MacroSet **macro_registry_slot(MacroRegistry *registry, size_t index) {
  return (MacroSet **)checked_storage_at((void *)registry->sets,
                                         (size_t)registry->capacity,
                                         sizeof(*registry->sets), index);
}

char *macro_string_item(const MacroSet *set, size_t index) {
  return *(char *const *)checked_storage_at_const((const void *)set->string,
                                                  (size_t)set->macro_count,
                                                  sizeof(*set->string), index);
}

char **macro_string_slot(MacroSet *set, size_t index) {
  return (char **)checked_storage_at((void *)set->string,
                                     (size_t)set->macro_capacity,
                                     sizeof(*set->string), index);
}

char *macro_alias_at(const MacroSet *set, size_t index) {
  if (index >= (size_t)set->macro_capacity)
    abort();
  return checked_storage_at(set->alias, (size_t)set->macro_capacity * 5,
                            sizeof(char), index * 5);
}

void macro_registry_initialize(MacroRegistry *registry,
                               ChannelRegistry *channels) {
  memset(registry, 0, sizeof(*registry));
  registry->channels = channels;
}

void macro_registry_destroy(MacroRegistry *registry) {
  if (registry == nullptr)
    return;
  ChannelRegistry *channels = registry->channels;
  for (int index = 0; index < registry->count; index++) {
    MacroSet *set = macro_registry_item(registry, (size_t)index);
    for (int macro = 0; macro < set->macro_count; macro++)
      free(macro_string_item(set, (size_t)macro));
    free(set->desc);
    free(set->alias);
    free((void *)set->string);
    free(set);
  }
  free((void *)registry->sets);
  macro_registry_initialize(registry, channels);
}

MacroSet *get_macro_set(const MacroSetRequest *request) {
  MacroRegistry *registry = request->registry;
  DbRef player = request->player;
  int which = request->slot;
  struct commac *commac = get_commac(registry->channels, player);
  if (commac == nullptr)
    return nullptr;

  int set = -1;
  if (which >= 0 && which < MAX_SLOTS)
    set = commac_macro_at(commac, (size_t)which);
  else if (commac->curmac >= 0)
    set = commac_macro_at(commac, (size_t)commac->curmac);

  return set == -1 ? nullptr : macro_registry_item(registry, (size_t)set);
}

void do_create_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                     char *description) {
  struct commac *commac = get_commac(registry->channels, player);
  int first = -1;
  for (int index = 0; index < MAX_SLOTS && first < 0; index++) {
    if (commac_macro_at(commac, (size_t)index) == -1)
      first = index;
  }
  if (first < 0) {
    notify_checked(match->evaluation, player, player,
                   "MACRO: Sorry, you already have 5 sets defined on you.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }

  if (registry->count >= registry->capacity) {
    registry->capacity += 10;
    MacroSet **sets =
        (MacroSet **)malloc(sizeof(*sets) * (size_t)registry->capacity);
    for (int index = 0; index < registry->count; index++)
      *(MacroSet **)checked_storage_at((void *)sets, (size_t)registry->capacity,
                                       sizeof(*sets), (size_t)index) =
          macro_registry_item(registry, (size_t)index);
    free((void *)registry->sets);
    registry->sets = sets;
  }

  const int set = registry->count++;
  MacroSet *created = malloc(sizeof(*created));
  *macro_registry_slot(registry, (size_t)set) = created;
  created->player = macro_player_storage_value(player);
  created->status = 0;
  created->macro_count = 0;
  created->macro_capacity = 0;
  created->alias = nullptr;
  created->string = nullptr;
  created->desc = malloc(strlen(description) + 1);
  StringCopy(created->desc, description);
  commac->curmac = first;
  commac_macro_set(commac, (size_t)first, set);

  notify_printf(match->evaluation, player,
                "MACRO: Macro set %d created with description %s.", set,
                description);
}

int can_write_macros(DbRef player, MacroSet *set) {
  if (set->status & MACRO_L)
    return 0;
  if (set->player == player)
    return 1;
  return set->status & MACRO_W;
}

int can_read_macros(GameDatabase *database, DbRef player, MacroSet *set) {
  if (is_wizard(database, player))
    return 1;
  if (set == nullptr)
    return 0;
  if (set->player == player)
    return 1;
  return set->status & MACRO_R;
}
