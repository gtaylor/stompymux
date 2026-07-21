/*
 * flags.c - flag manipulation routines
 */

#include "mux/server/platform.h"

#include "p.glue.h"

#include "mux/commands/command.h"
#include "mux/commands/command_runtime.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/world/world_context.h"

bool is_good_obj(GameDatabase *database, DbRef x) {
  return x >= 0 && x < database->top && typeof_obj(database, x) < NOTYPE;
}

bool is_safe(GameDatabase *database, const ServerConfiguration *configuration,
             DbRef x, DbRef p) {
  return is_owns_others(database, x) ||
         (game_object_flags(database, x) & SAFE) ||
         (configuration->safe_unowned &&
          (game_object_owner(database, x) != game_object_owner(database, p)));
}

bool is_examinable(EvaluationContext *evaluation, DbRef p, DbRef x) {
  GameDatabase *database = evaluation->world->database;
  return is_wizard(database, p) ||
         (game_object_owner(database, p) == game_object_owner(database, x)) ||
         is_on_enter_lock(evaluation, p, x);
}

bool is_myopic_exam(EvaluationContext *evaluation, DbRef p, DbRef x) {
  GameDatabase *database = evaluation->world->database;
  return !is_myopic(database, p) &&
         (is_wizard(database, p) ||
          (game_object_owner(database, p) == game_object_owner(database, x)) ||
          is_on_enter_lock(evaluation, p, x));
}

bool is_controls(EvaluationContext *evaluation, DbRef p, DbRef x) {
  GameDatabase *database = evaluation->world->database;
  return is_good_obj(database, x) &&
         !(is_god(database, x) && !is_god(database, p)) &&
         (is_wizard(database, p) ||
          ((game_object_owner(database, p) == game_object_owner(database, x)) &&
           (is_inherits(database, p) || !is_inherits(database, x))) ||
          is_on_enter_lock(evaluation, p, x));
}

bool can_link_exit(EvaluationContext *evaluation, DbRef p, DbRef x) {
  GameDatabase *database = evaluation->world->database;
  return typeof_obj(database, x) == TYPE_EXIT &&
         (game_object_location(database, x) == NOTHING ||
          is_controls(evaluation, p, x));
}

bool is_linkable(EvaluationContext *evaluation, DbRef p, DbRef x) {
  GameDatabase *database = evaluation->world->database;
  return is_good_obj(database, x) && has_contents(database, x) &&
         is_controls(evaluation, p, x);
}

void mark(GameDatabase *database, DbRef x) {
  const unsigned char mask = (unsigned char)(1U << (x & 7));
  database->markbits->chunk[x >> 3] =
      (char)((unsigned char)database->markbits->chunk[x >> 3] | mask);
}
void unmark(GameDatabase *database, DbRef x) {
  const unsigned char mask = (unsigned char)(1U << (x & 7));
  database->markbits->chunk[x >> 3] =
      (char)((unsigned char)database->markbits->chunk[x >> 3] &
             (unsigned char)~mask);
}
bool is_marked(GameDatabase *database, DbRef x) {
  const unsigned char mask = (unsigned char)(1U << (x & 7));
  return ((unsigned char)database->markbits->chunk[x >> 3] & mask) != 0;
}
void unmark_all(GameDatabase *database) {
  DbRef i;

  for (i = 0; i < ((database->top + 7) >> 3); i++)
    database->markbits->chunk[i] = 0x0;
}

bool see_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
              DbRef o, long f) {
  (void)x;
  (void)a;
  (void)o;
  (void)f;
  return is_wizard(evaluation->world->database, p);
}
bool see_attr_explicit(GameDatabase *database, DbRef p, DbRef x, Attribute *a,
                       DbRef o, long f) {
  (void)x;
  (void)a;
  (void)o;
  (void)f;
  return is_wizard(database, p);
}
bool set_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
              long f) {
  (void)x;
  (void)a;
  (void)f;
  return is_wizard(evaluation->world->database, p);
}
bool read_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
               DbRef o, long f) {
  return see_attr(evaluation, p, x, a, o, f);
}
bool write_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
                long f) {
  return set_attr(evaluation, p, x, a, f);
}

/**
 * Sets or clears the indicated bit, no security checking.
 * @param target The target object for setting/unsetting
 * @param player The object who is setting/unsetting
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_any(EvaluationContext *evaluation, DbRef target, DbRef player,
                  Flag flag, int fflags, int reset) {
  (void)evaluation;
  if (fflags & FLAG_WORD3) {
    if (reset)
      game_object_set_flags3(
          evaluation->world->database, target,
          game_object_flags3(evaluation->world->database, target) & ~flag);
    else
      game_object_set_flags3(
          evaluation->world->database, target,
          game_object_flags3(evaluation->world->database, target) | flag);
  } else if (fflags & FLAG_WORD2) {
    if (reset)
      game_object_set_flags2(
          evaluation->world->database, target,
          game_object_flags2(evaluation->world->database, target) & ~flag);
    else
      game_object_set_flags2(
          evaluation->world->database, target,
          game_object_flags2(evaluation->world->database, target) | flag);
  } else {
    if (reset)
      game_object_set_flags(
          evaluation->world->database, target,
          game_object_flags(evaluation->world->database, target) & ~flag);
    else
      game_object_set_flags(
          evaluation->world->database, target,
          game_object_flags(evaluation->world->database, target) | flag);
  }
  return 1;
} /* end fh_and(evaluation, ) */

/**
 * Function to block out non-GOD for setting or clearing a bit.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_god(EvaluationContext *evaluation, DbRef target, DbRef player,
                  Flag flag, int fflags, int reset) {
  if (!is_god(evaluation->world->database, player))
    return 0;

  return (fh_any(evaluation, target, player, flag, fflags, reset));
} /* end fh_god(evaluation, ) */

/**
 * Blocks out non-WIZARDS setting or clearing a bit.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_wiz(EvaluationContext *evaluation, DbRef target, DbRef player,
                  Flag flag, int fflags, int reset) {
  if (!is_wizard(evaluation->world->database, player) &&
      !is_god(evaluation->world->database, player))
    return 0;

  return (fh_any(evaluation, target, player, flag, fflags, reset));
} /* end fh_wiz(evaluation, ) */

/**
 * Only allows the bit to be set on players by WIZARDS.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_fixed(EvaluationContext *evaluation, DbRef target, DbRef player,
                    Flag flag, int fflags, int reset) {
  if (is_player(evaluation->world->database, target))
    if (!is_wizard(evaluation->world->database, player) &&
        !is_god(evaluation->world->database, player))
      return 0;

  return (fh_any(evaluation, target, player, flag, fflags, reset));
} /* end fh_fixed(evaluation, ) */

/**
 * Only allows players to set or clear this bit.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_inherit(EvaluationContext *evaluation, DbRef target, DbRef player,
                      Flag flag, int fflags, int reset) {
  if (!is_inherits(evaluation->world->database, player))
    return 0;

  return (fh_any(evaluation, target, player, flag, fflags, reset));
} /* end fh_inherit(evaluation, ) */

/**
 * Only allows GOD to set/clear this bit. Used for WIZARD flag.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_wiz_bit(EvaluationContext *evaluation, DbRef target, DbRef player,
                      Flag flag, int fflags, int reset) {
  if (!is_god(evaluation->world->database, player))
    return 0;
  if (is_god(evaluation->world->database, target) && reset) {
    notify(evaluation, player, "You cannot make yourself mortal.");
    return 0;
  }

  return (fh_any(evaluation, target, player, flag, fflags, reset));
} /* end fh_wiz_bit(evaluation, ) */

/**
 * Manipulates the dark bit. Only Wizards may set it on players.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_dark_bit(EvaluationContext *evaluation, DbRef target,
                       DbRef player, Flag flag, int fflags, int reset) {
  if (!reset && is_player(evaluation->world->database, target) &&
      !is_wizard(evaluation->world->database, player))
    return 0;

  return (fh_any(evaluation, target, player, flag, fflags, reset));
} /* end fh_dark_bit(evaluation, ) */

/**
 * Manipulates the going bit.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_going_bit(EvaluationContext *evaluation, DbRef target,
                        DbRef player, Flag flag, int fflags, int reset) {
  if (is_going(evaluation->world->database, target) && reset &&
      (typeof_obj(evaluation->world->database, target) != TYPE_GARBAGE)) {
    notify(evaluation, player, "Your object has been spared from destruction.");
    return (fh_any(evaluation, target, player, flag, fflags, reset));
  }

  if (!is_god(evaluation->world->database, player))
    return 0;

  return (fh_any(evaluation, target, player, flag, fflags, reset));
} /* end fh_going_bit(evaluation, ) */

/**
 * Sets or clears bits that affect hearing.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_hear_bit(EvaluationContext *evaluation, DbRef target,
                       DbRef player, Flag flag, int fflags, int reset) {
  int could_hear;

  if (is_player(evaluation->world->database, target) && (flag & MONITOR)) {
    if (is_wizard(evaluation->world->database, player))
      fh_any(evaluation, target, player, flag, fflags, reset);
    else
      return 0;
  }

  could_hear = is_hearer(evaluation, target);
  fh_any(evaluation, target, player, flag, fflags, reset);
  handle_ears(evaluation, target, could_hear, is_hearer(evaluation, target));

  return 1;
} /* end fh_hear_bit(evaluation, ) */

/**
 * Sets or clears bits that affect xcode in glue.h.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_xcode_bit(EvaluationContext *evaluation, DbRef target,
                        DbRef player, Flag flag, int fflags, int reset) {
  int got_xcode;
  int new_xcode;

  got_xcode = is_hardcode(evaluation->world->database, target);
  fh_wiz(evaluation, target, player, flag, fflags, reset);
  new_xcode = is_hardcode(evaluation->world->database, target);
  handle_xcode(evaluation->btech, player, target, got_xcode, new_xcode);

  return 1;
} /* end fh_xcode_bit(evaluation, ) */

/**
 * Alphabetized flag listing
 * 0 = Flag's visible name
 * 1 = Flag's bit representation
 * 2 = Flag's letter alias
 * 3 = Flag's wordspace
 * 4 = Who may see the flag (0 = all)
 * 5 = Permissions
 */
FLAGENT gen_flags[] = {
    {"ANSI", ANSI, 'X', FLAG_WORD2, 0, fh_any},
    {"ANSIMAP", ANSIMAP, 'P', FLAG_WORD2, 0, fh_any},
    {"AUDIBLE", HEARTHRU, 'a', 0, 0, fh_hear_bit},
    {"AUDITORIUM", AUDITORIUM, 'b', FLAG_WORD2, 0, fh_any},
    {"CONNECTED", CONNECTED, 'c', FLAG_WORD2, 0, fh_god},
    {"DARK", DARK, 'D', 0, 0, fh_dark_bit},
    {"ENTER_OK", ENTER_OK, 'e', 0, 0, fh_any},
    {"FIXED", FIXED, 'f', FLAG_WORD2, 0, fh_fixed},
    {"FLOATING", FLOATING, 'F', FLAG_WORD2, 0, fh_any},
    {"GAGGED", GAGGED, 'j', FLAG_WORD2, 0, fh_wiz},
    {"GOING", GOING, 'G', 0, 0, fh_going_bit},
    {"HALTED", HALT, 'h', 0, 0, fh_any},
    {"BLIND", BLIND, '(', FLAG_WORD2, 0, fh_any},
    {"IN_CHARACTER", IN_CHARACTER, '#', FLAG_WORD2, 0, fh_wiz},
    {"INHERIT", INHERIT, 'I', 0, 0, fh_inherit},
    {"KEY", KEY, 'K', FLAG_WORD2, 0, fh_any},
    {"LIGHT", LIGHT, 'l', FLAG_WORD2, 0, fh_any},
    {"MONITOR", MONITOR, 'M', 0, 0, fh_hear_bit},
    {"MYOPIC", MYOPIC, 'm', 0, 0, fh_any},
    {"NOBLEED", NOBLEED, '-', FLAG_WORD2, 0, fh_any},
    {"NO_COMMAND", NO_COMMAND, 'n', FLAG_WORD2, 0, fh_any},
    {"NOSPOOF", NOSPOOF, 'N', 0, 0, fh_any},
    {"OPAQUE", OPAQUE, 'O', 0, 0, fh_any},
    {"PUPPET", PUPPET, 'p', 0, 0, fh_hear_bit},
    {"QUIET", QUIET, 'Q', 0, 0, fh_any},
    {"SAFE", SAFE, 's', 0, 0, fh_any},
    {"STICKY", STICKY, 'S', 0, 0, fh_wiz},
    {"SUSPECT", SUSPECT, 'u', FLAG_WORD2, CA_WIZARD, fh_wiz},
    {"TRACE", TRACE, 'T', 0, 0, fh_any},
    {"TRANSPARENT", SEETHRU, 't', 0, 0, fh_any},
    {"UNFINDABLE", UNFINDABLE, 'U', FLAG_WORD2, 0, fh_any},
    {"VERBOSE", VERBOSE, 'v', 0, 0, fh_any},
    {"WIZARD", WIZARD, 'W', 0, 0, fh_wiz_bit},
    {"XCODE", HARDCODE, 'X', FLAG_WORD2, 0, fh_xcode_bit},
    {"ZOMBIE", ZOMBIE, 'z', FLAG_WORD2, CA_WIZARD, fh_wiz},
    {nullptr, 0, ' ', 0, 0, nullptr}};

/**
 * Listing of valid object types
 */
OBJENT object_types[8] = {
    {"ROOM", 'R', CA_PUBLIC, OF_CONTENTS | OF_EXITS | OF_DROPTO | OF_HOME},
    {"THING", ' ', CA_PUBLIC,
     OF_CONTENTS | OF_LOCATION | OF_EXITS | OF_HOME | OF_SIBLINGS},
    {"EXIT", 'E', CA_PUBLIC, OF_SIBLINGS},
    {"PLAYER", 'P', CA_PUBLIC,
     OF_CONTENTS | OF_LOCATION | OF_EXITS | OF_HOME | OF_OWNER | OF_SIBLINGS},
    {"TYPE5", '+', CA_GOD, 0},
    {"GARBAGE", '-', CA_PUBLIC,
     OF_CONTENTS | OF_LOCATION | OF_EXITS | OF_HOME | OF_SIBLINGS},
    {"GARBAGE", '#', CA_GOD, 0}};

/**
 * Initializes flag hash tables.
 */
void init_flagtab(WorldIndexes *indexes) {
  FLAGENT *fp;
  char *nbuf, *np;
  const char *bp;

  hash_table_initialize(&indexes->flags, 100 * HASH_FACTOR);
  nbuf = alloc_sbuf("init_flagtab");

  for (fp = gen_flags; fp->flagname; fp++) {
    for (np = nbuf, bp = fp->flagname; *bp; np++, bp++)
      *np = ToLower(*bp);
    *np = '\0';
    hash_table_add(nbuf, (int *)fp, &indexes->flags);
  }

  free_sbuf(nbuf);
} /* end init_flagtab() */

/**
 * Displays available flags. Used in @list flags.
 */
void display_flagtab(EvaluationContext *evaluation, DbRef player) {
  char *buf, *bp;
  FLAGENT *fp;

  bp = buf = alloc_lbuf("display_flagtab");
  safe_str("Flags:", buf, &bp);

  for (fp = gen_flags; fp->flagname; fp++) {
    if ((fp->listperm & CA_WIZARD) &&
        !is_wizard(evaluation->world->database, player))
      continue;
    if ((fp->listperm & CA_GOD) && !is_god(evaluation->world->database, player))
      continue;
    safe_chr(' ', buf, &bp);
    safe_str(fp->flagname, buf, &bp);
    safe_chr('(', buf, &bp);
    safe_chr(fp->flaglett, buf, &bp);
    safe_chr(')', buf, &bp);
  }

  *bp = '\0';
  notify(evaluation, player, buf);
  free_lbuf(buf);
} /* end display_flagtab() */

/**
 * ??
 */
FLAGENT *find_flag(WorldIndexes *indexes, DbRef thing, char *flagname) {
  char *cp;

  /* Make sure the flag name is valid */
  for (cp = flagname; *cp; cp++)
    *cp = ToLower(*cp);

  return (FLAGENT *)hash_table_find(flagname, &indexes->flags);
} /* end find_flag() */

/**
 * Sets or clears a specified flag on an object.
 * @param target Target object
 * @param player The object doing the setting
 * @paran flag The flag being set/unset
 * @param key Are we @set/quiet'in?
 */
void flag_set(EvaluationContext *evaluation, WorldIndexes *indexes,
              DbRef target, DbRef player, char *flag, int key) {
  FLAGENT *fp;
  int negate, result;

  /*
   * Trim spaces, and handle the negation character
   */

  negate = 0;
  while (*flag && isspace(*flag))
    flag++;
  if (*flag == '!') {
    negate = 1;
    flag++;
  }
  while (*flag && isspace(*flag))
    flag++;

  /*
   * Make sure a flag name was specified
   */

  if (*flag == '\0') {
    if (negate)
      notify(evaluation, player, "You must specify a flag to clear.");
    else
      notify(evaluation, player, "You must specify a flag to set.");
    return;
  }
  fp = find_flag(indexes, target, flag);
  if (fp == nullptr) {
    notify(evaluation, player, "I don't understand that flag.");
    return;
  }
  /*
   * Invoke the flag handler, and print feedback
   */

  result = fp->handler(evaluation, target, player, fp->flagvalue, fp->flagflag,
                       negate);
  if (!result)
    notify(evaluation, player, "Permission denied.");
  else if (!(key & SET_QUIET) && !is_quiet(evaluation->world->database, player))
    notify_printf(evaluation, player, "%s - %s %s",
                  game_object_name(evaluation->world->database, target),
                  fp->flagname, negate ? "cleared." : "set.");
  return;
} /* end flag_set() */

/**
 * Converts a flags word into corresponding letters.
 * @param player The invoking object
 * @param flagword ??
 * @param flag2word ??
 * @param flag3word ??
 */
char *decode_flags(GameDatabase *database, DbRef player, Flag flagword,
                   Flag flag2word, Flag flag3word) {
  char *buf, *bp;
  FLAGENT *fp;
  int flagtype;
  Flag fv;

  buf = bp = alloc_sbuf("decode_flags");
  *bp = '\0';

  if (!is_good_obj(database, player)) {
    StringCopy(buf, "#-2 ERROR");
    return buf;
  }

  flagtype = (int)(flagword & TYPE_MASK);
  if (object_types[flagtype].lett != ' ')
    safe_sb_chr(object_types[flagtype].lett, buf, &bp);

  for (fp = gen_flags; fp->flagname; fp++) {
    if (fp->flagflag & FLAG_WORD3)
      fv = flag3word;
    else if (fp->flagflag & FLAG_WORD2)
      fv = flag2word;
    else
      fv = flagword;
    if (fv & fp->flagvalue) {
      if ((fp->listperm & CA_WIZARD) && !is_wizard(database, player))
        continue;
      if ((fp->listperm & CA_GOD) && !is_god(database, player))
        continue;
      /*
       * don't show CONNECT on dark wizards to mortals
       */
      if ((flagtype == TYPE_PLAYER) && (fp->flagvalue == CONNECTED) &&
          ((flagword & (WIZARD | DARK)) == (WIZARD | DARK)) &&
          !is_wizard(database, player))
        continue;
      safe_sb_chr(fp->flaglett, buf, &bp);
    }
  }

  *bp = '\0';
  return buf;
} /* end decode_flags() */

/**
 * Does object have flag visible to player?
 * @param player The player we're looking for
 * @param target The object with the flag
 * @param flagname The flag in question
 */
int has_flag(WorldContext *world, DbRef player, DbRef target, char *flagname) {
  FLAGENT *fp;
  Flag fv;

  fp = find_flag(world->indexes, target, flagname);
  if (fp == nullptr)
    return 0;

  if (fp->flagflag & FLAG_WORD3)
    fv = game_object_flags3(world->database, target);
  else if (fp->flagflag & FLAG_WORD2)
    fv = game_object_flags2(world->database, target);
  else
    fv = game_object_flags(world->database, target);

  if (fv & fp->flagvalue) {
    if ((fp->listperm & CA_WIZARD) && !is_wizard(world->database, player))
      return 0;
    if ((fp->listperm & CA_GOD) && !is_god(world->database, player))
      return 0;
    /*
     * don't show CONNECT on dark wizards to mortals
     */
    if (is_player(world->database, target) && (fp->flagvalue == CONNECTED) &&
        ((game_object_flags(world->database, target) & (WIZARD | DARK)) ==
         (WIZARD | DARK)) &&
        !is_wizard(world->database, player))
      return 0;
    return 1;
  }
  return 0;
} /* end has_flag() */

/**
 * Returns an mbuf containing the type and flags on thing.
 * @param player The player to send to
 * @param target The object whose flags we're checking
 */
char *flag_description(GameDatabase *database, DbRef player, DbRef target) {
  char *buff, *bp;
  FLAGENT *fp;
  int otype;
  Flag fv;

  /*
   * Allocate the return buffer
   */

  otype = typeof_obj(database, target);
  bp = buff = alloc_mbuf("flag_description");

  /*
   * Store the header strings and object type
   */

  safe_mb_str("Type: ", buff, &bp);
  safe_mb_str(object_types[otype].name, buff, &bp);
  safe_mb_str(" Flags:", buff, &bp);
  if (object_types[otype].perm != CA_PUBLIC) {
    *bp = '\0';
    return buff;
  }
  /*
   * Store the type-invariant flags
   */

  for (fp = gen_flags; fp->flagname; fp++) {
    if (fp->flagflag & FLAG_WORD3)
      fv = game_object_flags3(database, target);
    else if (fp->flagflag & FLAG_WORD2)
      fv = game_object_flags2(database, target);
    else
      fv = game_object_flags(database, target);
    if (fv & fp->flagvalue) {
      if ((fp->listperm & CA_WIZARD) && !is_wizard(database, player))
        continue;
      if ((fp->listperm & CA_GOD) && !is_god(database, player))
        continue;
      /*
       * don't show CONNECT on dark wizards to mortals
       */
      if (is_player(database, target) && (fp->flagvalue == CONNECTED) &&
          ((game_object_flags(database, target) & (WIZARD | DARK)) ==
           (WIZARD | DARK)) &&
          !is_wizard(database, player))
        continue;
      safe_mb_chr(' ', buff, &bp);
      safe_mb_str(fp->flagname, buff, &bp);
    }
  }

  /*
   * Terminate the string, and return the buffer to the caller
   */

  *bp = '\0';
  return buff;
} /* end flag_description() */

/**
 * Returns an lbuf containing the name and number of an object.
 * @param target The target object
 */
char *unparse_object_numonly(GameDatabase *database, DbRef target) {
  char *buf;

  buf = alloc_lbuf("unparse_object_numonly");
  if (target == NOTHING) {
    StringCopy(buf, "*NOTHING*");
  } else if (target == HOME) {
    StringCopy(buf, "*HOME*");
  } else if (!is_good_obj(database, target)) {
    snprintf(buf, LBUF_SIZE, "*ILLEGAL*(#%ld)", target);
  } else {
    snprintf(buf, LBUF_SIZE, "%s(#%ld)", game_object_name(database, target),
             target);
  }
  return buf;
} /* end unparse_object_numonly() */

/**
 * Returns an lbuf pointing to the object name and possibly the db# and flags.
 */
char *unparse_object(GameDatabase *database, EvaluationContext *evaluation,
                     DbRef player, DbRef target, int obey_myopic) {
  char *buf, *fp;
  int exam;

  buf = alloc_lbuf("unparse_object");
  if (target == NOTHING) {
    StringCopy(buf, "*NOTHING*");
  } else if (target == HOME) {
    StringCopy(buf, "*HOME*");
  } else if (!is_good_obj(database, target)) {
    snprintf(buf, LBUF_SIZE, "*ILLEGAL*(#%ld)", target);
  } else {
    if (obey_myopic)
      exam = is_myopic_exam(evaluation, player, target);
    else
      exam = is_examinable(evaluation, player, target);
    if (exam) {

      /*
       * show everything
       */
      fp = unparse_flags(database, player, target);
      snprintf(buf, LBUF_SIZE, "%s(#%ld%s%s)",
               game_object_name(database, target), target, *fp ? ":" : "", fp);
      free_sbuf(fp);
    } else {
      /*
       * show only the name.
       */
      StringCopy(buf, game_object_name(database, target));
    }
  }
  return buf;
} /* end unparse_object() */

/**
 * Converts a list of flag letters into its bit pattern.
 * Also set the type qualifier if specified and not already set.
 * @param player The evoking object
 * @param flaglist The list of flags to conver to bit pattern
 * @param fset ??
 * @param p_type ??
 */
int convert_flags(EvaluationContext *evaluation, DbRef player, char *flaglist,
                  FLAGSET *fset, Flag *p_type) {
  int i, handled;
  char *s;
  Flag flag1mask, flag2mask, flag3mask, type;
  FLAGENT *fp;

  flag1mask = flag2mask = flag3mask = 0;
  type = NOTYPE;

  for (s = flaglist; *s; s++) {
    handled = 0;

    /*
     * Check for object type
     */

    for (i = 0; (i <= 7) && !handled; i++) {
      if ((object_types[i].lett == *s) &&
          !(((object_types[i].perm & CA_WIZARD) &&
             !is_wizard(evaluation->world->database, player)) ||
            ((object_types[i].perm & CA_GOD) &&
             !is_god(evaluation->world->database, player)))) {
        if ((type != NOTYPE) && (type != i)) {
          notify_printf(evaluation, player,
                        "%c: Conflicting type specifications.", *s);
          return 0;
        }
        type = i;
        handled = 1;
      }
    }

    /*
     * Check generic flags
     */

    if (handled)
      continue;
    for (fp = gen_flags; (fp->flagname) && !handled; fp++) {
      if ((fp->flaglett == *s) &&
          !(((fp->listperm & CA_WIZARD) &&
             !is_wizard(evaluation->world->database, player)) ||
            ((fp->listperm & CA_GOD) &&
             !is_god(evaluation->world->database, player)))) {
        if (fp->flagflag & FLAG_WORD3)
          flag3mask |= fp->flagvalue;
        else if (fp->flagflag & FLAG_WORD2)
          flag2mask |= fp->flagvalue;
        else
          flag1mask |= fp->flagvalue;
        handled = 1;
      }
    }

    if (!handled) {
      notify_printf(evaluation, player,
                    "%c: Flag unknown or not valid for specified object type",
                    *s);
      return 0;
    }
  }

  /*
   * return flags to search for and type
   */

  (*fset).word1 = flag1mask;
  (*fset).word2 = flag2mask;
  (*fset).word3 = flag3mask;
  *p_type = type;
  return 1;
} /* end convert_flags() */
