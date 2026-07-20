/*
 * match.c -- Routines for parsing arguments
 */

#include "mux/server/platform.h"

#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/world/match.h"
#include "mux/world/world_context.h"

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

#define md (*match_context)

static void promote_match(MatchContext *match_context, DbRef what,
                          int confidence) {
  LuaLockInvocation lock;
  LuaLockResult result;
  /*
   * Check for type and locks, if requested
   */

  if (md.pref_type != NOTYPE) {
    if (is_good_obj(md.evaluation->world->database, what) &&
        (typeof_obj(md.evaluation->world->database, what) == md.pref_type))
      confidence |= CON_TYPE;
  }
  if (md.check_keys) {
    MSTATE save_md;

    save_match_state(match_context, &save_md);
    if (is_good_obj(md.evaluation->world->database, what) &&
        lock_test(md.evaluation, md.player, md.player, md.player, what,
                  LUA_LOCK_DEFAULT, LUA_LOCK_OPERATION_MATCH, true, &lock,
                  &result))
      confidence |= CON_LOCK;
    restore_match_state(match_context, &save_md);
  }
  /*
   * If nothing matched, take it
   */

  if (md.count == 0) {
    md.match = what;
    md.confidence = confidence;
    md.count = 1;
    return;
  }
  /*
   * If confidence is lower, ignore
   */

  if (confidence < md.confidence) {
    return;
  }
  /*
   * If confidence is higher, replace
   */

  if (confidence > md.confidence) {
    md.match = what;
    md.confidence = confidence;
    md.count = 1;
    return;
  }
  /*
   * Equal confidence, pick randomly
   */

  if (random() % 2) {
    md.match = what;
  }
  md.count++;
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * This function removes repeated spaces from the template to which object
 * * names are being matched.  It also removes inital and terminal spaces.
 */

static char *munge_space_for_match(MatchContext *match_context, char *name) {
  char *p, *q;

  p = name;
  q = md.normalized;
  while (isspace(*p))
    p++; /*
          * remove inital spaces
          */
  while (*p) {
    while (*p && !isspace(*p))
      *q++ = *p++;
    while (*p && isspace(*++p))
      ;
    if (*p)
      *q++ = ' ';
  }
  *q = '\0'; /*
              * remove terminal spaces and terminate * * *
              *
              * * string
              */
  return md.normalized;
}

void match_player(MatchContext *match_context) {
  DbRef match;
  char *p;

  if (md.confidence >= CON_DBREF) {
    return;
  }
  if (is_good_obj(md.evaluation->world->database, md.absolute_form) &&
      is_player(md.evaluation->world->database, md.absolute_form)) {
    promote_match(match_context, md.absolute_form, CON_DBREF);
    return;
  }
  if (*md.string == LOOKUP_TOKEN) {
    for (p = md.string + 1; isspace(*p); p++)
      ;
    match = lookup_player(md.evaluation->world, NOTHING, p, 1);
    if (is_good_obj(md.evaluation->world->database, match)) {
      promote_match(match_context, match, CON_TOKEN);
    }
  }
}

/*
 * returns nnn if name = #nnn, else NOTHING
 */

static DbRef absolute_name(MatchContext *match_context, int need_pound) {
  DbRef match;
  char *mname;

  mname = md.string;
  if (need_pound) {
    if (*md.string != NUMBER_TOKEN) {
      return NOTHING;
    } else {
      mname++;
    }
  }
  match = parse_dbref(mname);
  if (is_good_obj(md.evaluation->world->database, match)) {
    return match;
  }
  return NOTHING;
}

void match_absolute(MatchContext *match_context) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.absolute_form))
    promote_match(match_context, md.absolute_form, CON_DBREF);
}

void match_numeric(MatchContext *match_context) {
  DbRef match;

  if (md.confidence >= CON_DBREF)
    return;
  match = absolute_name(match_context, 0);
  if (is_good_obj(md.evaluation->world->database, match))
    promote_match(match_context, match, CON_DBREF);
}

void match_me(MatchContext *match_context) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.absolute_form) &&
      (md.absolute_form == md.player)) {
    promote_match(match_context, md.player, CON_DBREF | CON_LOCAL);
    return;
  }
  if (!string_compare(md.evaluation->world->configuration, md.string, "me"))
    promote_match(match_context, md.player, CON_TOKEN | CON_LOCAL);
  return;
}

void match_home(MatchContext *match_context) {
  if (md.confidence >= CON_DBREF)
    return;
  if (!string_compare(md.evaluation->world->configuration, md.string, "home"))
    promote_match(match_context, HOME, CON_TOKEN);
  return;
}

void match_here(MatchContext *match_context) {
  DbRef loc;

  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.player) &&
      has_location(md.evaluation->world->database, md.player)) {
    loc = game_object_location(md.evaluation->world->database, md.player);
    if (is_good_obj(md.evaluation->world->database, loc)) {
      if (loc == md.absolute_form) {
        promote_match(match_context, loc, CON_DBREF | CON_LOCAL);
      } else if (!string_compare(md.evaluation->world->configuration, md.string,
                                 "here")) {
        promote_match(match_context, loc, CON_TOKEN | CON_LOCAL);
      } else if (!string_compare(md.evaluation->world->configuration, md.string,
                                 (char *)game_object_pure_name(
                                     md.evaluation->world->database, loc))) {
        promote_match(match_context, loc, CON_COMPLETE | CON_LOCAL);
      }
    }
  }
}

static void match_list(MatchContext *match_context, DbRef first, int local) {
  char *namebuf;

  if (md.confidence >= CON_DBREF)
    return;
  DOLIST(md.evaluation->world->database, first, first) {
    if (first == md.absolute_form) {
      promote_match(match_context, first, CON_DBREF | local);
      return;
    }
    /*
     * Warning: make sure there are no other calls to game_object_name() in
     * promote_match or its called subroutines; they
     * would overwrite game_object_name()'s static buffer which is
     * needed by string_match().
     */
    namebuf =
        (char *)game_object_pure_name(md.evaluation->world->database, first);

    if (!string_compare(md.evaluation->world->configuration, namebuf,
                        md.string)) {
      promote_match(match_context, first, CON_COMPLETE | local);
    } else if (string_match(namebuf, md.string)) {
      promote_match(match_context, first, local);
    }
  }
}

void match_possession(MatchContext *match_context) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.player) &&
      has_contents(md.evaluation->world->database, md.player))
    match_list(match_context,
               game_object_contents(md.evaluation->world->database, md.player),
               CON_LOCAL);
}

void match_neighbor(MatchContext *match_context) {
  DbRef loc;

  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.player) &&
      has_location(md.evaluation->world->database, md.player)) {
    loc = game_object_location(md.evaluation->world->database, md.player);
    if (is_good_obj(md.evaluation->world->database, loc)) {
      match_list(match_context,
                 game_object_contents(md.evaluation->world->database, loc),
                 CON_LOCAL);
    }
  }
}

static int match_exit_internal(MatchContext *match_context, DbRef loc,
                               DbRef baseloc, int local) {
  DbRef exit;
  int result, key;

  if (!is_good_obj(md.evaluation->world->database, loc) ||
      !has_exits(md.evaluation->world->database, loc))
    return 1;

  result = 0;
  DOLIST(md.evaluation->world->database, exit,
         game_object_exits(md.evaluation->world->database, loc)) {
    if (exit == md.absolute_form) {
      key = 0;
      if (is_examinable(match_context->evaluation, md.player, loc))
        key |= VE_LOC_XAM;
      if (is_dark(md.evaluation->world->database, loc))
        key |= VE_LOC_DARK;
      if (is_dark(md.evaluation->world->database, baseloc))
        key |= VE_BASE_DARK;
      if (exit_visible(match_context->evaluation, exit, md.player, key)) {
        promote_match(match_context, exit, CON_DBREF | local);
        return 1;
      }
    }
    if (matches_exit_from_list(md.string,
                               (char *)game_object_pure_name(
                                   md.evaluation->world->database, exit))) {
      promote_match(match_context, exit, CON_COMPLETE | local);
      result = 1;
    }
  }
  return result;
}

void match_exit(MatchContext *match_context) {
  DbRef loc;

  if (md.confidence >= CON_DBREF)
    return;
  loc = game_object_location(md.evaluation->world->database, md.player);
  if (is_good_obj(md.evaluation->world->database, md.player) &&
      has_location(md.evaluation->world->database, md.player))
    (void)match_exit_internal(match_context, loc, loc, CON_LOCAL);
}

void match_exit_with_parents(MatchContext *match_context) {
  DbRef loc, parent;
  int lev;

  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.player) &&
      has_location(md.evaluation->world->database, md.player)) {
    loc = game_object_location(md.evaluation->world->database, md.player);
    ITER_PARENTS(md.evaluation->world->database,
                 md.evaluation->world->configuration, loc, parent, lev) {
      if (match_exit_internal(match_context, parent, loc, CON_LOCAL))
        break;
    }
  }
}

void match_carried_exit(MatchContext *match_context) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.player) &&
      has_exits(md.evaluation->world->database, md.player))
    (void)match_exit_internal(match_context, md.player, md.player, CON_LOCAL);
}

void match_carried_exit_with_parents(MatchContext *match_context) {
  DbRef parent;
  int lev;

  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.player) &&
      (has_exits(md.evaluation->world->database, md.player) ||
       is_room(md.evaluation->world->database, md.player))) {
    ITER_PARENTS(md.evaluation->world->database,
                 md.evaluation->world->configuration, md.player, parent, lev) {
      if (match_exit_internal(match_context, parent, md.player, CON_LOCAL))
        break;
    }
  }
}

void match_master_exit(MatchContext *match_context) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.player) &&
      has_exits(md.evaluation->world->database, md.player))
    (void)match_exit_internal(
        match_context, md.evaluation->world->configuration->master_room,
        md.evaluation->world->configuration->master_room, 0);
}

void match_zone_exit(MatchContext *match_context) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.evaluation->world->database, md.player) &&
      has_exits(md.evaluation->world->database, md.player))
    (void)match_exit_internal(
        match_context,
        game_object_zone(md.evaluation->world->database, md.player),
        game_object_zone(md.evaluation->world->database, md.player), 0);
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
  if (md.confidence >= CON_TOKEN)
    return;

  if (!(key & MAT_NO_EXITS)) {
    if (key & MAT_EXIT_PARENTS) {
      match_carried_exit_with_parents(match_context);
      match_exit_with_parents(match_context);
    } else {
      match_carried_exit(match_context);
      match_exit(match_context);
    }
  }
  match_neighbor(match_context);
  match_possession(match_context);
}

DbRef match_result(MatchContext *match_context) {
  switch (md.count) {
  case 0:
    return NOTHING;
  case 1:
    return md.match;
  default:
    return AMBIGUOUS;
  }
}

/*
 * use this if you don't care about ambiguity
 */

DbRef last_match_result(MatchContext *match_context) { return md.match; }

DbRef match_status(EvaluationContext *evaluation, DbRef player, DbRef match) {
  switch (match) {
  case NOTHING:
    notify(evaluation, player, NOMATCH_MESSAGE);
    return NOTHING;
  case AMBIGUOUS:
    notify(evaluation, player, AMBIGUOUS_MESSAGE);
    return NOTHING;
  case NOPERM:
    notify(evaluation, player, NOPERM_MESSAGE);
    return NOTHING;
  default:
    break;
  }
  if (is_good_obj(evaluation->world->database, match) &&
      is_dark(evaluation->world->database, match) &&
      is_good_obj(evaluation->world->database, player) &&
      !is_wizard(evaluation->world->database,
                 game_object_owner(evaluation->world->database, player)))
    return match_status(evaluation, player, NOTHING);
  return match;
}

DbRef noisy_match_result(MatchContext *match_context) {
  return match_status(match_context->evaluation, md.player,
                      match_result(match_context));
}

void save_match_state(MatchContext *match_context, MSTATE *mstate) {
  mstate->confidence = md.confidence;
  mstate->count = md.count;
  mstate->pref_type = md.pref_type;
  mstate->check_keys = md.check_keys;
  mstate->absolute_form = md.absolute_form;
  mstate->match = md.match;
  mstate->player = md.player;
  mstate->string = alloc_lbuf("save_match_state");
  StringCopy(mstate->string, md.string);
}

void restore_match_state(MatchContext *match_context, MSTATE *mstate) {
  md.confidence = mstate->confidence;
  md.count = mstate->count;
  md.pref_type = mstate->pref_type;
  md.check_keys = mstate->check_keys;
  md.absolute_form = mstate->absolute_form;
  md.match = mstate->match;
  md.player = mstate->player;
  StringCopy(md.string, mstate->string);
  free_lbuf(mstate->string);
}

void init_match(MatchContext *match_context, DbRef player, char *name,
                int type) {
  md.confidence = -1;
  md.count = md.check_keys = 0;
  md.pref_type = type;
  md.match = NOTHING;
  md.player = player;
  md.string = munge_space_for_match(match_context, (char *)name);
  md.absolute_form = absolute_name(match_context, 1);
}

void init_match_check_keys(MatchContext *match_context, DbRef player,
                           char *name, int type) {
  init_match(match_context, player, name, type);
  md.check_keys = 1;
}
