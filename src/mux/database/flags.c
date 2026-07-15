/*
 * flags.c - flag manipulation routines
 */

#include "mux/server/platform.h"

#include "p.glue.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"

bool is_good_obj(DbRef x) {
  return x >= 0 && x < mudstate.db_top && typeof_obj(x) < NOTYPE;
}

bool is_safe(DbRef x, DbRef p) {
  return is_owns_others(x) || (obj_flags(x) & SAFE) ||
         (mudconf.safe_unowned && (obj_owner(x) != obj_owner(p)));
}

void mark(DbRef x) {
  mudstate.markbits->chunk[x >> 3] |= mudconf.markdata[x & 7];
}
void unmark(DbRef x) {
  mudstate.markbits->chunk[x >> 3] &= ~mudconf.markdata[x & 7];
}
bool is_marked(DbRef x) {
  return mudstate.markbits->chunk[x >> 3] & mudconf.markdata[x & 7];
}

static bool has_priv_suffix(const char *name) {
  return name && strlen(name) > 4 &&
         !strcasecmp(name + (strlen(name) - 5), ".PRIV");
}

bool see_attr(DbRef p, DbRef x, Attribute *a, DbRef o, long f) {
  return !(a->flags & (AF_INTERNAL | AF_IS_LOCK)) &&
         ((is_god(p) || (f & AF_VISUAL) ||
           (((obj_owner(p) == o) || is_examinable(p, x)) &&
            !(a->flags & (AF_DARK | AF_MDARK)) && !(f & (AF_DARK | AF_MDARK)) &&
            !has_priv_suffix(a->name)) ||
           (is_wizard(p) && !(a->flags & AF_DARK)) ||
           (!(a->flags & (AF_DARK | AF_MDARK | AF_ODARK)) &&
            !has_priv_suffix(a->name))));
}
bool see_attr_explicit(DbRef p, DbRef x, Attribute *a, DbRef o, long f) {
  return !(a->flags & (AF_INTERNAL | AF_IS_LOCK)) &&
         ((f & AF_VISUAL) ||
          ((obj_owner(p) == o) && !(a->flags & (AF_DARK | AF_MDARK)) &&
           !has_priv_suffix(a->name)));
}
bool set_attr(DbRef p, DbRef x, Attribute *a, long f) {
  return !(a->flags & (AF_INTERNAL | AF_IS_LOCK)) &&
         (is_god(p) ||
          (!is_god(x) && !(f & AF_LOCK) &&
           ((is_controls(p, x) && !(a->flags & (AF_WIZARD | AF_GOD)) &&
             !(f & (AF_WIZARD | AF_GOD)) && !has_priv_suffix(a->name)) ||
            (is_wizard(p) && !(a->flags & AF_GOD)))));
}
bool read_attr(DbRef p, DbRef x, Attribute *a, DbRef o, long f) {
  return !(a->flags & AF_INTERNAL) &&
         ((is_god(p) || (f & AF_VISUAL) ||
           (((obj_owner(p) == o) || is_examinable(p, x)) &&
            !(a->flags & (AF_DARK | AF_MDARK)) && !(f & (AF_DARK | AF_MDARK)) &&
            !has_priv_suffix(a->name)) ||
           (is_wizard(p) && !(a->flags & AF_DARK)) ||
           (!(a->flags & (AF_DARK | AF_MDARK | AF_ODARK)) &&
            !has_priv_suffix(a->name))));
}
bool write_attr(DbRef p, DbRef x, Attribute *a, long f) {
  return !(a->flags & AF_INTERNAL) &&
         (is_god(p) ||
          (!is_god(x) && !(f & AF_LOCK) &&
           ((is_controls(p, x) && !(a->flags & (AF_WIZARD | AF_GOD)) &&
             !(f & (AF_WIZARD | AF_GOD)) && !has_priv_suffix(a->name)) ||
            (is_wizard(p) && !(a->flags & AF_GOD)))));
}

/**
 * Sets or clears the indicated bit, no security checking.
 * @param target The target object for setting/unsetting
 * @param player The object who is setting/unsetting
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_any(DbRef target, DbRef player, Flag flag, int fflags,
                  int reset) {
  if (fflags & FLAG_WORD3) {
    if (reset)
      s_flags3(target, obj_flags3(target) & ~flag);
    else
      s_flags3(target, obj_flags3(target) | flag);
  } else if (fflags & FLAG_WORD2) {
    if (reset)
      s_flags2(target, obj_flags2(target) & ~flag);
    else
      s_flags2(target, obj_flags2(target) | flag);
  } else {
    if (reset)
      s_flags(target, obj_flags(target) & ~flag);
    else
      s_flags(target, obj_flags(target) | flag);
  }
  return 1;
} /* end fh_and() */

/**
 * Function to block out non-GOD for setting or clearing a bit.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_god(DbRef target, DbRef player, Flag flag, int fflags,
                  int reset) {
  if (!is_god(player))
    return 0;

  return (fh_any(target, player, flag, fflags, reset));
} /* end fh_god() */

/**
 * Blocks out non-WIZARDS setting or clearing a bit.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_wiz(DbRef target, DbRef player, Flag flag, int fflags,
                  int reset) {
  if (!is_wizard(player) && !is_god(player))
    return 0;

  return (fh_any(target, player, flag, fflags, reset));
} /* end fh_wiz() */

/**
 * Only allows the bit to be set on players by WIZARDS.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_fixed(DbRef target, DbRef player, Flag flag, int fflags,
                    int reset) {
  if (is_player(target))
    if (!is_wizard(player) && !is_god(player))
      return 0;

  return (fh_any(target, player, flag, fflags, reset));
} /* end fh_fixed() */

/**
 * Only allows players to set or clear this bit.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_inherit(DbRef target, DbRef player, Flag flag, int fflags,
                      int reset) {
  if (!is_inherits(player))
    return 0;

  return (fh_any(target, player, flag, fflags, reset));
} /* end fh_inherit() */

/**
 * Only allows GOD to set/clear this bit. Used for WIZARD flag.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_wiz_bit(DbRef target, DbRef player, Flag flag, int fflags,
                      int reset) {
  if (!is_god(player))
    return 0;
  if (is_god(target) && reset) {
    notify(player, "You cannot make yourself mortal.");
    return 0;
  }

  return (fh_any(target, player, flag, fflags, reset));
} /* end fh_wiz_bit() */

/**
 * Manipulates the dark bit. Only Wizards may set it on players.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_dark_bit(DbRef target, DbRef player, Flag flag, int fflags,
                       int reset) {
  if (!reset && is_player(target) && !is_wizard(player))
    return 0;

  return (fh_any(target, player, flag, fflags, reset));
} /* end fh_dark_bit() */

/**
 * Manipulates the going bit.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_going_bit(DbRef target, DbRef player, Flag flag, int fflags,
                        int reset) {
  if (is_going(target) && reset && (typeof_obj(target) != TYPE_GARBAGE)) {
    notify(player, "Your object has been spared from destruction.");
    return (fh_any(target, player, flag, fflags, reset));
  }

  if (!is_god(player))
    return 0;

  return (fh_any(target, player, flag, fflags, reset));
} /* end fh_going_bit() */

/**
 * Sets or clears bits that affect hearing.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_hear_bit(DbRef target, DbRef player, Flag flag, int fflags,
                       int reset) {
  int could_hear;

  if (is_player(target) && (flag & MONITOR)) {
    if (is_wizard(player))
      fh_any(target, player, flag, fflags, reset);
    else
      return 0;
  }

  could_hear = is_hearer(target);
  fh_any(target, player, flag, fflags, reset);
  handle_ears(target, could_hear, is_hearer(target));

  return 1;
} /* end fh_hear_bit() */

/**
 * Sets or clears bits that affect xcode in glue.h.
 * @param target Target object for setting/unsetting
 * @param player The object that is setting/unsetting
 * @param flag The flag to be manipulated
 * @param fflags ??
 * @param reset If 1, we're resetting the flag
 */
static int fh_xcode_bit(DbRef target, DbRef player, Flag flag, int fflags,
                        int reset) {
  int got_xcode;
  int new_xcode;

  got_xcode = is_hardcode(target);
  fh_wiz(target, player, flag, fflags, reset);
  new_xcode = is_hardcode(target);
  handle_xcode(player, target, got_xcode, new_xcode);

  return 1;
} /* end fh_xcode_bit() */

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
    {"HAS_DAILY", HAS_DAILY, '*', FLAG_WORD2, CA_GOD, fh_god},
    {"HAS_FORWARDLIST", HAS_FWDLIST, '&', FLAG_WORD2, CA_GOD, fh_god},
    {"HAS_HOURLY", HAS_HOURLY, '*', FLAG_WORD2, CA_GOD, fh_god},
    {"HAS_LISTEN", HAS_LISTEN, '@', FLAG_WORD2, CA_GOD, fh_god},
    {"HAS_STARTUP", HAS_STARTUP, '+', 0, CA_GOD, fh_god},
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
    {"ROBOT", ROBOT, 'r', 0, 0, fh_any},
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
void init_flagtab(void) {
  FLAGENT *fp;
  char *nbuf, *np, *bp;

  hash_table_initialize(&mudstate.flags_htab, 100 * HASH_FACTOR);
  nbuf = alloc_sbuf("init_flagtab");

  for (fp = gen_flags; fp->flagname; fp++) {
    for (np = nbuf, bp = (char *)fp->flagname; *bp; np++, bp++)
      *np = ToLower(*bp);
    *np = '\0';
    hash_table_add(nbuf, (int *)fp, &mudstate.flags_htab);
  }

  free_sbuf(nbuf);
} /* end init_flagtab() */

/**
 * Displays available flags. Used in @list flags.
 */
void display_flagtab(DbRef player) {
  char *buf, *bp;
  FLAGENT *fp;

  bp = buf = alloc_lbuf("display_flagtab");
  safe_str((char *)"Flags:", buf, &bp);

  for (fp = gen_flags; fp->flagname; fp++) {
    if ((fp->listperm & CA_WIZARD) && !is_wizard(player))
      continue;
    if ((fp->listperm & CA_GOD) && !is_god(player))
      continue;
    safe_chr(' ', buf, &bp);
    safe_str((char *)fp->flagname, buf, &bp);
    safe_chr('(', buf, &bp);
    safe_chr(fp->flaglett, buf, &bp);
    safe_chr(')', buf, &bp);
  }

  *bp = '\0';
  notify(player, buf);
  free_lbuf(buf);
} /* end display_flagtab() */

/**
 * ??
 */
FLAGENT *find_flag(DbRef thing, char *flagname) {
  char *cp;

  /* Make sure the flag name is valid */
  for (cp = flagname; *cp; cp++)
    *cp = ToLower(*cp);

  return (FLAGENT *)hash_table_find(flagname, &mudstate.flags_htab);
} /* end find_flag() */

/**
 * Sets or clears a specified flag on an object.
 * @param target Target object
 * @param player The object doing the setting
 * @paran flag The flag being set/unset
 * @param key Are we @set/quiet'in?
 */
void flag_set(DbRef target, DbRef player, char *flag, int key) {
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
      notify(player, "You must specify a flag to clear.");
    else
      notify(player, "You must specify a flag to set.");
    return;
  }
  fp = find_flag(target, flag);
  if (fp == nullptr) {
    notify(player, "I don't understand that flag.");
    return;
  }
  /*
   * Invoke the flag handler, and print feedback
   */

  result = fp->handler(target, player, fp->flagvalue, fp->flagflag, negate);
  if (!result)
    notify(player, "Permission denied.");
  else if (!(key & SET_QUIET) && !is_quiet(player))
    notify_printf(player, "%s - %s %s", Name(target), fp->flagname,
                  negate ? "cleared." : "set.");
  return;
} /* end flag_set() */

/**
 * Converts a flags word into corresponding letters.
 * @param player The invoking object
 * @param flagword ??
 * @param flag2word ??
 * @param flag3word ??
 */
char *decode_flags(DbRef player, Flag flagword, Flag flag2word,
                   Flag flag3word) {
  char *buf, *bp;
  FLAGENT *fp;
  int flagtype;
  Flag fv;

  buf = bp = alloc_sbuf("decode_flags");
  *bp = '\0';

  if (!is_good_obj(player)) {
    StringCopy(buf, "#-2 ERROR");
    return buf;
  }

  flagtype = (flagword & TYPE_MASK);
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
      if ((fp->listperm & CA_WIZARD) && !is_wizard(player))
        continue;
      if ((fp->listperm & CA_GOD) && !is_god(player))
        continue;
      /*
       * don't show CONNECT on dark wizards to mortals
       */
      if ((flagtype == TYPE_PLAYER) && (fp->flagvalue == CONNECTED) &&
          ((flagword & (WIZARD | DARK)) == (WIZARD | DARK)) &&
          !is_wizard(player))
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
int has_flag(DbRef player, DbRef target, char *flagname) {
  FLAGENT *fp;
  Flag fv;

  fp = find_flag(target, flagname);
  if (fp == nullptr)
    return 0;

  if (fp->flagflag & FLAG_WORD3)
    fv = obj_flags3(target);
  else if (fp->flagflag & FLAG_WORD2)
    fv = obj_flags2(target);
  else
    fv = obj_flags(target);

  if (fv & fp->flagvalue) {
    if ((fp->listperm & CA_WIZARD) && !is_wizard(player))
      return 0;
    if ((fp->listperm & CA_GOD) && !is_god(player))
      return 0;
    /*
     * don't show CONNECT on dark wizards to mortals
     */
    if (is_player(target) && (fp->flagvalue == CONNECTED) &&
        ((obj_flags(target) & (WIZARD | DARK)) == (WIZARD | DARK)) &&
        !is_wizard(player))
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
char *flag_description(DbRef player, DbRef target) {
  char *buff, *bp;
  FLAGENT *fp;
  int otype;
  Flag fv;

  /*
   * Allocate the return buffer
   */

  otype = typeof_obj(target);
  bp = buff = alloc_mbuf("flag_description");

  /*
   * Store the header strings and object type
   */

  safe_mb_str((char *)"Type: ", buff, &bp);
  safe_mb_str((char *)object_types[otype].name, buff, &bp);
  safe_mb_str((char *)" Flags:", buff, &bp);
  if (object_types[otype].perm != CA_PUBLIC) {
    *bp = '\0';
    return buff;
  }
  /*
   * Store the type-invariant flags
   */

  for (fp = gen_flags; fp->flagname; fp++) {
    if (fp->flagflag & FLAG_WORD3)
      fv = obj_flags3(target);
    else if (fp->flagflag & FLAG_WORD2)
      fv = obj_flags2(target);
    else
      fv = obj_flags(target);
    if (fv & fp->flagvalue) {
      if ((fp->listperm & CA_WIZARD) && !is_wizard(player))
        continue;
      if ((fp->listperm & CA_GOD) && !is_god(player))
        continue;
      /*
       * don't show CONNECT on dark wizards to mortals
       */
      if (is_player(target) && (fp->flagvalue == CONNECTED) &&
          ((obj_flags(target) & (WIZARD | DARK)) == (WIZARD | DARK)) &&
          !is_wizard(player))
        continue;
      safe_mb_chr(' ', buff, &bp);
      safe_mb_str((char *)fp->flagname, buff, &bp);
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
char *unparse_object_numonly(DbRef target) {
  char *buf;

  buf = alloc_lbuf("unparse_object_numonly");
  if (target == NOTHING) {
    StringCopy(buf, "*NOTHING*");
  } else if (target == HOME) {
    StringCopy(buf, "*HOME*");
  } else if (!is_good_obj(target)) {
    snprintf(buf, LBUF_SIZE, "*ILLEGAL*(#%ld)", target);
  } else {
    snprintf(buf, LBUF_SIZE, "%s(#%ld)", Name(target), target);
  }
  return buf;
} /* end unparse_object_numonly() */

/**
 * Returns an lbuf pointing to the object name and possibly the db# and flags.
 */
char *unparse_object(DbRef player, DbRef target, int obey_myopic) {
  char *buf, *fp;
  int exam;

  buf = alloc_lbuf("unparse_object");
  if (target == NOTHING) {
    StringCopy(buf, "*NOTHING*");
  } else if (target == HOME) {
    StringCopy(buf, "*HOME*");
  } else if (!is_good_obj(target)) {
    snprintf(buf, LBUF_SIZE, "*ILLEGAL*(#%ld)", target);
  } else {
    if (obey_myopic)
      exam = is_myopic_exam(player, target);
    else
      exam = is_examinable(player, target);
    if (exam) {

      /*
       * show everything
       */
      fp = unparse_flags(player, target);
      snprintf(buf, LBUF_SIZE, "%s(#%ld%s%s)", Name(target), target,
               *fp ? ":" : "", fp);
      free_sbuf(fp);
    } else {
      /*
       * show only the name.
       */
      StringCopy(buf, Name(target));
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
int convert_flags(DbRef player, char *flaglist, FLAGSET *fset, Flag *p_type) {
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
          !(((object_types[i].perm & CA_WIZARD) && !is_wizard(player)) ||
            ((object_types[i].perm & CA_GOD) && !is_god(player)))) {
        if ((type != NOTYPE) && (type != i)) {
          notify_printf(player, "%c: Conflicting type specifications.", *s);
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
          !(((fp->listperm & CA_WIZARD) && !is_wizard(player)) ||
            ((fp->listperm & CA_GOD) && !is_god(player)))) {
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
      notify_printf(player,
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
