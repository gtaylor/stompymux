#include <stddef.h>
#include <string.h>

#include "btech/context.h"
#include "btechstats.h"
#include "btechstats_api.h"
#include "btechstats_global.h"
#include "btechstats_internal.h"
#include "context_internal.h" // IWYU pragma: keep
#include "mux/objects/character_state.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

static CharacterFixedState fixed_state_from_stats(BtechContext *context,
                                                  PSTATS *stats) {
  return (CharacterFixedState){
      .bruise = character_stats_value_get(stats,
                                          char_getvaluecode(context, "Bruise")),
      .lethal = character_stats_value_get(stats,
                                          char_getvaluecode(context, "Lethal")),
      .build =
          character_stats_value_get(stats, char_getvaluecode(context, "Build")),
      .reflexes = character_stats_value_get(
          stats, char_getvaluecode(context, "Reflexes")),
      .intuition = character_stats_value_get(
          stats, char_getvaluecode(context, "Intuition")),
      .learn =
          character_stats_value_get(stats, char_getvaluecode(context, "Learn")),
      .charisma = character_stats_value_get(
          stats, char_getvaluecode(context, "Charisma")),
  };
}

static void fixed_state_to_stats(BtechContext *context,
                                 const CharacterFixedState *fixed, int modes,
                                 PSTATS *stats) {
  if (modes & VALUES_HEALTH) {
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats,
        .code = char_getvaluecode(context, "Bruise"),
        .value = fixed->bruise});
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats,
        .code = char_getvaluecode(context, "Lethal"),
        .value = fixed->lethal});
  }
  if (modes & VALUES_ATTRS) {
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats,
        .code = char_getvaluecode(context, "Build"),
        .value = fixed->build});
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats,
        .code = char_getvaluecode(context, "Reflexes"),
        .value = fixed->reflexes});
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats,
        .code = char_getvaluecode(context, "Intuition"),
        .value = fixed->intuition});
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats,
        .code = char_getvaluecode(context, "Learn"),
        .value = fixed->learn});
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats,
        .code = char_getvaluecode(context, "Charisma"),
        .value = fixed->charisma});
  }
}

static bool selected_variable_value(int code, int modes) {
  return (((modes & VALUES_SKILLS) &&
           character_value_definition(code)->type == CHAR_SKILL) ||
          ((modes & VALUES_ADVS) &&
           (character_value_definition(code)->type == CHAR_ADVANTAGE ||
            code == LIVES_NUMBER))) != 0;
}

static void store_variable_values(BtechContext *context, DbRef player,
                                  PSTATS *stats, int modes) {
  for (int code = 0; code < NUM_CHARVALUES; code++) {
    if (!selected_variable_value(code, modes))
      continue;
    int default_value = code == LIVES_NUMBER ? 1 : 0;
    const CharacterValue *definition = character_value_definition(code);
    if (character_stats_value_get(stats, code) == default_value &&
        character_stats_xp_get(stats, code) == 0 &&
        character_stats_last_use_get(stats, code) == 0) {
      character_state_value_remove(context->database, player, definition->name);
      continue;
    }
    character_state_value_set(&(CharacterStateValueChange){
        .database = context->database,
        .player = player,
        .name = definition->name,
        .value = character_stats_value_get(stats, code),
        .experience = character_stats_xp_get(stats, code),
        .last_used = character_stats_last_use_get(stats, code)});
  }
}

typedef struct CharacterVariableRetrieval {
  BtechContext *context;
  DbRef player;
  int modes;
  PSTATS *stats;
} CharacterVariableRetrieval;

static void
retrieve_variable_values(const CharacterVariableRetrieval *request) {
  BtechContext *context = request->context;
  const DbRef PLAYER = request->player;
  const int MODES = request->modes;
  PSTATS *stats = request->stats;
  if (MODES & VALUES_ADVS)
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats, .code = LIVES_NUMBER, .value = 1});
  for (size_t index = 0;
       index < character_state_value_count(context->database, PLAYER);
       index++) {
    CharacterStateEntryResult result =
        character_state_value_entry(&(CharacterStateEntryRequest){
            .database = context->database, .player = PLAYER, .index = index});
    if (!result.found)
      continue;
    CharacterValueStateView entry = result.entry;
    int code = char_getvaluecode(context, entry.name);
    if (code < 0 || !selected_variable_value(code, MODES))
      continue;
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats, .code = code, .value = entry.value});
    character_stats_xp_set(&(CharacterStatsExperienceChange){
        .stats = stats, .code = code, .value = entry.xp});
    character_stats_last_use_set(&(CharacterStatsLastUseChange){
        .stats = stats, .code = code, .value = entry.last_used});
  }
}

void character_stats_store(BtechContext *context, DbRef player, PSTATS *stats,
                           int modes) {
  if (!is_player(context->database, player))
    return;
  if (modes & (VALUES_HEALTH | VALUES_ATTRS)) {
    CharacterFixedState fixed;
    if (!character_state_fixed_get(context->database, player, &fixed))
      return;
    CharacterFixedState changed = fixed_state_from_stats(context, stats);
    if (modes & VALUES_HEALTH) {
      fixed.bruise = changed.bruise;
      fixed.lethal = changed.lethal;
    }
    if (modes & VALUES_ATTRS) {
      fixed.build = changed.build;
      fixed.reflexes = changed.reflexes;
      fixed.intuition = changed.intuition;
      fixed.learn = changed.learn;
      fixed.charisma = changed.charisma;
    }
    character_state_fixed_set(context->database, player, &fixed);
  }
  if ((modes & (VALUES_ADVS | VALUES_SKILLS)) &&
      player == context->cached_target_character)
    context->cached_target_character = -1;
  store_variable_values(context, player, stats, modes);
}

void character_stats_retrieve(BtechContext *context, DbRef player, int modes,
                              PSTATS *stats) {
  memset(stats, 0, sizeof(*stats));
  if (!is_player(context->database, player))
    return;
  CharacterFixedState fixed;
  if (!character_state_fixed_get(context->database, player, &fixed))
    return;
  fixed_state_to_stats(context, &fixed, modes, stats);
  retrieve_variable_values(&(CharacterVariableRetrieval){
      .context = context, .player = player, .modes = modes, .stats = stats});
}

bool character_state_validate_all(BtechContext *context) {
  DbRef player;

  DO_WHOLE_DB(context->database, player) {
    for (size_t index = 0;
         index < character_state_value_count(context->database, player);
         index++) {
      CharacterStateEntryResult result =
          character_state_value_entry(&(CharacterStateEntryRequest){
              .database = context->database, .player = player, .index = index});
      if (!result.found)
        return false;
      CharacterValueStateView entry = result.entry;
      int code = char_getvaluecode(context, entry.name);
      if (code < 0)
        return false;
      const CharacterValue *definition = character_value_definition(code);
      if (strcmp(entry.name, definition->name) != 0 ||
          (definition->type != CHAR_SKILL &&
           definition->type != CHAR_ADVANTAGE && code != LIVES_NUMBER))
        return false;
    }
  }
  return true;
}
