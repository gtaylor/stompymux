/*
 * match.c -- Routines for parsing arguments
 */

#include "mux/commands/command_context.h"
#include "mux/lua/lua_runtime.h"
#include "mux/support/stringutil.h"
#include <ctype.h>
#include <stdlib.h>
#include <string.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/object_spatial.h"
#include "mux/world/player.h"

enum MatchConfidence : int {
  CON_LOCAL = 0x01,    /* Match is near me. */
  CON_TYPE = 0x02,     /* Match is of requested type. */
  CON_LOCK = 0x04,     /* I pass the lock on match. */
  CON_COMPLETE = 0x08, /* Name given is the full name. */
  CON_TOKEN = 0x10,    /* Name is a special token. */
  CON_DBREF = 0x20,    /* Name is a dbref. */
};

typedef struct MatchCandidate {
  DbRef object;
  int confidence;
} MatchCandidate;

typedef struct MatchStateSnapshot {
  int confidence;
  int count;
  int pref_type;
  bool check_keys;
  DbRef absolute_form;
  DbRef match;
  DbRef player;
  char *string;
  char *normalized;
} MatchStateSnapshot;

static MatchStateSnapshot match_state_snapshot(MatchContext *match_context) {
  char *normalized = alloc_lbuf("promote_match.normalized");

  (void)string_copy_bounded(normalized, LBUF_SIZE, match_context->string);
  return (MatchStateSnapshot){
      .confidence = match_context->confidence,
      .count = match_context->count,
      .pref_type = match_context->pref_type,
      .check_keys = match_context->check_keys,
      .absolute_form = match_context->absolute_form,
      .match = match_context->match,
      .player = match_context->player,
      .string = match_context->string,
      .normalized = normalized,
  };
}

static void match_state_restore(MatchContext *match_context,
                                MatchStateSnapshot *snapshot) {
  match_context->confidence = snapshot->confidence;
  match_context->count = snapshot->count;
  match_context->pref_type = snapshot->pref_type;
  match_context->check_keys = snapshot->check_keys;
  match_context->absolute_form = snapshot->absolute_form;
  match_context->match = snapshot->match;
  match_context->player = snapshot->player;
  match_context->string = snapshot->string;
  (void)string_copy_bounded(match_context->string,
                            strlen(snapshot->normalized) + 1,
                            snapshot->normalized);
  free_buf(snapshot->normalized);
}

static void promote_match(MatchContext *match_context,
                          MatchCandidate candidate) {
  DbRef what = candidate.object;
  int confidence = candidate.confidence;
  LuaLockInvocation lock;
  LuaLockResult *result;
  /*
   * Check for type and locks, if requested
   */

  if (match_context->pref_type != OBJECT_TYPE_NOTYPE) {
    if (is_good_obj(match_context->evaluation->world->database, what) &&
        (typeof_obj(match_context->evaluation->world->database, what) ==
         match_context->pref_type))
      confidence |= CON_TYPE;
  }
  if (match_context->check_keys) {
    MatchStateSnapshot save_md = match_state_snapshot(match_context);
    result = checked_storage_allocate(sizeof(*result));

    if (is_good_obj(match_context->evaluation->world->database, what) &&
        lock_test(match_context->evaluation, match_context->player,
                  match_context->player, match_context->player, what,
                  LUA_LOCK_MATCH, true, &lock, result))
      confidence |= CON_LOCK;
    match_state_restore(match_context, &save_md);
    free_buf(result);
  }
  /*
   * If nothing matched, take it
   */

  if (match_context->count == 0) {
    match_context->match = what;
    match_context->confidence = confidence;
    match_context->count = 1;
    return;
  }
  /*
   * If confidence is lower, ignore
   */

  if (confidence < match_context->confidence) {
    return;
  }
  /*
   * If confidence is higher, replace
   */

  if (confidence > match_context->confidence) {
    match_context->match = what;
    match_context->confidence = confidence;
    match_context->count = 1;
    return;
  }
  /*
   * Equal confidence, pick randomly
   */

  if (random() % 2) {
    match_context->match = what;
  }
  match_context->count++;
}

/*
 * ---------------------------------------------------------------------------
 * * This function removes repeated spaces from the template to which object
 * * names are being matched.  It also removes inital and terminal spaces.
 */

static char *munge_space_for_match(MatchContext *match_context,
                                   const char *name) {
  size_t input = 0;
  size_t output = 0;
  size_t length = strlen(name);

  while (input < length &&
         (isspace)(*(const unsigned char *)checked_storage_at_const(
             name, length, sizeof(char), input)))
    input++; /*
              * remove inital spaces
              */
  while (input < length) {
    while (input < length &&
           !(isspace)(*(const unsigned char *)checked_storage_at_const(
               name, length, sizeof(char), input))) {
      *(char *)checked_storage_at(match_context->normalized,
                                  sizeof(match_context->normalized),
                                  sizeof(char), output++) =
          *(const char *)checked_storage_at_const(name, length, sizeof(char),
                                                  input++);
    }
    while (input < length &&
           (isspace)(*(const unsigned char *)checked_storage_at_const(
               name, length, sizeof(char), input)))
      input++;
    if (input < length)
      *(char *)checked_storage_at(match_context->normalized,
                                  sizeof(match_context->normalized),
                                  sizeof(char), output++) = ' ';
  }
  *(char *)checked_storage_at(match_context->normalized,
                              sizeof(match_context->normalized), sizeof(char),
                              output) =
      '\0'; /*
             * remove terminal spaces and terminate * * *
             *
             * * string
             */
  return match_context->normalized;
}

void match_player(MatchContext *match_context) {
  DbRef match;

  if (match_context->confidence >= CON_DBREF) {
    return;
  }
  if (is_good_obj(match_context->evaluation->world->database,
                  match_context->absolute_form) &&
      is_player(match_context->evaluation->world->database,
                match_context->absolute_form)) {
    promote_match(match_context,
                  (MatchCandidate){match_context->absolute_form, CON_DBREF});
    return;
  }
  if (*match_context->string == LOOKUP_TOKEN) {
    size_t length = strlen(match_context->string);
    size_t offset = 1;
    while (offset < length &&
           (isspace)(*(const unsigned char *)checked_storage_at_const(
               match_context->string, length, sizeof(char), offset)))
      offset++;
    match = lookup_player(
        match_context->evaluation->world, NOTHING,
        checked_mutable_string_suffix(match_context->string, offset), 1);
    if (is_good_obj(match_context->evaluation->world->database, match)) {
      promote_match(match_context, (MatchCandidate){match, CON_TOKEN});
    }
  }
}

/*
 * returns nnn if name = #nnn, else NOTHING
 */

static DbRef absolute_name(MatchContext *match_context, int need_pound) {
  DbRef match;
  char *mname;

  mname = match_context->string;
  if (need_pound) {
    if (*match_context->string != NUMBER_TOKEN) {
      return NOTHING;
    }
    mname = checked_mutable_string_suffix(mname, 1);
  }
  match = parse_dbref(mname);
  if (is_good_obj(match_context->evaluation->world->database, match)) {
    return match;
  }
  return NOTHING;
}

void match_absolute(MatchContext *match_context) {
  if (match_context->confidence >= CON_DBREF)
    return;
  if (is_good_obj(match_context->evaluation->world->database,
                  match_context->absolute_form))
    promote_match(match_context,
                  (MatchCandidate){match_context->absolute_form, CON_DBREF});
}

void match_numeric(MatchContext *match_context) {
  DbRef match;

  if (match_context->confidence >= CON_DBREF)
    return;
  match = absolute_name(match_context, 0);
  if (is_good_obj(match_context->evaluation->world->database, match))
    promote_match(match_context, (MatchCandidate){match, CON_DBREF});
}

void match_me(MatchContext *match_context) {
  if (match_context->confidence >= CON_DBREF)
    return;
  if (is_good_obj(match_context->evaluation->world->database,
                  match_context->absolute_form) &&
      (match_context->absolute_form == match_context->player)) {
    promote_match(match_context, (MatchCandidate){match_context->player,
                                                  CON_DBREF | CON_LOCAL});
    return;
  }
  if (!string_compare(match_context->evaluation->world->configuration,
                      match_context->string, "me"))
    promote_match(match_context, (MatchCandidate){match_context->player,
                                                  CON_TOKEN | CON_LOCAL});
}

void match_here(MatchContext *match_context) {
  DbRef loc;

  if (match_context->confidence >= CON_DBREF)
    return;
  if (is_good_obj(match_context->evaluation->world->database,
                  match_context->player) &&
      has_location(match_context->evaluation->world->database,
                   match_context->player)) {
    loc = game_object_location(match_context->evaluation->world->database,
                               match_context->player);
    if (is_good_obj(match_context->evaluation->world->database, loc)) {
      if (loc == match_context->absolute_form) {
        promote_match(match_context,
                      (MatchCandidate){loc, CON_DBREF | CON_LOCAL});
      } else if (!string_compare(
                     match_context->evaluation->world->configuration,
                     match_context->string, "here")) {
        promote_match(match_context,
                      (MatchCandidate){loc, CON_TOKEN | CON_LOCAL});
      } else if (!string_compare(
                     match_context->evaluation->world->configuration,
                     match_context->string,
                     game_object_pure_name(
                         match_context->evaluation->world->database, loc))) {
        promote_match(match_context,
                      (MatchCandidate){loc, CON_COMPLETE | CON_LOCAL});
      }
    }
  }
}

static void match_list(MatchContext *match_context, DbRef first, int local) {
  const char *namebuf;

  if (match_context->confidence >= CON_DBREF)
    return;
  DOLIST(match_context->evaluation->world->database, first, first) {
    if (first == match_context->absolute_form) {
      promote_match(match_context, (MatchCandidate){first, CON_DBREF | local});
      return;
    }
    namebuf = game_object_pure_name(match_context->evaluation->world->database,
                                    first);

    if (!string_compare(match_context->evaluation->world->configuration,
                        namebuf, match_context->string)) {
      promote_match(match_context,
                    (MatchCandidate){first, CON_COMPLETE | local});
    } else if (string_match(namebuf, match_context->string)) {
      promote_match(match_context, (MatchCandidate){first, local});
    }
  }
}

void match_possession(MatchContext *match_context) {
  if (match_context->confidence >= CON_DBREF)
    return;
  if (is_good_obj(match_context->evaluation->world->database,
                  match_context->player) &&
      has_contents(match_context->evaluation->world->database,
                   match_context->player))
    match_list(match_context,
               game_object_contents(match_context->evaluation->world->database,
                                    match_context->player),
               CON_LOCAL);
}

void match_neighbor(MatchContext *match_context) {
  DbRef loc;

  if (match_context->confidence >= CON_DBREF)
    return;
  if (is_good_obj(match_context->evaluation->world->database,
                  match_context->player) &&
      has_location(match_context->evaluation->world->database,
                   match_context->player)) {
    loc = game_object_location(match_context->evaluation->world->database,
                               match_context->player);
    if (is_good_obj(match_context->evaluation->world->database, loc)) {
      match_list(
          match_context,
          game_object_contents(match_context->evaluation->world->database, loc),
          CON_LOCAL);
    }
  }
}

bool matches_exit_from_list(const char *string, const char *pattern) {
  size_t pattern_length = strlen(pattern);
  size_t pattern_offset = 0;
  size_t string_length = strlen(string);
  while (pattern_offset < pattern_length) {
    size_t candidate_offset = 0;
    while (candidate_offset < string_length &&
           pattern_offset < pattern_length &&
           *(const char *)checked_storage_at_const(
               pattern, pattern_length, sizeof(char), pattern_offset) !=
               EXIT_DELIMITER &&
           ascii_to_lower(*(const char *)checked_storage_at_const(
               string, string_length, sizeof(char), candidate_offset)) ==
               ascii_to_lower(*(const char *)checked_storage_at_const(
                   pattern, pattern_length, sizeof(char), pattern_offset))) {
      candidate_offset++;
      pattern_offset++;
    }

    if (candidate_offset == string_length) {
      while (pattern_offset < pattern_length &&
             (isspace)(*(const unsigned char *)checked_storage_at_const(
                 pattern, pattern_length, sizeof(char), pattern_offset)))
        pattern_offset++;
      if (pattern_offset == pattern_length ||
          *(const char *)checked_storage_at_const(
              pattern, pattern_length, sizeof(char), pattern_offset) ==
              EXIT_DELIMITER)
        return true;
    }

    while (pattern_offset < pattern_length) {
      char character = *(const char *)checked_storage_at_const(
          pattern, pattern_length, sizeof(char), pattern_offset++);
      if (character == EXIT_DELIMITER)
        break;
    }
    while (pattern_offset < pattern_length &&
           (isspace)(*(const unsigned char *)checked_storage_at_const(
               pattern, pattern_length, sizeof(char), pattern_offset)))
      pattern_offset++;
  }

  return false;
}

typedef struct ExitMatchRequest {
  MatchContext *context;
  DbRef location;
  DbRef base_location;
  int confidence;
} ExitMatchRequest;

static int match_exit_internal(const ExitMatchRequest *request) {
  MatchContext *match_context = request->context;
  DbRef loc = request->location;
  DbRef baseloc = request->base_location;
  int local = request->confidence;
  DbRef exit;
  int result;
  int key;

  if (!is_good_obj(match_context->evaluation->world->database, loc) ||
      !has_exits(match_context->evaluation->world->database, loc))
    return 1;

  result = 0;
  DOLIST(match_context->evaluation->world->database, exit,
         game_object_exits(match_context->evaluation->world->database, loc)) {
    if (exit == match_context->absolute_form) {
      key = 0;
      if (is_examinable(match_context->evaluation->world->database,
                        match_context->player, loc))
        key |= VE_LOC_XAM;
      if (is_dark(match_context->evaluation->world->database, loc))
        key |= VE_LOC_DARK;
      if (is_dark(match_context->evaluation->world->database, baseloc))
        key |= VE_LOC_DARK;
      if (exit_visible(
              &(ExitVisibilityRequest){.evaluation = match_context->evaluation,
                                       .exit = exit,
                                       .viewer = match_context->player,
                                       .options = key})) {
        promote_match(match_context, (MatchCandidate){exit, CON_DBREF | local});
        return 1;
      }
    }
    if (matches_exit_from_list(
            match_context->string,
            game_object_pure_name(match_context->evaluation->world->database,
                                  exit))) {
      promote_match(match_context,
                    (MatchCandidate){exit, CON_COMPLETE | local});
      result = 1;
    }
  }
  return result;
}

void match_exit(MatchContext *match_context) {
  DbRef loc;

  if (match_context->confidence >= CON_DBREF)
    return;
  loc = game_object_location(match_context->evaluation->world->database,
                             match_context->player);
  if (is_good_obj(match_context->evaluation->world->database,
                  match_context->player) &&
      has_location(match_context->evaluation->world->database,
                   match_context->player))
    (void)match_exit_internal(&(ExitMatchRequest){.context = match_context,
                                                  .location = loc,
                                                  .base_location = loc,
                                                  .confidence = CON_LOCAL});
}

void match_carried_exit(MatchContext *match_context) {
  if (match_context->confidence >= CON_DBREF)
    return;
  if (is_good_obj(match_context->evaluation->world->database,
                  match_context->player) &&
      has_exits(match_context->evaluation->world->database,
                match_context->player)) {
    (void)match_exit_internal(
        &(ExitMatchRequest){.context = match_context,
                            .location = match_context->player,
                            .base_location = match_context->player,
                            .confidence = CON_LOCAL});
  }
}

void match_zone_exit(MatchContext *match_context) {
  if (match_context->confidence >= CON_DBREF)
    return;
  if (is_good_obj(match_context->evaluation->world->database,
                  match_context->player) &&
      has_exits(match_context->evaluation->world->database,
                match_context->player)) {
    (void)match_exit_internal(&(ExitMatchRequest){
        .context = match_context,
        .location = game_object_zone(match_context->evaluation->world->database,
                                     match_context->player),
        .base_location =
            game_object_zone(match_context->evaluation->world->database,
                             match_context->player)});
  }
}

void match_everything(MatchContext *match_context, int key) {
  /*
   * Try matching me, then here, then absolute, then player FIRST, since
   * this will hit most cases. STOP if we get something, since those are
   * exact matches.
   */

  match_me(match_context);
  match_here(match_context);
  match_absolute(match_context);
  if (key & MAT_NUMERIC)
    match_numeric(match_context);
  match_player(match_context);
  if (match_context->confidence >= CON_TOKEN)
    return;

  if (!(key & MAT_NO_EXITS)) {
    match_carried_exit(match_context);
    match_exit(match_context);
  }
  match_neighbor(match_context);
  match_possession(match_context);
}

DbRef match_result(MatchContext *match_context) {
  switch (match_context->count) {
  case 0:
    return NOTHING;
  case 1:
    return match_context->match;
  default:
    return AMBIGUOUS;
  }
}

/*
 * use this if you don't care about ambiguity
 */

DbRef last_match_result(MatchContext *match_context) {
  return match_context->match;
}

// NOLINTNEXTLINE(misc-no-recursion): matcher rejects possession cycles.
DbRef match_status(EvaluationContext *evaluation, DbRef player, DbRef match) {
  switch (match) {
  case NOTHING:
    notify_checked(evaluation, player, player, NOMATCH_MESSAGE,
                   MSG_ME_ALL | MSG_F_DOWN);
    return NOTHING;
  case AMBIGUOUS:
    notify_checked(evaluation, player, player, AMBIGUOUS_MESSAGE,
                   MSG_ME_ALL | MSG_F_DOWN);
    return NOTHING;
  case NOPERM:
    notify_checked(evaluation, player, player, NOPERM_MESSAGE,
                   MSG_ME_ALL | MSG_F_DOWN);
    return NOTHING;
  default:
    break;
  }
  if (is_good_obj(evaluation->world->database, match) &&
      is_dark(evaluation->world->database, match) &&
      is_good_obj(evaluation->world->database, player) &&
      !is_wizard(evaluation->world->database, player))
    return match_status(evaluation, player, NOTHING);
  return match;
}

DbRef noisy_match_result(MatchContext *match_context) {
  return match_status(match_context->evaluation, match_context->player,
                      match_result(match_context));
}

void init_match(MatchContext *match_context, DbRef player, const char *name,
                int type) {
  match_context->confidence = -1;
  match_context->count = match_context->check_keys = false;
  match_context->pref_type = type;
  match_context->match = NOTHING;
  match_context->player = player;
  match_context->string = munge_space_for_match(match_context, name);
  match_context->absolute_form = absolute_name(match_context, 1);
}

void init_match_check_keys(MatchContext *match_context, DbRef player,
                           const char *name, int type) {
  init_match(match_context, player, name, type);
  match_context->check_keys = true;
}
