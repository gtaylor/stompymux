#include "btechstats_internal.h"

static CharacterFixedState fixed_state_from_stats(BtechContext *context,
                                                  PSTATS *stats) {
  return (CharacterFixedState){
      .bruise = stats->values[char_getvaluecode(context, "Bruise")],
      .lethal = stats->values[char_getvaluecode(context, "Lethal")],
      .build = stats->values[char_getvaluecode(context, "Build")],
      .reflexes = stats->values[char_getvaluecode(context, "Reflexes")],
      .intuition = stats->values[char_getvaluecode(context, "Intuition")],
      .learn = stats->values[char_getvaluecode(context, "Learn")],
      .charisma = stats->values[char_getvaluecode(context, "Charisma")],
  };
}

static void fixed_state_to_stats(BtechContext *context,
                                 const CharacterFixedState *fixed, int modes,
                                 PSTATS *stats) {
  if (modes & VALUES_HEALTH) {
    stats->values[char_getvaluecode(context, "Bruise")] = fixed->bruise;
    stats->values[char_getvaluecode(context, "Lethal")] = fixed->lethal;
  }
  if (modes & VALUES_ATTRS) {
    stats->values[char_getvaluecode(context, "Build")] = fixed->build;
    stats->values[char_getvaluecode(context, "Reflexes")] = fixed->reflexes;
    stats->values[char_getvaluecode(context, "Intuition")] = fixed->intuition;
    stats->values[char_getvaluecode(context, "Learn")] = fixed->learn;
    stats->values[char_getvaluecode(context, "Charisma")] = fixed->charisma;
  }
}

static bool selected_variable_value(int code, int modes) {
  return ((modes & VALUES_SKILLS) && char_values[code].type == CHAR_SKILL) ||
         ((modes & VALUES_ADVS) &&
          (char_values[code].type == CHAR_ADVANTAGE || code == LIVES_NUMBER));
}

static void store_variable_values(BtechContext *context, DbRef player,
                                  PSTATS *stats, int modes) {
  for (int code = 0; code < NUM_CHARVALUES; code++) {
    if (!selected_variable_value(code, modes))
      continue;
    int default_value = code == LIVES_NUMBER ? 1 : 0;
    if (stats->values[code] == default_value && stats->xp[code] == 0 &&
        stats->last_use[code] == 0) {
      character_state_value_remove(context->database, player,
                                   char_values[code].name);
      continue;
    }
    character_state_value_set(context->database, player, char_values[code].name,
                              stats->values[code], stats->xp[code],
                              stats->last_use[code]);
  }
}

static void retrieve_variable_values(BtechContext *context, DbRef player,
                                     int modes, PSTATS *stats) {
  if (modes & VALUES_ADVS)
    stats->values[LIVES_NUMBER] = 1;
  for (size_t index = 0;
       index < character_state_value_count(context->database, player);
       index++) {
    CharacterValueStateView entry;
    if (!character_state_value_entry(context->database, player, index, &entry))
      continue;
    int code = char_getvaluecode(context, entry.name);
    if (code < 0 || !selected_variable_value(code, modes))
      continue;
    stats->values[code] = entry.value;
    stats->xp[code] = entry.xp;
    stats->last_use[code] = entry.last_used;
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
  bzero(stats, sizeof(*stats));
  if (!is_player(context->database, player))
    return;
  CharacterFixedState fixed;
  if (!character_state_fixed_get(context->database, player, &fixed))
    return;
  fixed_state_to_stats(context, &fixed, modes, stats);
  retrieve_variable_values(context, player, modes, stats);
}

bool character_state_validate_all(BtechContext *context) {
  DbRef player;

  DO_WHOLE_DB(context->database, player) {
    for (size_t index = 0;
         index < character_state_value_count(context->database, player);
         index++) {
      CharacterValueStateView entry;
      if (!character_state_value_entry(context->database, player, index,
                                       &entry))
        return false;
      int code = char_getvaluecode(context, entry.name);
      if (code < 0 || strcmp(entry.name, char_values[code].name) != 0 ||
          (char_values[code].type != CHAR_SKILL &&
           char_values[code].type != CHAR_ADVANTAGE && code != LIVES_NUMBER))
        return false;
    }
  }
  return true;
}
