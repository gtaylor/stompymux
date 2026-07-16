/*
 * look.c -- commands which look at things
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/commands/look.h"
#include "mux/database/attrs.h"
#include "mux/database/boolexp.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/world/match.h"

extern void ufun(char *, char *, int, int, int, DbRef, DbRef);

static void look_exits(DbRef player, DbRef loc, const char *exit_name) {
  DbRef thing, parent;
  char *buff, *e, *s, *buff1, *e1;
  int foundany, lev, key;

  /*
   * make sure location has exits
   */

  if (!is_good_obj(loc) || !has_exits(loc))
    return;

  /*
   * make sure there is at least one visible exit
   */

  foundany = 0;
  key = 0;
  if (is_dark(loc))
    key |= VE_BASE_DARK;
  ITER_PARENTS(loc, parent, lev) {
    key &= ~VE_LOC_DARK;
    if (is_dark(parent))
      key |= VE_LOC_DARK;
    DOLIST(thing, obj_exits(parent)) {
      if (exit_displayable(thing, player, key)) {
        foundany = 1;
        break;
      }
    }
    if (foundany)
      break;
  }

  if (!foundany)
    return;
  /*
   * Display the list of exit names
   */

  notify(player, exit_name);
  e = buff = alloc_lbuf("look_exits");
  e1 = buff1 = alloc_lbuf("look_exits2");
  ITER_PARENTS(loc, parent, lev) {
    key &= ~VE_LOC_DARK;
    if (is_dark(parent))
      key |= VE_LOC_DARK;
    if (is_transparent(loc)) {
      DOLIST(thing, obj_exits(parent)) {
        if (exit_displayable(thing, player, key)) {
          StringCopy(buff, Name(thing));
          for (e = buff; *e && (*e != ';'); e++)
            ;
          *e = '\0';
          notify_printf(player, "%s leads to %s.", buff,
                        Name(obj_location(thing)));
        }
      }
    } else {
      DOLIST(thing, obj_exits(parent)) {
        if (exit_displayable(thing, player, key)) {
          e1 = buff1;
          /* Put the exit name in buff1 */
          /*
           * chop off first * *
           *
           * * exit alias to *
           * * * display
           */

          if (buff != e)
            safe_str("  ", buff, &e);
          for (s = Name(thing); *s && (*s != ';'); s++)
            safe_chr(*s, buff1, &e1);
          *e1 = 0;
          /* Copy the exit name into 'buff' */
          /* Append this exit to the list */
          safe_str(buff1, buff, &e);
        }
      }
    }
  }

  if (!(is_transparent(loc))) {
    safe_str("\r\n", buff, &e);
    *e = 0;
    notify(player, buff);
  }
  free_lbuf(buff);
  free_lbuf(buff1);
}

#define CONTENTS_LOCAL 0
#define CONTENTS_NESTED 1
#define CONTENTS_REMOTE 2

static void look_contents(DbRef player, DbRef loc, const char *contents_name,
                          int style) {
  DbRef thing;
  int can_see_loc;
  char *buff;

  /*
   * check to see if he can see the location
   */

  can_see_loc =
      (!is_dark(loc) || (mudconf.see_own_dark && is_examinable(player, loc)));

  /*
   * check to see if there is anything there
   */

  DOLIST(thing, obj_contents(loc)) {
    if (can_see(player, thing, can_see_loc)) {

      /*
       * something exists!  show him everything
       */

      notify(player, contents_name);
      DOLIST(thing, obj_contents(loc)) {
        if (can_see(player, thing, can_see_loc)) {
          buff = unparse_object(player, thing, 1);
          notify(player, buff);
          free_lbuf(buff);
        }
      }
      break; /*
              * we're done
              */
    }
  }
}

static void view_atr(DbRef player, DbRef thing, Attribute *ap, char *text,
                     DbRef aowner, long aflags, int skip_tag) {
  char xbuf[6];
  char *xbufp;
  BooleanExpression *boolexp;

  if (ap->flags & AF_IS_LOCK) {
    boolexp = boolean_expression_parse(player, text, 1);
    text = boolean_expression_unparse(player, boolexp);
    boolean_expression_free(boolexp);
  }
  /*
   * If we don't control the object or own the attribute, hide the * *
   * * * attr owner and flag info.
   */

  if (!is_controls(player, thing) && (obj_owner(player) != aowner)) {
    if (skip_tag && (ap->number == A_DESC))
      notify_printf(player, "%s", text);
    else
      notify_printf(player, "\033[1m%s:\033[0m %s", ap->name, text);
    return;
  }
  /*
   * Generate flags
   */

  xbufp = xbuf;
  if (aflags & AF_LOCK)
    *xbufp++ = '+';
  if (aflags & AF_NOPROG)
    *xbufp++ = '$';
  if (aflags & AF_PRIVATE)
    *xbufp++ = 'I';
  if (aflags & AF_REGEXP)
    *xbufp++ = 'R';
  if (aflags & AF_VISUAL)
    *xbufp++ = 'V';
  if (aflags & AF_MDARK)
    *xbufp++ = 'M';
  if (aflags & AF_WIZARD)
    *xbufp++ = 'W';

  *xbufp = '\0';

  if ((aowner != obj_owner(thing)) && (aowner != NOTHING)) {
    notify_printf(player, "\033[1m%s [#%ld%s]:\033[0m %s", ap->name, aowner,
                  xbuf, text);
  } else if (*xbuf) {
    notify_printf(player, "\033[1m%s [%s]:\033[0m %s", ap->name, xbuf, text);
  } else if (!skip_tag || (ap->number != A_DESC)) {
    notify_printf(player, "\033[1m%s:\033[0m %s", ap->name, text);
  } else {
    notify_printf(player, "%s", text);
  }
}

static void look_atrs1(DbRef player, DbRef thing, DbRef othing,
                       int check_exclude, int hash_insert) {
  DbRef aowner;
  int ca;
  long aflags;
  Attribute *attr, *cattr;
  char *as, *buf;

  cattr = malloc(sizeof(Attribute));
  for (ca = attribute_list_first(thing, &as); ca;
       ca = attribute_list_next(&as)) {
    if ((ca == A_DESC) || (ca == A_LOCK))
      continue;
    attr = attribute_by_number(ca);
    if (!attr)
      continue;

    bcopy((char *)attr, (char *)cattr, sizeof(Attribute));

    /*
     * Should we exclude this attr?
     */

    if (check_exclude && ((attr->flags & AF_PRIVATE) ||
                          numeric_hash_table_find(ca, &mudstate.parent_htab)))
      continue;

    buf = attribute_get(thing, ca, &aowner, &aflags);
    if (read_attr(player, othing, attr, aowner, aflags)) {
      /* check_zone/attribute_by_number overwrites attr!! */

      if (attr->number != cattr->number)
        bcopy((char *)cattr, (char *)attr, sizeof(Attribute));

      if (!(check_exclude && (aflags & AF_PRIVATE))) {
        if (hash_insert)
          numeric_hash_table_add(ca, (int *)attr, &mudstate.parent_htab);
        view_atr(player, thing, attr, buf, aowner, aflags, 0);
      }
    }
    free_lbuf(buf);
  }
  free(cattr);
}

static void look_atrs(DbRef player, DbRef thing, int check_parents) {
  DbRef parent;
  int lev, check_exclude, hash_insert;

  if (!check_parents) {
    look_atrs1(player, thing, thing, 0, 0);
  } else {
    hash_insert = 1;
    check_exclude = 0;
    numeric_hash_table_flush(&mudstate.parent_htab, 0);
    ITER_PARENTS(thing, parent, lev) {
      if (!is_good_obj(obj_parent(parent)))
        hash_insert = 0;
      look_atrs1(player, parent, thing, check_exclude, hash_insert);
      check_exclude = 1;
    }
  }
}

static void look_simple(DbRef player, DbRef thing) {
  int pattr;
  char *buff;

  /*
   * Only makes sense for things that can hear
   */

  if (!is_hearer(player))
    return;

  /*
   * Get the name and db-number if we can examine it.
   */

  if (is_examinable(player, thing)) {
    buff = unparse_object(player, thing, 1);
    notify(player, buff);
    free_lbuf(buff);
  }
  pattr = A_DESC;
  did_it(player, thing, pattr, "You see nothing special.", A_ODESC, nullptr,
         A_ADESC, (char **)nullptr, 0);

  if (!mudconf.quiet_look) {
    look_atrs(player, thing, 0);
  }
}

static void show_a_desc(DbRef player, DbRef loc) {
  int indent = 0;

  indent =
      (is_room(loc) && mudconf.indent_desc && attribute_get_raw(loc, A_DESC));

  if (indent)
    raw_notify_newline(player);
  did_it(player, loc, A_DESC, nullptr, A_ODESC, nullptr, A_ADESC,
         (char **)nullptr, 0);
  if (indent)
    raw_notify_newline(player);
}

static void show_desc(DbRef player, DbRef loc, int use_idesc) {
  char *got;
  DbRef aowner;
  long aflags;

  if ((typeof_obj(loc) != TYPE_ROOM) && use_idesc) {
    if (*(got = attribute_parent_get(loc, A_IDESC, &aowner, &aflags)))
      did_it(player, loc, A_IDESC, nullptr, A_ODESC, nullptr, A_ADESC,
             (char **)nullptr, 0);
    else
      show_a_desc(player, loc);
    free_lbuf(got);
  } else {
    show_a_desc(player, loc);
  }
}

void look_in(DbRef player, DbRef loc, int key) {
  int pattr, oattr, aattr;
  char *buff;

  /*
   * Only makes sense for things that can hear
   */

  if (!is_hearer(player))
    return;

  /*
   * tell him the name, and the number if he can link to it
   */

  buff = unparse_object(player, loc, 1);
  notify(player, buff);
  free_lbuf(buff);

  if (!is_good_obj(loc))
    return; /*
             * If we went to NOTHING et al,  skip the * *
             *
             * * rest
             */

  /*
   * tell him the description
   */

  show_desc(player, loc, loc == obj_location(player));

  /*
   * tell him the appropriate messages if he has the key
   */

  if (typeof_obj(loc) == TYPE_ROOM) {
    if (could_doit(player, loc, A_LOCK)) {
      pattr = A_SUCC;
      oattr = A_OSUCC;
      aattr = A_ASUCC;
    } else {
      pattr = A_FAIL;
      oattr = A_OFAIL;
      aattr = A_AFAIL;
    }
    did_it(player, loc, pattr, nullptr, oattr, nullptr, aattr, (char **)nullptr,
           0);
  }
  /*
   * tell him the attributes, contents and exits
   */

  if ((key & LK_SHOWATTR) && !mudconf.quiet_look)
    look_atrs(player, loc, 0);
  look_contents(player, loc, "Contents:", CONTENTS_LOCAL);
  if (key & LK_SHOWEXIT)
    look_exits(player, loc, "Obvious exits:");
}

void do_look(DbRef player, DbRef cause, int key, char *name) {
  DbRef thing, loc;
  int look_key;

  look_key = LK_SHOWATTR | LK_SHOWEXIT;

  loc = obj_location(player);
  if (!name || !*name) {
    thing = loc;
    if (is_good_obj(thing)) {
      if (key & LOOK_OUTSIDE) {
        if ((typeof_obj(thing) == TYPE_ROOM) || is_opaque(thing)) {
          notify_quiet(player, "You can't look outside.");
          return;
        }
        thing = obj_location(thing);
      }
      look_in(player, thing, look_key);
    }
    return;
  }
  /*
   * Look for the target locally
   */

  thing = (key & LOOK_OUTSIDE) ? loc : player;
  init_match(thing, name, NOTYPE);
  match_exit_with_parents();
  match_neighbor();
  match_possession();
  if (is_long_fingers(player)) {
    match_absolute();
    match_player();
  }
  match_here();
  match_me();
  match_master_exit();
  thing = match_result();

  /*
   * Not found locally, check possessive
   */

  if (!is_good_obj(thing)) {
    thing = match_status(
        player, match_possessed(player, ((key & LOOK_OUTSIDE) ? loc : player),
                                (char *)name, thing, 0));
  }
  /*
   * If we found something, go handle it
   */

  if (is_good_obj(thing)) {
    switch (typeof_obj(thing)) {
    case TYPE_ROOM:
      look_in(player, thing, look_key);
      break;
    case TYPE_THING:
    case TYPE_PLAYER:
      look_simple(player, thing);
      if (!is_opaque(thing)) {
        look_contents(player, thing, "Carrying:", CONTENTS_NESTED);
      }
      break;
    case TYPE_EXIT:
      look_simple(player, thing);
      if (is_transparent(thing) && (obj_location(thing) != NOTHING)) {
        look_key &= ~LK_SHOWATTR;
        look_in(player, obj_location(thing), look_key);
      }
      break;
    default:
      look_simple(player, thing);
    }
  }
}

static void debug_examine(DbRef player, DbRef thing) {
  DbRef aowner;
  char *buf;
  long aflags;
  int ca;
  BooleanExpression *boolexp;
  Attribute *attr;
  char *as, *cp;

  notify_printf(player, "Number  = %ld", thing);
  if (!is_good_obj(thing))
    return;

  notify_printf(player, "Name    = %s", Name(thing));
  notify_printf(player, "Location= %ld", obj_location(thing));
  notify_printf(player, "Contents= %ld", obj_contents(thing));
  notify_printf(player, "Exits   = %ld", obj_exits(thing));
  notify_printf(player, "Link    = %ld", obj_link(thing));
  notify_printf(player, "Next    = %ld", obj_next(thing));
  notify_printf(player, "Owner   = %ld", obj_owner(thing));
  notify_printf(player, "Zone    = %ld", obj_zone(thing));
  buf = flag_description(player, thing);
  notify_printf(player, "Flags   = %s", buf);
  free_mbuf(buf);
  buf = power_description(player, thing);
  notify_printf(player, "Powers  = %s", buf);
  free_mbuf(buf);
  buf = attribute_get(thing, A_LOCK, &aowner, &aflags);
  boolexp = boolean_expression_parse(player, buf, 1);
  free_lbuf(buf);
  notify_printf(player, "Lock    = %s",
                boolean_expression_unparse(player, boolexp));
  boolean_expression_free(boolexp);

  buf = alloc_lbuf("debug_dexamine");
  cp = buf;
  safe_str("Attr list: ", buf, &cp);

  for (ca = attribute_list_first(thing, &as); ca;
       ca = attribute_list_next(&as)) {
    attr = attribute_by_number(ca);
    if (!attr)
      continue;

    attribute_get_info(thing, ca, &aowner, &aflags);
    if (read_attr(player, thing, attr, aowner, aflags)) {
      if (attr) { /*
                   * Valid attr.
                   */
        safe_str(attr->name, buf, &cp);
        safe_chr(' ', buf, &cp);
      } else {
        safe_str(tprintf("%d ", ca), buf, &cp);
      }
    }
  }
  *cp = '\0';
  notify(player, buf);
  free_lbuf(buf);

  for (ca = attribute_list_first(thing, &as); ca;
       ca = attribute_list_next(&as)) {
    attr = attribute_by_number(ca);
    if (!attr)
      continue;

    buf = attribute_get(thing, ca, &aowner, &aflags);
    if (read_attr(player, thing, attr, aowner, aflags))
      view_atr(player, thing, attr, buf, aowner, aflags, 0);
    free_lbuf(buf);
  }
}

static void exam_wildattrs(DbRef player, DbRef thing, int do_parent) {
  int atr, got_any;
  long aflags;
  char *buf;
  DbRef aowner;
  Attribute *ap;

  got_any = 0;
  for (atr = (int)olist_first(); atr != NOTHING; atr = (int)olist_next()) {
    ap = attribute_by_number(atr);
    if (!ap)
      continue;

    if (do_parent && !(ap->flags & AF_PRIVATE))
      buf = attribute_parent_get(thing, atr, &aowner, &aflags);
    else
      buf = attribute_get(thing, atr, &aowner, &aflags);

    /*
     * Decide if the player should see the attr: * If obj is * *
     * * Examinable and has rights to see, yes. * If a player and
     * *  *  * * has rights to see, yes... *   except if faraway,
     * * * * attr=DESC, and *   remote DESC-reading is not turned
     * on. *  *  * *  * * If I own the attrib and have rights to
     * see, yes... * * * * except if faraway, attr=DESC, and *
     * remote * DESC-reading * * is not turned on.
     */

    if (is_examinable(player, thing) &&
        read_attr(player, thing, ap, aowner, aflags)) {
      got_any = 1;
      view_atr(player, thing, ap, buf, aowner, aflags, 0);
    } else if ((typeof_obj(thing) == TYPE_PLAYER) &&
               read_attr(player, thing, ap, aowner, aflags)) {
      got_any = 1;
      if (aowner == obj_owner(player) || atr != A_DESC) {
        view_atr(player, thing, ap, buf, aowner, aflags, 0);
      } else if (mudconf.read_rem_desc || nearby(player, thing)) {
        show_desc(player, thing, 0);
      } else {
        notify(player, "<Too far away to get a good look>");
      }
    } else if (read_attr(player, thing, ap, aowner, aflags)) {
      got_any = 1;
      // The view_atr() branches below aren't a safe merge: when atr ==
      // A_DESC and nearby() is true, the show_desc() branch above must win,
      // so the last branch's nearby() check has to stay order-dependent.
      if (aowner == obj_owner(player)) { // NOLINT(bugprone-branch-clone)
        view_atr(player, thing, ap, buf, aowner, aflags, 0);
      } else if ((atr == A_DESC) &&
                 (mudconf.read_rem_desc || nearby(player, thing))) {
        show_desc(player, thing, 0);
      } else if (nearby(player, thing)) {
        view_atr(player, thing, ap, buf, aowner, aflags, 0);
      } else {
        notify(player, "<Too far away to get a good look>");
      }
    }
    free_lbuf(buf);
  }
  if (!got_any)
    notify_quiet(player, "No matching attributes found.");
}

void do_examine(DbRef player, DbRef cause, int key, char *name) {
  DbRef thing, content, exit, aowner, loc;
  char *temp, *buf, *buf2;
  BooleanExpression *boolexp;
  int control, do_parent;
  long aflags;

  /*
   * This command is pointless if the player can't hear.
   */

  if (!is_hearer(player))
    return;

  do_parent = key & EXAM_PARENT;
  thing = NOTHING;
  if (!name || !*name) {
    if ((thing = obj_location(player)) == NOTHING)
      return;
  } else {

    /* Check for obj/attr first */

    olist_push();
    if (parse_attrib_wild(player, name, &thing, do_parent, 1, 0)) {
      exam_wildattrs(player, thing, do_parent);
      olist_pop();
      return;
    }
    olist_pop();

    /* Look it up */

    init_match(player, name, NOTYPE);
    match_everything(MAT_EXIT_PARENTS);
    thing = noisy_match_result();
    if (!is_good_obj(thing))
      return;
  }

  /*
   * Check for the /debug switch
   */

  if (key == EXAM_DEBUG) {
    if (!is_examinable(player, thing)) {
      notify_quiet(player, "Permission denied.");
    } else {
      debug_examine(player, thing);
    }
    return;
  }
  control = (is_examinable(player, thing) || can_link_exit(player, thing));

  if (control) {
    buf2 = unparse_object(player, thing, 0);
    notify(player, buf2);
    free_lbuf(buf2);
    if (mudconf.ex_flags) {
      buf2 = flag_description(player, thing);
      notify(player, buf2);
      free_mbuf(buf2);
    }
  } else {
    if ((key == EXAM_DEFAULT) && !mudconf.exam_public) {
      if (mudconf.read_rem_name) {
        buf2 = alloc_lbuf("do_examine.pub_name");
        StringCopy(buf2, Name(thing));
        notify_printf(player, "%s is owned by %s", buf2,
                      Name(obj_owner(thing)));
        free_lbuf(buf2);
      } else {
        notify_printf(player, "Owned by %s", Name(obj_owner(thing)));
      }
      return;
    }
  }

  temp = alloc_lbuf("do_examine.info");

  if (control || mudconf.read_rem_desc || nearby(player, thing)) {
    temp = attribute_get_string(temp, thing, A_DESC, &aowner, &aflags);
    if (*temp) {
      if (is_examinable(player, thing) || (aowner == obj_owner(player))) {
        view_atr(player, thing, attribute_by_number(A_DESC), temp, aowner,
                 aflags, 1);
      } else {
        show_desc(player, thing, 0);
      }
    }
  } else {
    notify(player, "<Too far away to get a good look>");
  }

  if (control) {

    /*
     * print owner, key, and value
     */

    buf2 = attribute_get(thing, A_LOCK, &aowner, &aflags);
    boolexp = boolean_expression_parse(player, buf2, 1);
    buf = boolean_expression_unparse(player, boolexp);
    boolean_expression_free(boolexp);
    StringCopy(buf2, Name(obj_owner(thing)));
    notify_printf(player, "Owner: %s  Key: %s", buf2, buf);
    free_lbuf(buf2);

    if (mudconf.have_zones) {
      buf2 = unparse_object(player, obj_zone(thing), 0);
      notify_printf(player, "Zone: %s", buf2);
      free_lbuf(buf2);
    }
    /*
     * print parent
     */

    loc = obj_parent(thing);
    if (loc != NOTHING) {
      buf2 = unparse_object(player, loc, 0);
      notify_printf(player, "Parent: %s", buf2);
      free_lbuf(buf2);
    }
    buf2 = power_description(player, thing);
    notify(player, buf2);
    free_mbuf(buf2);
  }
  if (key != EXAM_BRIEF)
    look_atrs(player, thing, do_parent);

  /*
   * show him interesting stuff
   */

  if (control) {

    /*
     * Contents
     */

    if (obj_contents(thing) != NOTHING) {
      notify(player, "Contents:");
      DOLIST(content, obj_contents(thing)) {
        buf2 = unparse_object(player, content, 0);
        notify(player, buf2);
        free_lbuf(buf2);
      }
    }
    /*
     * Show stuff that depends on the object type
     */

    switch (typeof_obj(thing)) {
    case TYPE_ROOM:

      /*
       * tell him about exits
       */

      if (obj_exits(thing) != NOTHING) {
        notify(player, "Exits:");
        DOLIST(exit, obj_exits(thing)) {
          buf2 = unparse_object(player, exit, 0);
          notify(player, buf2);
          free_lbuf(buf2);
        }
      } else {
        notify(player, "No exits.");
      }

      /*
       * print dropto if present
       */

      if (obj_dropto(thing) != NOTHING) {
        buf2 = unparse_object(player, obj_dropto(thing), 0);
        notify_printf(player, "Dropped objects go to: %s", buf2);
        free_lbuf(buf2);
      }
      break;
    case TYPE_THING:
    case TYPE_PLAYER:

      /*
       * tell him about exits
       */

      if (obj_exits(thing) != NOTHING) {
        notify(player, "Exits:");
        DOLIST(exit, obj_exits(thing)) {
          buf2 = unparse_object(player, exit, 0);
          notify(player, buf2);
          free_lbuf(buf2);
        }
      } else {
        notify(player, "No exits.");
      }

      /*
       * print home
       */

      loc = obj_home(thing);
      buf2 = unparse_object(player, loc, 0);
      notify_printf(player, "Home: %s", buf2);
      free_lbuf(buf2);

      /*
       * print location if player can link to it
       */

      loc = obj_location(thing);
      if ((obj_location(thing) != NOTHING) &&
          (is_examinable(player, loc) || is_examinable(player, thing) ||
           is_linkable(player, loc))) {
        buf2 = unparse_object(player, loc, 0);
        notify_printf(player, "Location: %s", buf2);
        free_lbuf(buf2);
      }
      break;
    case TYPE_EXIT:
      buf2 = unparse_object(player, obj_exits(thing), 0);
      notify_printf(player, "Source: %s", buf2);
      free_lbuf(buf2);

      /*
       * print destination
       */

      switch (obj_location(thing)) {
      case NOTHING:
        break;
      case HOME:
        notify(player, "Destination: *HOME*");
        break;
      default:
        buf2 = unparse_object(player, obj_location(thing), 0);
        notify_printf(player, "Destination: %s", buf2);
        free_lbuf(buf2);
        break;
      }
      break;
    default:
      break;
    }
  } else if (!is_opaque(thing) && nearby(player, thing)) {
    if (has_contents(thing))
      look_contents(player, thing, "Contents:", CONTENTS_REMOTE);
    if (typeof_obj(thing) != TYPE_EXIT)
      look_exits(player, thing, "Obvious exits:");
  }
  free_lbuf(temp);

  if (!control) {
    if (mudconf.read_rem_name) {
      buf2 = alloc_lbuf("do_examine.pub_name");
      StringCopy(buf2, Name(thing));
      notify_printf(player, "%s is owned by %s", buf2, Name(obj_owner(thing)));
      free_lbuf(buf2);
    } else {
      notify_printf(player, "Owned by %s", Name(obj_owner(thing)));
    }
  }
}

void do_inventory(DbRef player, DbRef cause, int key) {
  DbRef thing;
  char *buff, *s, *e;

  thing = obj_contents(player);
  if (thing == NOTHING) {
    notify(player, "You aren't carrying anything.");
  } else {
    notify(player, "You are carrying:");
    DOLIST(thing, thing) {
      buff = unparse_object(player, thing, 1);
      notify(player, buff);
      free_lbuf(buff);
    }
  }

  thing = obj_exits(player);
  if (thing != NOTHING) {
    notify(player, "Exits:");
    e = buff = alloc_lbuf("look_exits");
    DOLIST(thing, thing) {
      /*
       * chop off first exit alias to display
       */
      for (s = Name(thing); *s && (*s != ';'); s++)
        safe_chr(*s, buff, &e);
      safe_str("  ", buff, &e);
    }
    *e = 0;
    notify(player, buff);
    free_lbuf(buff);
  }
}

void do_entrances(DbRef player, DbRef cause, int key, char *name) {
  DbRef thing, i, j;
  char *exit, *message;
  int control_thing, count;
  long low_bound, high_bound;
  FWDLIST *fp;

  parse_range(&name, &low_bound, &high_bound);
  if (!name || !*name) {
    if (has_location(player))
      thing = obj_location(player);
    else
      thing = player;
    if (!is_good_obj(thing))
      return;
  } else {
    init_match(player, name, NOTYPE);
    match_everything(MAT_EXIT_PARENTS);
    thing = noisy_match_result();
    if (!is_good_obj(thing))
      return;
  }

  message = alloc_lbuf("do_entrances");
  control_thing = is_examinable(player, thing);
  count = 0;
  for (i = low_bound; i <= high_bound; i++) {
    if (control_thing || is_examinable(player, i)) {
      switch (typeof_obj(i)) {
      case TYPE_EXIT:
        if (obj_location(i) == thing) {
          exit = unparse_object(player, obj_exits(i), 0);
          notify_printf(player, "%s (%s)", exit, Name(i));
          free_lbuf(exit);
          count++;
        }
        break;
      case TYPE_ROOM:
        if (obj_dropto(i) == thing) {
          exit = unparse_object(player, i, 0);
          notify_printf(player, "%s [dropto]", exit);
          free_lbuf(exit);
          count++;
        }
        break;
      case TYPE_THING:
      case TYPE_PLAYER:
        if (obj_home(i) == thing) {
          exit = unparse_object(player, i, 0);
          notify_printf(player, "%s [home]", exit);
          free_lbuf(exit);
          count++;
        }
        break;
      default:
        break;
      }

      /*
       * Check for parents
       */

      if (obj_parent(i) == thing) {
        exit = unparse_object(player, i, 0);
        notify_printf(player, "%s [parent]", exit);
        free_lbuf(exit);
        count++;
      }
      /*
       * Check for forwarding
       */

      if (has_fwdlist(i)) {
        fp = fwdlist_get(i);
        if (!fp)
          continue;
        for (j = 0; j < fp->count; j++) {
          if (fp->data[j] != thing)
            continue;
          exit = unparse_object(player, i, 0);
          notify_printf(player, "%s [forward]", exit);
          free_lbuf(exit);
          count++;
        }
      }
    }
  }
  free_lbuf(message);
  notify_printf(player, "%d entrance%s found.", count, (count == 1) ? "" : "s");
}

/*
 * check the current location for bugs
 */

static void sweep_check(DbRef player, DbRef what, int key, int is_loc) {
  DbRef aowner, parent;
  int canhear, cancom, isplayer, ispuppet, isconnected, attr;
  long aflags;
  int is_parent, lev;
  char *buf, *buf2, *bp, *as, *buff, *s;
  Attribute *ap;

  if (is_dark(what) && !is_wizard(player) && !mudconf.sweep_dark)
    return;
  canhear = 0;
  cancom = 0;
  isplayer = 0;
  ispuppet = 0;
  isconnected = 0;
  is_parent = 0;

  if ((key & SWEEP_LISTEN) &&
      (((typeof_obj(what) == TYPE_EXIT) || is_loc) && is_audible(what))) {
    canhear = 1;
  } else if (key & SWEEP_LISTEN) {
    if (is_monitor(what))
      buff = alloc_lbuf("Hearer");
    else
      buff = nullptr;

    for (attr = attribute_list_first(what, &as); attr;
         attr = attribute_list_next(&as)) {
      if (attr == A_LISTEN) {
        canhear = 1;
        break;
      }
      if (buff && is_monitor(what)) {
        ap = attribute_by_number(attr);
        if (!ap || (ap->flags & AF_NOPROG))
          continue;

        attribute_get_string(buff, what, attr, &aowner, &aflags);

        /*
         * Make sure we can execute it
         */

        if ((buff[0] != AMATCH_LISTEN) || (aflags & AF_NOPROG))
          continue;

        /*
         * Make sure there's a : in it
         */

        for (s = buff + 1; *s && (*s != ':'); s++)
          ;
        if (s) {
          canhear = 1;
          break;
        }
      }
    }
    if (buff)
      free_lbuf(buff);
  }
  if ((key & SWEEP_COMMANDS) && (typeof_obj(what) != TYPE_EXIT)) {

    /*
     * Look for commands on the object and parents too
     */

    ITER_PARENTS(what, parent, lev) {
      if (has_commands(parent)) {
        cancom = 1;
        if (lev) {
          is_parent = 1;
          break;
        }
      }
    }
  }
  if (key & SWEEP_CONNECT) {
    if (is_connected(what) ||
        (is_puppet(what) && is_connected(obj_owner(what))) ||
        (mudconf.player_listen && (typeof_obj(what) == TYPE_PLAYER) &&
         canhear && is_connected(obj_owner(what))))
      isconnected = 1;
  }
  if (key & SWEEP_PLAYER || isconnected) {
    if (typeof_obj(what) == TYPE_PLAYER)
      isplayer = 1;
    if (is_puppet(what))
      ispuppet = 1;
  }
  if (canhear || cancom || isplayer || ispuppet || isconnected) {
    buf = alloc_lbuf("sweep_check.types");
    bp = buf;

    if (cancom)
      safe_str("commands ", buf, &bp);
    if (canhear)
      safe_str("messages ", buf, &bp);
    if (isplayer)
      safe_str("player ", buf, &bp);
    if (ispuppet) {
      safe_str("is_puppet(", buf, &bp);
      safe_str(Name(obj_owner(what)), buf, &bp);
      safe_str(") ", buf, &bp);
    }
    if (isconnected)
      safe_str("connected ", buf, &bp);
    if (is_parent)
      safe_str("parent ", buf, &bp);
    bp[-1] = '\0';
    if (typeof_obj(what) != TYPE_EXIT) {
      notify_printf(player, "  %s is listening. [%s]", Name(what), buf);
    } else {
      buf2 = alloc_lbuf("sweep_check.name");
      StringCopy(buf2, Name(what));
      for (bp = buf2; *bp && (*bp != ';'); bp++)
        ;
      *bp = '\0';
      notify_printf(player, "  %s is listening. [%s]", buf2, buf);
      free_lbuf(buf2);
    }
    free_lbuf(buf);
  }
}

void do_sweep(DbRef player, DbRef cause, int key, char *where) {
  DbRef here, sweeploc;
  int where_key, what_key;

  where_key = key & (SWEEP_ME | SWEEP_HERE | SWEEP_EXITS);
  what_key =
      key & (SWEEP_COMMANDS | SWEEP_LISTEN | SWEEP_PLAYER | SWEEP_CONNECT);

  if (where && *where) {
    sweeploc = match_controlled(player, where);
    if (!is_good_obj(sweeploc))
      return;
  } else {
    sweeploc = player;
  }

  if (!where_key)
    where_key = -1;
  if (!what_key)
    what_key = -1;
  else if (what_key == SWEEP_VERBOSE)
    what_key = SWEEP_VERBOSE | SWEEP_COMMANDS;

  /*
   * Check my location.  If I have none or it is dark, check just me.
   */

  if (where_key & SWEEP_HERE) {
    notify(player, "Sweeping location...");
    if (has_location(sweeploc)) {
      here = obj_location(sweeploc);
      if ((here == NOTHING) || (is_dark(here) && !mudconf.sweep_dark &&
                                !is_examinable(player, here))) {
        notify_quiet(player,
                     "Sorry, it is dark here and you can't search for bugs");
        sweep_check(player, sweeploc, what_key, 0);
      } else {
        sweep_check(player, here, what_key, 1);
        for (here = obj_contents(here); here != NOTHING; here = obj_next(here))
          sweep_check(player, here, what_key, 0);
      }
    } else {
      sweep_check(player, sweeploc, what_key, 0);
    }
  }
  /*
   * Check exits in my location
   */

  if ((where_key & SWEEP_EXITS) && has_location(sweeploc)) {
    notify(player, "Sweeping exits...");
    for (here = obj_exits(obj_location(sweeploc)); here != NOTHING;
         here = obj_next(here))
      sweep_check(player, here, what_key, 0);
  }
  /*
   * Check my inventory
   */

  if ((where_key & SWEEP_ME) && has_contents(sweeploc)) {
    notify(player, "Sweeping inventory...");
    for (here = obj_contents(sweeploc); here != NOTHING; here = obj_next(here))
      sweep_check(player, here, what_key, 0);
  }
  /*
   * Check carried exits
   */

  if ((where_key & SWEEP_EXITS) && has_exits(sweeploc)) {
    notify(player, "Sweeping carried exits...");
    for (here = obj_exits(sweeploc); here != NOTHING; here = obj_next(here))
      sweep_check(player, here, what_key, 0);
  }
  notify(player, "Sweep complete.");
}
