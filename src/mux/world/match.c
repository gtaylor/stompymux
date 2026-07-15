/*
 * match.c -- Routines for parsing arguments
 */

#include "mux/server/platform.h"

#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/world/match.h"

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

static MSTATE md;

static void promote_match(DbRef what, int confidence) {
  /*
   * Check for type and locks, if requested
   */

  if (md.pref_type != NOTYPE) {
    if (is_good_obj(what) && (typeof_obj(what) == md.pref_type))
      confidence |= CON_TYPE;
  }
  if (md.check_keys) {
    MSTATE save_md;

    save_match_state(&save_md);
    if (is_good_obj(what) && could_doit(md.player, what, A_LOCK))
      confidence |= CON_LOCK;
    restore_match_state(&save_md);
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

static char *munge_space_for_match(char *name) {
  static char buffer[LBUF_SIZE];
  char *p, *q;

  p = name;
  q = buffer;
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
  return (buffer);
}

void match_player(void) {
  DbRef match;
  char *p;

  if (md.confidence >= CON_DBREF) {
    return;
  }
  if (is_good_obj(md.absolute_form) && is_player(md.absolute_form)) {
    promote_match(md.absolute_form, CON_DBREF);
    return;
  }
  if (*md.string == LOOKUP_TOKEN) {
    for (p = md.string + 1; isspace(*p); p++)
      ;
    match = lookup_player(NOTHING, p, 1);
    if (is_good_obj(match)) {
      promote_match(match, CON_TOKEN);
    }
  }
}

/*
 * returns nnn if name = #nnn, else NOTHING
 */

static DbRef absolute_name(int need_pound) {
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
  if (is_good_obj(match)) {
    return match;
  }
  return NOTHING;
}

void match_absolute(void) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.absolute_form))
    promote_match(md.absolute_form, CON_DBREF);
}

void match_numeric(void) {
  DbRef match;

  if (md.confidence >= CON_DBREF)
    return;
  match = absolute_name(0);
  if (is_good_obj(match))
    promote_match(match, CON_DBREF);
}

void match_me(void) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.absolute_form) && (md.absolute_form == md.player)) {
    promote_match(md.player, CON_DBREF | CON_LOCAL);
    return;
  }
  if (!string_compare(md.string, "me"))
    promote_match(md.player, CON_TOKEN | CON_LOCAL);
  return;
}

void match_home(void) {
  if (md.confidence >= CON_DBREF)
    return;
  if (!string_compare(md.string, "home"))
    promote_match(HOME, CON_TOKEN);
  return;
}

void match_here(void) {
  DbRef loc;

  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.player) && has_location(md.player)) {
    loc = obj_location(md.player);
    if (is_good_obj(loc)) {
      if (loc == md.absolute_form) {
        promote_match(loc, CON_DBREF | CON_LOCAL);
      } else if (!string_compare(md.string, "here")) {
        promote_match(loc, CON_TOKEN | CON_LOCAL);
      } else if (!string_compare(md.string, (char *)PureName(loc))) {
        promote_match(loc, CON_COMPLETE | CON_LOCAL);
      }
    }
  }
}

static void match_list(DbRef first, int local) {
  char *namebuf;

  if (md.confidence >= CON_DBREF)
    return;
  DOLIST(first, first) {
    if (first == md.absolute_form) {
      promote_match(first, CON_DBREF | local);
      return;
    }
    /*
     * Warning: make sure there are no other calls to Name() in
     * promote_match or its called subroutines; they
     * would overwrite Name()'s static buffer which is
     * needed by string_match().
     */
    namebuf = (char *)PureName(first);

    if (!string_compare(namebuf, md.string)) {
      promote_match(first, CON_COMPLETE | local);
    } else if (string_match(namebuf, md.string)) {
      promote_match(first, local);
    }
  }
}

void match_possession(void) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.player) && has_contents(md.player))
    match_list(obj_contents(md.player), CON_LOCAL);
}

void match_neighbor(void) {
  DbRef loc;

  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.player) && has_location(md.player)) {
    loc = obj_location(md.player);
    if (is_good_obj(loc)) {
      match_list(obj_contents(loc), CON_LOCAL);
    }
  }
}

static int match_exit_internal(DbRef loc, DbRef baseloc, int local) {
  DbRef exit;
  int result, key;

  if (!is_good_obj(loc) || !has_exits(loc))
    return 1;

  result = 0;
  DOLIST(exit, obj_exits(loc)) {
    if (exit == md.absolute_form) {
      key = 0;
      if (is_examinable(md.player, loc))
        key |= VE_LOC_XAM;
      if (is_dark(loc))
        key |= VE_LOC_DARK;
      if (is_dark(baseloc))
        key |= VE_BASE_DARK;
      if (exit_visible(exit, md.player, key)) {
        promote_match(exit, CON_DBREF | local);
        return 1;
      }
    }
    if (matches_exit_from_list(md.string, (char *)PureName(exit))) {
      promote_match(exit, CON_COMPLETE | local);
      result = 1;
    }
  }
  return result;
}

void match_exit(void) {
  DbRef loc;

  if (md.confidence >= CON_DBREF)
    return;
  loc = obj_location(md.player);
  if (is_good_obj(md.player) && has_location(md.player))
    (void)match_exit_internal(loc, loc, CON_LOCAL);
}

void match_exit_with_parents(void) {
  DbRef loc, parent;
  int lev;

  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.player) && has_location(md.player)) {
    loc = obj_location(md.player);
    ITER_PARENTS(loc, parent, lev) {
      if (match_exit_internal(parent, loc, CON_LOCAL))
        break;
    }
  }
}

void match_carried_exit(void) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.player) && has_exits(md.player))
    (void)match_exit_internal(md.player, md.player, CON_LOCAL);
}

void match_carried_exit_with_parents(void) {
  DbRef parent;
  int lev;

  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.player) && (has_exits(md.player) || is_room(md.player))) {
    ITER_PARENTS(md.player, parent, lev) {
      if (match_exit_internal(parent, md.player, CON_LOCAL))
        break;
    }
  }
}

void match_master_exit(void) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.player) && has_exits(md.player))
    (void)match_exit_internal(mudconf.master_room, mudconf.master_room, 0);
}

void match_zone_exit(void) {
  if (md.confidence >= CON_DBREF)
    return;
  if (is_good_obj(md.player) && has_exits(md.player))
    (void)match_exit_internal(obj_zone(md.player), obj_zone(md.player), 0);
}

void match_everything(int key) {
  /*
   * Try matching me, then here, then absolute, then player FIRST, since
   * this will hit most cases. STOP if we get something, since those are
   * exact matches.
   */

  match_me();
  match_here();
  match_absolute();
  if (key & MAT_NUMERIC)
    match_numeric();
  if (key & MAT_HOME)
    match_home();
  match_player();
  if (md.confidence >= CON_TOKEN)
    return;

  if (!(key & MAT_NO_EXITS)) {
    if (key & MAT_EXIT_PARENTS) {
      match_carried_exit_with_parents();
      match_exit_with_parents();
    } else {
      match_carried_exit();
      match_exit();
    }
  }
  match_neighbor();
  match_possession();
}

DbRef match_result(void) {
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

DbRef last_match_result(void) { return md.match; }

DbRef match_status(DbRef player, DbRef match) {
  switch (match) {
  case NOTHING:
    notify(player, NOMATCH_MESSAGE);
    return NOTHING;
  case AMBIGUOUS:
    notify(player, AMBIGUOUS_MESSAGE);
    return NOTHING;
  case NOPERM:
    notify(player, NOPERM_MESSAGE);
    return NOTHING;
  }
  if (is_good_obj(match) && is_dark(match) && is_good_obj(player) &&
      !is_wizard(obj_owner(player)))
    return match_status(player, NOTHING);
  return match;
}

DbRef noisy_match_result(void) {
  return match_status(md.player, match_result());
}

void save_match_state(MSTATE *mstate) {
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

void restore_match_state(MSTATE *mstate) {
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

void init_match(DbRef player, char *name, int type) {
  md.confidence = -1;
  md.count = md.check_keys = 0;
  md.pref_type = type;
  md.match = NOTHING;
  md.player = player;
  md.string = munge_space_for_match((char *)name);
  md.absolute_form = absolute_name(1);
}

void init_match_check_keys(DbRef player, char *name, int type) {
  init_match(player, name, type);
  md.check_keys = 1;
}
