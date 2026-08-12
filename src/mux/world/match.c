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

#define CON_LOCAL                                                              \
  0x01 /*                                                                      \
        * Match is near me                                                     \
        */
#define CON_TYPE                                                               \
  0x02 /*                                                                      \
        * Match is of requested type                                           \
        */
#define CON_LOCK                                                               \
  0x04 /*                                                                      \
        * I pass the lock on match                                             \
        */
#define CON_COMPLETE                                                           \
  0x08 /*                                                                      \
        * Name given is the full name                                          \
        */
#define CON_TOKEN                                                              \
  0x10 /*                                                                      \
        * Name is a special token                                              \
        */
#define CON_DBREF                                                              \
  0x20 /*                                                                      \
        * Name is a dbref                                                      \
        */

#define MD (*match_context)

typedef struct MatchCandidate {
  DbRef object;
  int confidence;
} MatchCandidate;

static void promote_match(MatchContext *match_context,
                          MatchCandidate candidate) {
  DbRef what = candidate.object;
  int confidence = candidate.confidence;
  LuaLockInvocation lock;
  LuaLockResult result;
  /*
   * Check for type and locks, if requested
   */

  if (MD.pref_type != OBJECT_TYPE_NOTYPE) {
    if (is_good_obj(MD.evaluation->world->database, what) &&
        (typeof_obj(MD.evaluation->world->database, what) == MD.pref_type))
      confidence |= CON_TYPE;
  }
  if (MD.check_keys) {
    MSTATE save_md;

    save_match_state(match_context, &save_md);
    if (is_good_obj(MD.evaluation->world->database, what) &&
        lock_test(MD.evaluation, MD.player, MD.player, MD.player, what,
                  LUA_LOCK_DEFAULT, LUA_LOCK_OPERATION_MATCH, true, &lock,
                  &result))
      confidence |= CON_LOCK;
    restore_match_state(match_context, &save_md);
  }
  /*
   * If nothing matched, take it
   */

  if (MD.count == 0) {
    MD.match = what;
    MD.confidence = confidence;
    MD.count = 1;
    return;
  }
  /*
   * If confidence is lower, ignore
   */

  if (confidence < MD.confidence) {
    return;
  }
  /*
   * If confidence is higher, replace
   */

  if (confidence > MD.confidence) {
    MD.match = what;
    MD.confidence = confidence;
    MD.count = 1;
    return;
  }
  /*
   * Equal confidence, pick randomly
   */

  if (random() % 2) {
    MD.match = what;
  }
  MD.count++;
}

/*
 * ---------------------------------------------------------------------------
 * * This function removes repeated spaces from the template to which object
 * * names are being matched.  It also removes inital and terminal spaces.
 */

static char *munge_space_for_match(MatchContext *match_context, char *name) {
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
      *(char *)checked_storage_at(MD.normalized, sizeof(MD.normalized),
                                  sizeof(char), output++) =
          *(const char *)checked_storage_at_const(name, length, sizeof(char),
                                                  input++);
    }
    while (input < length &&
           (isspace)(*(const unsigned char *)checked_storage_at_const(
               name, length, sizeof(char), input)))
      input++;
    if (input < length)
      *(char *)checked_storage_at(MD.normalized, sizeof(MD.normalized),
                                  sizeof(char), output++) = ' ';
  }
  *(char *)checked_storage_at(MD.normalized, sizeof(MD.normalized),
                              sizeof(char), output) =
      '\0'; /*
             * remove terminal spaces and terminate * * *
             *
             * * string
             */
  return MD.normalized;
}

void match_player(MatchContext *match_context) {
  DbRef match;

  if (MD.confidence >= CON_DBREF) {
    return;
  }
  if (is_good_obj(MD.evaluation->world->database, MD.absolute_form) &&
      is_player(MD.evaluation->world->database, MD.absolute_form)) {
    promote_match(match_context, (MatchCandidate){MD.absolute_form, CON_DBREF});
    return;
  }
  if (*MD.string == LOOKUP_TOKEN) {
    size_t length = strlen(MD.string);
    size_t offset = 1;
    while (offset < length &&
           (isspace)(*(const unsigned char *)checked_storage_at_const(
               MD.string, length, sizeof(char), offset)))
      offset++;
    match = lookup_player(MD.evaluation->world, NOTHING,
                          checked_mutable_string_suffix(MD.string, offset), 1);
    if (is_good_obj(MD.evaluation->world->database, match)) {
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

  mname = MD.string;
  if (need_pound) {
    if (*MD.string != NUMBER_TOKEN) {
      return NOTHING;
    }
    mname = checked_mutable_string_suffix(mname, 1);
  }
  match = parse_dbref(mname);
  if (is_good_obj(MD.evaluation->world->database, match)) {
    return match;
  }
  return NOTHING;
}

void match_absolute(MatchContext *match_context) {
  if (MD.confidence >= CON_DBREF)
    return;
  if (is_good_obj(MD.evaluation->world->database, MD.absolute_form))
    promote_match(match_context, (MatchCandidate){MD.absolute_form, CON_DBREF});
}

void match_numeric(MatchContext *match_context) {
  DbRef match;

  if (MD.confidence >= CON_DBREF)
    return;
  match = absolute_name(match_context, 0);
  if (is_good_obj(MD.evaluation->world->database, match))
    promote_match(match_context, (MatchCandidate){match, CON_DBREF});
}

void match_me(MatchContext *match_context) {
  if (MD.confidence >= CON_DBREF)
    return;
  if (is_good_obj(MD.evaluation->world->database, MD.absolute_form) &&
      (MD.absolute_form == MD.player)) {
    promote_match(match_context,
                  (MatchCandidate){MD.player, CON_DBREF | CON_LOCAL});
    return;
  }
  if (!string_compare(MD.evaluation->world->configuration, MD.string, "me"))
    promote_match(match_context,
                  (MatchCandidate){MD.player, CON_TOKEN | CON_LOCAL});
}

void match_home(MatchContext *match_context) {
  if (MD.confidence >= CON_DBREF)
    return;
  if (!string_compare(MD.evaluation->world->configuration, MD.string, "home"))
    promote_match(match_context, (MatchCandidate){HOME, CON_TOKEN});
}

void match_here(MatchContext *match_context) {
  DbRef loc;

  if (MD.confidence >= CON_DBREF)
    return;
  if (is_good_obj(MD.evaluation->world->database, MD.player) &&
      has_location(MD.evaluation->world->database, MD.player)) {
    loc = game_object_location(MD.evaluation->world->database, MD.player);
    if (is_good_obj(MD.evaluation->world->database, loc)) {
      if (loc == MD.absolute_form) {
        promote_match(match_context,
                      (MatchCandidate){loc, CON_DBREF | CON_LOCAL});
      } else if (!string_compare(MD.evaluation->world->configuration, MD.string,
                                 "here")) {
        promote_match(match_context,
                      (MatchCandidate){loc, CON_TOKEN | CON_LOCAL});
      } else if (!string_compare(MD.evaluation->world->configuration, MD.string,
                                 game_object_pure_name(
                                     MD.evaluation->world->database, loc))) {
        promote_match(match_context,
                      (MatchCandidate){loc, CON_COMPLETE | CON_LOCAL});
      }
    }
  }
}

static void match_list(MatchContext *match_context, DbRef first, int local) {
  char *namebuf;

  if (MD.confidence >= CON_DBREF)
    return;
  DOLIST(MD.evaluation->world->database, first, first) {
    if (first == MD.absolute_form) {
      promote_match(match_context, (MatchCandidate){first, CON_DBREF | local});
      return;
    }
    /*
     * Warning: make sure there are no other calls to game_object_name() in
     * promote_match or its called subroutines; they
     * would overwrite game_object_name()'s static buffer which is
     * needed by string_match().
     */
    namebuf = game_object_pure_name(MD.evaluation->world->database, first);

    if (!string_compare(MD.evaluation->world->configuration, namebuf,
                        MD.string)) {
      promote_match(match_context,
                    (MatchCandidate){first, CON_COMPLETE | local});
    } else if (string_match(namebuf, MD.string)) {
      promote_match(match_context, (MatchCandidate){first, local});
    }
  }
}

void match_possession(MatchContext *match_context) {
  if (MD.confidence >= CON_DBREF)
    return;
  if (is_good_obj(MD.evaluation->world->database, MD.player) &&
      has_contents(MD.evaluation->world->database, MD.player))
    match_list(match_context,
               game_object_contents(MD.evaluation->world->database, MD.player),
               CON_LOCAL);
}

void match_neighbor(MatchContext *match_context) {
  DbRef loc;

  if (MD.confidence >= CON_DBREF)
    return;
  if (is_good_obj(MD.evaluation->world->database, MD.player) &&
      has_location(MD.evaluation->world->database, MD.player)) {
    loc = game_object_location(MD.evaluation->world->database, MD.player);
    if (is_good_obj(MD.evaluation->world->database, loc)) {
      match_list(match_context,
                 game_object_contents(MD.evaluation->world->database, loc),
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
  int result, key;

  if (!is_good_obj(MD.evaluation->world->database, loc) ||
      !has_exits(MD.evaluation->world->database, loc))
    return 1;

  result = 0;
  DOLIST(MD.evaluation->world->database, exit,
         game_object_exits(MD.evaluation->world->database, loc)) {
    if (exit == MD.absolute_form) {
      key = 0;
      if (is_examinable(match_context->evaluation->world->database, MD.player,
                        loc))
        key |= VE_LOC_XAM;
      if (is_dark(MD.evaluation->world->database, loc))
        key |= VE_LOC_DARK;
      if (is_dark(MD.evaluation->world->database, baseloc))
        key |= VE_LOC_DARK;
      if (exit_visible(
              &(ExitVisibilityRequest){.evaluation = match_context->evaluation,
                                       .exit = exit,
                                       .viewer = MD.player,
                                       .options = key})) {
        promote_match(match_context, (MatchCandidate){exit, CON_DBREF | local});
        return 1;
      }
    }
    if (matches_exit_from_list(
            MD.string,
            game_object_pure_name(MD.evaluation->world->database, exit))) {
      promote_match(match_context,
                    (MatchCandidate){exit, CON_COMPLETE | local});
      result = 1;
    }
  }
  return result;
}

void match_exit(MatchContext *match_context) {
  DbRef loc;

  if (MD.confidence >= CON_DBREF)
    return;
  loc = game_object_location(MD.evaluation->world->database, MD.player);
  if (is_good_obj(MD.evaluation->world->database, MD.player) &&
      has_location(MD.evaluation->world->database, MD.player))
    (void)match_exit_internal(&(ExitMatchRequest){.context = match_context,
                                                  .location = loc,
                                                  .base_location = loc,
                                                  .confidence = CON_LOCAL});
}

void match_carried_exit(MatchContext *match_context) {
  if (MD.confidence >= CON_DBREF)
    return;
  if (is_good_obj(MD.evaluation->world->database, MD.player) &&
      has_exits(MD.evaluation->world->database, MD.player))
    (void)match_exit_internal(&(ExitMatchRequest){.context = match_context,
                                                  .location = MD.player,
                                                  .base_location = MD.player,
                                                  .confidence = CON_LOCAL});
}

void match_zone_exit(MatchContext *match_context) {
  if (MD.confidence >= CON_DBREF)
    return;
  if (is_good_obj(MD.evaluation->world->database, MD.player) &&
      has_exits(MD.evaluation->world->database, MD.player))
    (void)match_exit_internal(&(ExitMatchRequest){
        .context = match_context,
        .location = game_object_zone(MD.evaluation->world->database, MD.player),
        .base_location =
            game_object_zone(MD.evaluation->world->database, MD.player)});
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
  if (key & MAT_HOME)
    match_home(match_context);
  match_player(match_context);
  if (MD.confidence >= CON_TOKEN)
    return;

  if (!(key & MAT_NO_EXITS)) {
    match_carried_exit(match_context);
    match_exit(match_context);
  }
  match_neighbor(match_context);
  match_possession(match_context);
}

DbRef match_result(MatchContext *match_context) {
  switch (MD.count) {
  case 0:
    return NOTHING;
  case 1:
    return MD.match;
  default:
    return AMBIGUOUS;
  }
}

/*
 * use this if you don't care about ambiguity
 */

DbRef last_match_result(MatchContext *match_context) { return MD.match; }

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
  return match_status(match_context->evaluation, MD.player,
                      match_result(match_context));
}

void save_match_state(MatchContext *match_context, MSTATE *mstate) {
  mstate->confidence = MD.confidence;
  mstate->count = MD.count;
  mstate->pref_type = MD.pref_type;
  mstate->check_keys = MD.check_keys;
  mstate->absolute_form = MD.absolute_form;
  mstate->match = MD.match;
  mstate->player = MD.player;
  mstate->string = alloc_lbuf("save_match_state");
  string_copy(mstate->string, MD.string);
}

void restore_match_state(MatchContext *match_context, MSTATE *mstate) {
  MD.confidence = mstate->confidence;
  MD.count = mstate->count;
  MD.pref_type = mstate->pref_type;
  MD.check_keys = mstate->check_keys;
  MD.absolute_form = mstate->absolute_form;
  MD.match = mstate->match;
  MD.player = mstate->player;
  string_copy(MD.string, mstate->string);
  free_lbuf(mstate->string);
}

void init_match(MatchContext *match_context, DbRef player, char *name,
                int type) {
  MD.confidence = -1;
  MD.count = MD.check_keys = 0;
  MD.pref_type = type;
  MD.match = NOTHING;
  MD.player = player;
  MD.string = munge_space_for_match(match_context, name);
  MD.absolute_form = absolute_name(match_context, 1);
}

void init_match_check_keys(MatchContext *match_context, DbRef player,
                           char *name, int type) {
  init_match(match_context, player, name, type);
  MD.check_keys = 1;
}
