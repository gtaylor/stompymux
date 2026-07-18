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
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include "mux/world/walkdb.h"
#include "mux/world/world_context.h"

extern void ufun(char *, char *, int, int, int, DbRef, DbRef);

static void look_exits(EvaluationContext *evaluation, DbRef player, DbRef loc,
                       const char *exit_name) {
  WorldContext *world = evaluation->world;
  DbRef thing, parent;
  char *buff, *e, *s, *buff1, *e1;
  int foundany, lev, key;

  /*
   * make sure location has exits
   */

  if (!is_good_obj(evaluation->world->database, loc) ||
      !has_exits(evaluation->world->database, loc))
    return;

  /*
   * make sure there is at least one visible exit
   */

  foundany = 0;
  key = 0;
  if (is_dark(evaluation->world->database, loc))
    key |= VE_BASE_DARK;
  ITER_PARENTS(world->database, world->configuration, loc, parent, lev) {
    key &= ~VE_LOC_DARK;
    if (is_dark(evaluation->world->database, parent))
      key |= VE_LOC_DARK;
    DOLIST(evaluation->world->database, thing,
           game_object_exits(evaluation->world->database, parent)) {
      if (exit_displayable(world->database, thing, player, key)) {
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

  notify(evaluation, player, exit_name);
  e = buff = alloc_lbuf("look_exits");
  e1 = buff1 = alloc_lbuf("look_exits2");
  ITER_PARENTS(world->database, world->configuration, loc, parent, lev) {
    key &= ~VE_LOC_DARK;
    if (is_dark(evaluation->world->database, parent))
      key |= VE_LOC_DARK;
    if (is_transparent(evaluation->world->database, loc)) {
      DOLIST(evaluation->world->database, thing,
             game_object_exits(evaluation->world->database, parent)) {
        if (exit_displayable(world->database, thing, player, key)) {
          StringCopy(buff,
                     game_object_name(evaluation->world->database, thing));
          for (e = buff; *e && (*e != ';'); e++)
            ;
          *e = '\0';
          notify_printf(
              evaluation, player, "%s leads to %s.", buff,
              game_object_name(
                  evaluation->world->database,
                  game_object_location(evaluation->world->database, thing)));
        }
      }
    } else {
      DOLIST(evaluation->world->database, thing,
             game_object_exits(evaluation->world->database, parent)) {
        if (exit_displayable(world->database, thing, player, key)) {
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
          for (s = game_object_name(evaluation->world->database, thing);
               *s && (*s != ';'); s++)
            safe_chr(*s, buff1, &e1);
          *e1 = 0;
          /* Copy the exit name into 'buff' */
          /* Append this exit to the list */
          safe_str(buff1, buff, &e);
        }
      }
    }
  }

  if (!(is_transparent(evaluation->world->database, loc))) {
    safe_str("\r\n", buff, &e);
    *e = 0;
    notify(evaluation, player, buff);
  }
  free_lbuf(buff);
  free_lbuf(buff1);
}

#define CONTENTS_LOCAL 0
#define CONTENTS_NESTED 1
#define CONTENTS_REMOTE 2

static void look_contents(EvaluationContext *evaluation, DbRef player,
                          DbRef loc, const char *contents_name, int style) {
  WorldContext *world = evaluation->world;
  DbRef thing;
  int can_see_loc;
  char *buff;

  /*
   * check to see if he can see the location
   */

  can_see_loc = (!is_dark(evaluation->world->database, loc) ||
                 (world->configuration->see_own_dark &&
                  is_examinable(evaluation, player, loc)));

  /*
   * check to see if there is anything there
   */

  DOLIST(evaluation->world->database, thing,
         game_object_contents(evaluation->world->database, loc)) {
    if (can_see(evaluation, world->configuration, player, thing, can_see_loc)) {

      /*
       * something exists!  show him everything
       */

      notify(evaluation, player, contents_name);
      DOLIST(evaluation->world->database, thing,
             game_object_contents(evaluation->world->database, loc)) {
        if (can_see(evaluation, world->configuration, player, thing,
                    can_see_loc)) {
          buff = unparse_object(evaluation->world->database, evaluation, player,
                                thing, 1);
          notify(evaluation, player, buff);
          free_lbuf(buff);
        }
      }
      break; /*
              * we're done
              */
    }
  }
}

static void view_atr(EvaluationContext *evaluation, DbRef player, DbRef thing,
                     Attribute *ap, char *text, DbRef aowner, long aflags,
                     int skip_tag) {
  char xbuf[6];
  char *xbufp;
  BooleanExpression *boolexp;
  char lock_text[LBUF_SIZE];

  if (ap->flags & AF_IS_LOCK) {
    boolexp = boolean_expression_parse(evaluation->world->database, nullptr,
                                       player, text, 1);
    boolean_expression_unparse(evaluation->world->database, evaluation,
                               lock_text, player, boolexp);
    text = lock_text;
    boolean_expression_free(boolexp);
  }
  /*
   * If we don't control the object or own the attribute, hide the * *
   * * * attr owner and flag info.
   */

  if (!is_controls(evaluation, player, thing) &&
      (game_object_owner(evaluation->world->database, player) != aowner)) {
    if (skip_tag && (ap->number == A_DESC))
      notify_printf(evaluation, player, "%s", text);
    else
      notify_printf(evaluation, player, "\033[1m%s:\033[0m %s", ap->name, text);
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

  if ((aowner != game_object_owner(evaluation->world->database, thing)) &&
      (aowner != NOTHING)) {
    notify_printf(evaluation, player, "\033[1m%s [#%ld%s]:\033[0m %s", ap->name,
                  aowner, xbuf, text);
  } else if (*xbuf) {
    notify_printf(evaluation, player, "\033[1m%s [%s]:\033[0m %s", ap->name,
                  xbuf, text);
  } else if (!skip_tag || (ap->number != A_DESC)) {
    notify_printf(evaluation, player, "\033[1m%s:\033[0m %s", ap->name, text);
  } else {
    notify_printf(evaluation, player, "%s", text);
  }
}

static void look_atrs1(EvaluationContext *evaluation, DbRef player, DbRef thing,
                       DbRef othing, int check_exclude, int hash_insert) {
  WorldContext *world = evaluation->world;
  DbRef aowner;
  int ca;
  long aflags;
  Attribute *attr, *cattr;
  char *as, *buf;

  cattr = malloc(sizeof(Attribute));
  for (ca = attribute_list_first(evaluation->world->database, thing, &as); ca;
       ca = attribute_list_next(&as)) {
    if ((ca == A_DESC) || (ca == A_LOCK))
      continue;
    attr = attribute_by_number(evaluation->world->database, ca);
    if (!attr)
      continue;

    bcopy((char *)attr, (char *)cattr, sizeof(Attribute));

    /*
     * Should we exclude this attr?
     */

    if (check_exclude &&
        ((attr->flags & AF_PRIVATE) ||
         numeric_hash_table_find(ca, &world->indexes->parent_commands)))
      continue;

    buf =
        attribute_get(evaluation->world->database, thing, ca, &aowner, &aflags);
    if (read_attr(evaluation, player, othing, attr, aowner, aflags)) {
      /* check_zone/attribute_by_number overwrites attr!! */

      if (attr->number != cattr->number)
        bcopy((char *)cattr, (char *)attr, sizeof(Attribute));

      if (!(check_exclude && (aflags & AF_PRIVATE))) {
        if (hash_insert)
          numeric_hash_table_add(ca, (int *)attr,
                                 &world->indexes->parent_commands);
        view_atr(evaluation, player, thing, attr, buf, aowner, aflags, 0);
      }
    }
    free_lbuf(buf);
  }
  free(cattr);
}

static void look_atrs(EvaluationContext *evaluation, DbRef player, DbRef thing,
                      int check_parents) {
  WorldContext *world = evaluation->world;
  DbRef parent;
  int lev, check_exclude, hash_insert;

  if (!check_parents) {
    look_atrs1(evaluation, player, thing, thing, 0, 0);
  } else {
    hash_insert = 1;
    check_exclude = 0;
    numeric_hash_table_flush(&world->indexes->parent_commands, 0);
    ITER_PARENTS(world->database, world->configuration, thing, parent, lev) {
      if (!is_good_obj(evaluation->world->database,
                       game_object_parent(evaluation->world->database, parent)))
        hash_insert = 0;
      look_atrs1(evaluation, player, parent, thing, check_exclude, hash_insert);
      check_exclude = 1;
    }
  }
}

static void look_simple(EvaluationContext *evaluation, DbRef player,
                        DbRef thing) {
  WorldContext *world = evaluation->world;
  int pattr;
  char *buff;

  /*
   * Only makes sense for things that can hear
   */

  if (!is_hearer(evaluation, player))
    return;

  /*
   * Get the name and db-number if we can examine it.
   */

  if (is_examinable(evaluation, player, thing)) {
    buff = unparse_object(evaluation->world->database, evaluation, player,
                          thing, 1);
    notify(evaluation, player, buff);
    free_lbuf(buff);
  }
  pattr = A_DESC;
  did_it(evaluation, player, thing, pattr, "You see nothing special.", A_ODESC,
         nullptr, A_ADESC, (char **)nullptr, 0);

  if (!world->configuration->quiet_look) {
    look_atrs(evaluation, player, thing, 0);
  }
}

static void show_a_desc(EvaluationContext *evaluation, DbRef player,
                        DbRef loc) {
  WorldContext *world = evaluation->world;
  int indent = 0;

  indent = (is_room(evaluation->world->database, loc) &&
            world->configuration->indent_desc &&
            attribute_get_raw(evaluation->world->database, loc, A_DESC));

  if (indent)
    raw_notify_newline(evaluation, player);
  did_it(evaluation, player, loc, A_DESC, nullptr, A_ODESC, nullptr, A_ADESC,
         (char **)nullptr, 0);
  if (indent)
    raw_notify_newline(evaluation, player);
}

static void show_desc(EvaluationContext *evaluation, DbRef player, DbRef loc,
                      int use_idesc) {
  char *got;
  DbRef aowner;
  long aflags;

  if ((typeof_obj(evaluation->world->database, loc) != TYPE_ROOM) &&
      use_idesc) {
    if (*(got = attribute_parent_get(evaluation->world->database, loc, A_IDESC,
                                     &aowner, &aflags)))
      did_it(evaluation, player, loc, A_IDESC, nullptr, A_ODESC, nullptr,
             A_ADESC, (char **)nullptr, 0);
    else
      show_a_desc(evaluation, player, loc);
    free_lbuf(got);
  } else {
    show_a_desc(evaluation, player, loc);
  }
}

void look_in(EvaluationContext *evaluation, DbRef player, DbRef loc, int key) {
  WorldContext *world = evaluation->world;
  int pattr, oattr, aattr;
  char *buff;

  /*
   * Only makes sense for things that can hear
   */

  if (!is_hearer(evaluation, player))
    return;

  /*
   * tell him the name, and the number if he can link to it
   */

  buff =
      unparse_object(evaluation->world->database, evaluation, player, loc, 1);
  notify(evaluation, player, buff);
  free_lbuf(buff);

  if (!is_good_obj(evaluation->world->database, loc))
    return; /*
             * If we went to NOTHING et al,  skip the * *
             *
             * * rest
             */

  /*
   * tell him the description
   */

  show_desc(evaluation, player, loc,
            loc == game_object_location(evaluation->world->database, player));

  /*
   * tell him the appropriate messages if he has the key
   */

  if (typeof_obj(evaluation->world->database, loc) == TYPE_ROOM) {
    if (could_doit_with_context(evaluation, player, loc, A_LOCK)) {
      pattr = A_SUCC;
      oattr = A_OSUCC;
      aattr = A_ASUCC;
    } else {
      pattr = A_FAIL;
      oattr = A_OFAIL;
      aattr = A_AFAIL;
    }
    did_it(evaluation, player, loc, pattr, nullptr, oattr, nullptr, aattr,
           (char **)nullptr, 0);
  }
  /*
   * tell him the attributes, contents and exits
   */

  if ((key & LK_SHOWATTR) && !world->configuration->quiet_look)
    look_atrs(evaluation, player, loc, 0);
  look_contents(evaluation, player, loc, "Contents:", CONTENTS_LOCAL);
  if (key & LK_SHOWEXIT)
    look_exits(evaluation, player, loc, "Obvious exits:");
}

void do_look(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *name = invocation->first;
  DbRef thing, loc;
  int look_key;

  look_key = LK_SHOWATTR | LK_SHOWEXIT;

  loc = game_object_location(evaluation->world->database, player);
  if (!name || !*name) {
    thing = loc;
    if (is_good_obj(evaluation->world->database, thing)) {
      if (key & LOOK_OUTSIDE) {
        if ((typeof_obj(evaluation->world->database, thing) == TYPE_ROOM) ||
            is_opaque(evaluation->world->database, thing)) {
          notify_quiet(evaluation, player, "You can't look outside.");
          return;
        }
        thing = game_object_location(evaluation->world->database, thing);
      }
      look_in(evaluation, player, thing, look_key);
    }
    return;
  }
  /*
   * Look for the target locally
   */

  thing = (key & LOOK_OUTSIDE) ? loc : player;
  init_match(&invocation->context->match, thing, name, NOTYPE);
  match_exit_with_parents(&invocation->context->match);
  match_neighbor(&invocation->context->match);
  match_possession(&invocation->context->match);
  if (is_long_fingers(evaluation->world->database, player)) {
    match_absolute(&invocation->context->match);
    match_player(&invocation->context->match);
  }
  match_here(&invocation->context->match);
  match_me(&invocation->context->match);
  match_master_exit(&invocation->context->match);
  thing = match_result(&invocation->context->match);

  /*
   * Not found locally, check possessive
   */

  if (!is_good_obj(evaluation->world->database, thing)) {
    thing = match_status(evaluation, player,
                         match_possessed(&invocation->context->match, player,
                                         ((key & LOOK_OUTSIDE) ? loc : player),
                                         (char *)name, thing, 0));
  }
  /*
   * If we found something, go handle it
   */

  if (is_good_obj(evaluation->world->database, thing)) {
    switch (typeof_obj(evaluation->world->database, thing)) {
    case TYPE_ROOM:
      look_in(evaluation, player, thing, look_key);
      break;
    case TYPE_THING:
    case TYPE_PLAYER:
      look_simple(evaluation, player, thing);
      if (!is_opaque(evaluation->world->database, thing)) {
        look_contents(evaluation, player, thing, "Carrying:", CONTENTS_NESTED);
      }
      break;
    case TYPE_EXIT:
      look_simple(evaluation, player, thing);
      if (is_transparent(evaluation->world->database, thing) &&
          (game_object_location(evaluation->world->database, thing) !=
           NOTHING)) {
        look_key &= ~LK_SHOWATTR;
        look_in(evaluation, player,
                game_object_location(evaluation->world->database, thing),
                look_key);
      }
      break;
    default:
      look_simple(evaluation, player, thing);
    }
  }
}

static void debug_examine(EvaluationContext *evaluation, DbRef player,
                          DbRef thing) {
  DbRef aowner;
  char *buf;
  long aflags;
  int ca;
  BooleanExpression *boolexp;
  Attribute *attr;
  char *as, *cp;
  char lock_text[LBUF_SIZE];

  notify_printf(evaluation, player, "Number  = %ld", thing);
  if (!is_good_obj(evaluation->world->database, thing))
    return;

  notify_printf(evaluation, player, "Name    = %s",
                game_object_name(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Location= %ld",
                game_object_location(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Contents= %ld",
                game_object_contents(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Exits   = %ld",
                game_object_exits(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Link    = %ld",
                game_object_link(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Next    = %ld",
                game_object_next(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Owner   = %ld",
                game_object_owner(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Zone    = %ld",
                game_object_zone(evaluation->world->database, thing));
  buf = flag_description(evaluation->world->database, player, thing);
  notify_printf(evaluation, player, "Flags   = %s", buf);
  free_mbuf(buf);
  buf = power_description(evaluation->world->database, player, thing);
  notify_printf(evaluation, player, "Powers  = %s", buf);
  free_mbuf(buf);
  buf = attribute_get(evaluation->world->database, thing, A_LOCK, &aowner,
                      &aflags);
  boolexp = boolean_expression_parse(evaluation->world->database, nullptr,
                                     player, buf, 1);
  free_lbuf(buf);
  boolean_expression_unparse(evaluation->world->database, evaluation, lock_text,
                             player, boolexp);
  notify_printf(evaluation, player, "Lock    = %s", lock_text);
  boolean_expression_free(boolexp);

  buf = alloc_lbuf("debug_dexamine");
  cp = buf;
  safe_str("Attr list: ", buf, &cp);

  for (ca = attribute_list_first(evaluation->world->database, thing, &as); ca;
       ca = attribute_list_next(&as)) {
    attr = attribute_by_number(evaluation->world->database, ca);
    if (!attr)
      continue;

    attribute_get_info(evaluation->world->database, thing, ca, &aowner,
                       &aflags);
    if (read_attr(evaluation, player, thing, attr, aowner, aflags)) {
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
  notify(evaluation, player, buf);
  free_lbuf(buf);

  for (ca = attribute_list_first(evaluation->world->database, thing, &as); ca;
       ca = attribute_list_next(&as)) {
    attr = attribute_by_number(evaluation->world->database, ca);
    if (!attr)
      continue;

    buf =
        attribute_get(evaluation->world->database, thing, ca, &aowner, &aflags);
    if (read_attr(evaluation, player, thing, attr, aowner, aflags))
      view_atr(evaluation, player, thing, attr, buf, aowner, aflags, 0);
    free_lbuf(buf);
  }
}

static void exam_wildattrs(EvaluationContext *evaluation, DbRef player,
                           DbRef thing, int do_parent, ObjectList *attributes) {
  WorldContext *world = evaluation->world;
  int atr, got_any;
  long aflags;
  char *buf;
  DbRef aowner;
  Attribute *ap;

  got_any = 0;
  for (atr = (int)object_list_first(attributes); atr != NOTHING;
       atr = (int)object_list_next(attributes)) {
    ap = attribute_by_number(evaluation->world->database, atr);
    if (!ap)
      continue;

    if (do_parent && !(ap->flags & AF_PRIVATE))
      buf = attribute_parent_get(evaluation->world->database, thing, atr,
                                 &aowner, &aflags);
    else
      buf = attribute_get(evaluation->world->database, thing, atr, &aowner,
                          &aflags);

    /*
     * Decide if the player should see the attr: * If obj is * *
     * * Examinable and has rights to see, yes. * If a player and
     * *  *  * * has rights to see, yes... *   except if faraway,
     * * * * attr=DESC, and *   remote DESC-reading is not turned
     * on. *  *  * *  * * If I own the attrib and have rights to
     * see, yes... * * * * except if faraway, attr=DESC, and *
     * remote * DESC-reading * * is not turned on.
     */

    if (is_examinable(evaluation, player, thing) &&
        read_attr(evaluation, player, thing, ap, aowner, aflags)) {
      got_any = 1;
      view_atr(evaluation, player, thing, ap, buf, aowner, aflags, 0);
    } else if ((typeof_obj(evaluation->world->database, thing) ==
                TYPE_PLAYER) &&
               read_attr(evaluation, player, thing, ap, aowner, aflags)) {
      got_any = 1;
      if (aowner == game_object_owner(evaluation->world->database, player) ||
          atr != A_DESC) {
        view_atr(evaluation, player, thing, ap, buf, aowner, aflags, 0);
      } else if (world->configuration->read_rem_desc ||
                 nearby(evaluation->world->database, player, thing)) {
        show_desc(evaluation, player, thing, 0);
      } else {
        notify(evaluation, player, "<Too far away to get a good look>");
      }
    } else if (read_attr(evaluation, player, thing, ap, aowner, aflags)) {
      got_any = 1;
      // The view_atr() branches below aren't a safe merge: when atr ==
      // A_DESC and nearby(evaluation->world->database, ) is true, the
      // show_desc() branch above must win, so the last branch's
      // nearby(evaluation->world->database, ) check has to stay
      // order-dependent.
      if (aowner ==
          game_object_owner(evaluation->world->database,
                            player)) { // NOLINT(bugprone-branch-clone)
        view_atr(evaluation, player, thing, ap, buf, aowner, aflags, 0);
      } else if ((atr == A_DESC) &&
                 (world->configuration->read_rem_desc ||
                  nearby(evaluation->world->database, player, thing))) {
        show_desc(evaluation, player, thing, 0);
      } else if (nearby(evaluation->world->database, player, thing)) {
        view_atr(evaluation, player, thing, ap, buf, aowner, aflags, 0);
      } else {
        notify(evaluation, player, "<Too far away to get a good look>");
      }
    }
    free_lbuf(buf);
  }
  if (!got_any)
    notify_quiet(evaluation, player, "No matching attributes found.");
}

void do_examine(CommandInvocation *invocation) {
  WorldContext *world = invocation->context->world;
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *name = invocation->first;
  DbRef thing, content, exit, aowner, loc;
  char *temp, *buf2;
  BooleanExpression *boolexp;
  int control, do_parent;
  long aflags;
  char lock_text[LBUF_SIZE];
  ObjectList attributes;

  /*
   * This command is pointless if the player can't hear.
   */

  if (!is_hearer(evaluation, player))
    return;

  do_parent = key & EXAM_PARENT;
  thing = NOTHING;
  if (!name || !*name) {
    if ((thing = game_object_location(evaluation->world->database, player)) ==
        NOTHING)
      return;
  } else {

    /* Check for obj/attr first */

    object_list_initialize(&attributes);
    if (parse_attrib_wild(&invocation->context->match, player, name, &thing,
                          do_parent, 1, 0, &attributes, world->configuration,
                          world->indexes)) {
      exam_wildattrs(&invocation->context->evaluation, player, thing, do_parent,
                     &attributes);
      object_list_destroy(&attributes);
      return;
    }
    object_list_destroy(&attributes);

    /* Look it up */

    init_match(&invocation->context->match, player, name, NOTYPE);
    match_everything(&invocation->context->match, MAT_EXIT_PARENTS);
    thing = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  }

  /*
   * Check for the /debug switch
   */

  if (key == EXAM_DEBUG) {
    if (!is_examinable(evaluation, player, thing)) {
      notify_quiet(evaluation, player, "Permission denied.");
    } else {
      debug_examine(evaluation, player, thing);
    }
    return;
  }
  control = (is_examinable(evaluation, player, thing) ||
             can_link_exit(evaluation, player, thing));

  if (control) {
    buf2 = unparse_object(evaluation->world->database, evaluation, player,
                          thing, 0);
    notify(evaluation, player, buf2);
    free_lbuf(buf2);
    if (world->configuration->ex_flags) {
      buf2 = flag_description(evaluation->world->database, player, thing);
      notify(evaluation, player, buf2);
      free_mbuf(buf2);
    }
  } else {
    if ((key == EXAM_DEFAULT) && !world->configuration->exam_public) {
      if (world->configuration->read_rem_name) {
        buf2 = alloc_lbuf("do_examine.pub_name");
        StringCopy(buf2, game_object_name(evaluation->world->database, thing));
        notify_printf(
            evaluation, player, "%s is owned by %s", buf2,
            game_object_name(
                evaluation->world->database,
                game_object_owner(evaluation->world->database, thing)));
        free_lbuf(buf2);
      } else {
        notify_printf(
            evaluation, player, "Owned by %s",
            game_object_name(
                evaluation->world->database,
                game_object_owner(evaluation->world->database, thing)));
      }
      return;
    }
  }

  temp = alloc_lbuf("do_examine.info");

  if (control || world->configuration->read_rem_desc ||
      nearby(evaluation->world->database, player, thing)) {
    temp = attribute_get_string(evaluation->world->database, temp, thing,
                                A_DESC, &aowner, &aflags);
    if (*temp) {
      if (is_examinable(evaluation, player, thing) ||
          (aowner == game_object_owner(evaluation->world->database, player))) {
        view_atr(evaluation, player, thing,
                 attribute_by_number(evaluation->world->database, A_DESC), temp,
                 aowner, aflags, 1);
      } else {
        show_desc(&invocation->context->evaluation, player, thing, 0);
      }
    }
  } else {
    notify(evaluation, player, "<Too far away to get a good look>");
  }

  if (control) {

    /*
     * print owner, key, and value
     */

    buf2 = attribute_get(evaluation->world->database, thing, A_LOCK, &aowner,
                         &aflags);
    boolexp = boolean_expression_parse(evaluation->world->database, nullptr,
                                       player, buf2, 1);
    boolean_expression_unparse(evaluation->world->database, evaluation,
                               lock_text, player, boolexp);
    boolean_expression_free(boolexp);
    StringCopy(buf2, game_object_name(evaluation->world->database,
                                      game_object_owner(
                                          evaluation->world->database, thing)));
    notify_printf(evaluation, player, "Owner: %s  Key: %s", buf2, lock_text);
    free_lbuf(buf2);

    if (world->configuration->have_zones) {
      buf2 = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_zone(evaluation->world->database, thing), 0);
      notify_printf(evaluation, player, "Zone: %s", buf2);
      free_lbuf(buf2);
    }
    /*
     * print parent
     */

    loc = game_object_parent(evaluation->world->database, thing);
    if (loc != NOTHING) {
      buf2 = unparse_object(evaluation->world->database, evaluation, player,
                            loc, 0);
      notify_printf(evaluation, player, "Parent: %s", buf2);
      free_lbuf(buf2);
    }
    buf2 = power_description(evaluation->world->database, player, thing);
    notify(evaluation, player, buf2);
    free_mbuf(buf2);
  }
  if (key != EXAM_BRIEF)
    look_atrs(evaluation, player, thing, do_parent);

  /*
   * show him interesting stuff
   */

  if (control) {

    /*
     * Contents
     */

    if (game_object_contents(evaluation->world->database, thing) != NOTHING) {
      notify(evaluation, player, "Contents:");
      DOLIST(evaluation->world->database, content,
             game_object_contents(evaluation->world->database, thing)) {
        buf2 = unparse_object(evaluation->world->database, evaluation, player,
                              content, 0);
        notify(evaluation, player, buf2);
        free_lbuf(buf2);
      }
    }
    /*
     * Show stuff that depends on the object type
     */

    switch (typeof_obj(evaluation->world->database, thing)) {
    case TYPE_ROOM:

      /*
       * tell him about exits
       */

      if (game_object_exits(evaluation->world->database, thing) != NOTHING) {
        notify(evaluation, player, "Exits:");
        DOLIST(evaluation->world->database, exit,
               game_object_exits(evaluation->world->database, thing)) {
          buf2 = unparse_object(evaluation->world->database, evaluation, player,
                                exit, 0);
          notify(evaluation, player, buf2);
          free_lbuf(buf2);
        }
      } else {
        notify(evaluation, player, "No exits.");
      }

      /*
       * print dropto if present
       */

      if (game_object_location(evaluation->world->database, thing) != NOTHING) {
        buf2 = unparse_object(
            evaluation->world->database, evaluation, player,
            game_object_location(evaluation->world->database, thing), 0);
        notify_printf(evaluation, player, "Dropped objects go to: %s", buf2);
        free_lbuf(buf2);
      }
      break;
    case TYPE_THING:
    case TYPE_PLAYER:

      /*
       * tell him about exits
       */

      if (game_object_exits(evaluation->world->database, thing) != NOTHING) {
        notify(evaluation, player, "Exits:");
        DOLIST(evaluation->world->database, exit,
               game_object_exits(evaluation->world->database, thing)) {
          buf2 = unparse_object(evaluation->world->database, evaluation, player,
                                exit, 0);
          notify(evaluation, player, buf2);
          free_lbuf(buf2);
        }
      } else {
        notify(evaluation, player, "No exits.");
      }

      /*
       * print home
       */

      loc = game_object_link(evaluation->world->database, thing);
      buf2 = unparse_object(evaluation->world->database, evaluation, player,
                            loc, 0);
      notify_printf(evaluation, player, "Home: %s", buf2);
      free_lbuf(buf2);

      /*
       * print location if player can link to it
       */

      loc = game_object_location(evaluation->world->database, thing);
      if ((game_object_location(evaluation->world->database, thing) !=
           NOTHING) &&
          (is_examinable(evaluation, player, loc) ||
           is_examinable(evaluation, player, thing) ||
           is_linkable(evaluation, player, loc))) {
        buf2 = unparse_object(evaluation->world->database, evaluation, player,
                              loc, 0);
        notify_printf(evaluation, player, "Location: %s", buf2);
        free_lbuf(buf2);
      }
      break;
    case TYPE_EXIT:
      buf2 = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_exits(evaluation->world->database, thing), 0);
      notify_printf(evaluation, player, "Source: %s", buf2);
      free_lbuf(buf2);

      /*
       * print destination
       */

      switch (game_object_location(evaluation->world->database, thing)) {
      case NOTHING:
        break;
      case HOME:
        notify(evaluation, player, "Destination: *HOME*");
        break;
      default:
        buf2 = unparse_object(
            evaluation->world->database, evaluation, player,
            game_object_location(evaluation->world->database, thing), 0);
        notify_printf(evaluation, player, "Destination: %s", buf2);
        free_lbuf(buf2);
        break;
      }
      break;
    default:
      break;
    }
  } else if (!is_opaque(evaluation->world->database, thing) &&
             nearby(evaluation->world->database, player, thing)) {
    if (has_contents(evaluation->world->database, thing))
      look_contents(evaluation, player, thing, "Contents:", CONTENTS_REMOTE);
    if (typeof_obj(evaluation->world->database, thing) != TYPE_EXIT)
      look_exits(evaluation, player, thing, "Obvious exits:");
  }
  free_lbuf(temp);

  if (!control) {
    if (world->configuration->read_rem_name) {
      buf2 = alloc_lbuf("do_examine.pub_name");
      StringCopy(buf2, game_object_name(evaluation->world->database, thing));
      notify_printf(evaluation, player, "%s is owned by %s", buf2,
                    game_object_name(
                        evaluation->world->database,
                        game_object_owner(evaluation->world->database, thing)));
      free_lbuf(buf2);
    } else {
      notify_printf(evaluation, player, "Owned by %s",
                    game_object_name(
                        evaluation->world->database,
                        game_object_owner(evaluation->world->database, thing)));
    }
  }
}

void do_inventory(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  DbRef thing;
  char *buff, *s, *e;

  thing = game_object_contents(evaluation->world->database, player);
  if (thing == NOTHING) {
    notify(evaluation, player, "You aren't carrying anything.");
  } else {
    notify(evaluation, player, "You are carrying:");
    DOLIST(evaluation->world->database, thing, thing) {
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing, 1);
      notify(evaluation, player, buff);
      free_lbuf(buff);
    }
  }

  thing = game_object_exits(evaluation->world->database, player);
  if (thing != NOTHING) {
    notify(evaluation, player, "Exits:");
    e = buff = alloc_lbuf("look_exits");
    DOLIST(evaluation->world->database, thing, thing) {
      /*
       * chop off first exit alias to display
       */
      for (s = game_object_name(evaluation->world->database, thing);
           *s && (*s != ';'); s++)
        safe_chr(*s, buff, &e);
      safe_str("  ", buff, &e);
    }
    *e = 0;
    notify(evaluation, player, buff);
    free_lbuf(buff);
  }
}

void do_entrances(CommandInvocation *invocation) {
  WorldContext *world = invocation->context->world;
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  char *name = invocation->first;
  DbRef thing, i, j;
  char *exit, *message;
  int control_thing, count;
  long low_bound, high_bound;
  FWDLIST *fp;

  parse_range(world->database, world->configuration, &name, &low_bound,
              &high_bound);
  if (!name || !*name) {
    if (has_location(evaluation->world->database, player))
      thing = game_object_location(evaluation->world->database, player);
    else
      thing = player;
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  } else {
    init_match(&invocation->context->match, player, name, NOTYPE);
    match_everything(&invocation->context->match, MAT_EXIT_PARENTS);
    thing = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  }

  message = alloc_lbuf("do_entrances");
  control_thing = is_examinable(evaluation, player, thing);
  count = 0;
  for (i = low_bound; i <= high_bound; i++) {
    if (control_thing || is_examinable(evaluation, player, i)) {
      switch (typeof_obj(evaluation->world->database, i)) {
      case TYPE_EXIT:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit = unparse_object(
              evaluation->world->database, evaluation, player,
              game_object_exits(evaluation->world->database, i), 0);
          notify_printf(evaluation, player, "%s (%s)", exit,
                        game_object_name(evaluation->world->database, i));
          free_lbuf(exit);
          count++;
        }
        break;
      case TYPE_ROOM:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, player,
                                i, 0);
          notify_printf(evaluation, player, "%s [dropto]", exit);
          free_lbuf(exit);
          count++;
        }
        break;
      case TYPE_THING:
      case TYPE_PLAYER:
        if (game_object_link(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, player,
                                i, 0);
          notify_printf(evaluation, player, "%s [home]", exit);
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

      if (game_object_parent(evaluation->world->database, i) == thing) {
        exit = unparse_object(evaluation->world->database, evaluation, player,
                              i, 0);
        notify_printf(evaluation, player, "%s [parent]", exit);
        free_lbuf(exit);
        count++;
      }
      /*
       * Check for forwarding
       */

      if (has_fwdlist(evaluation->world->database, i)) {
        fp = fwdlist_get(evaluation->world->database, i);
        if (!fp)
          continue;
        for (j = 0; j < fp->count; j++) {
          if (fp->data[j] != thing)
            continue;
          exit = unparse_object(evaluation->world->database, evaluation, player,
                                i, 0);
          notify_printf(evaluation, player, "%s [forward]", exit);
          free_lbuf(exit);
          count++;
        }
      }
    }
  }
  free_lbuf(message);
  notify_printf(evaluation, player, "%d entrance%s found.", count,
                (count == 1) ? "" : "s");
}

/*
 * check the current location for bugs
 */

static void sweep_check(EvaluationContext *evaluation, DbRef player, DbRef what,
                        int key, int is_loc) {
  WorldContext *world = evaluation->world;
  DbRef aowner, parent;
  int canhear, cancom, isplayer, ispuppet, isconnected, attr;
  long aflags;
  int is_parent, lev;
  char *buf, *buf2, *bp, *as, *buff, *s;
  Attribute *ap;

  if (is_dark(evaluation->world->database, what) &&
      !is_wizard(evaluation->world->database, player) &&
      !world->configuration->sweep_dark)
    return;
  canhear = 0;
  cancom = 0;
  isplayer = 0;
  ispuppet = 0;
  isconnected = 0;
  is_parent = 0;

  if ((key & SWEEP_LISTEN) &&
      (((typeof_obj(evaluation->world->database, what) == TYPE_EXIT) ||
        is_loc) &&
       is_audible(evaluation->world->database, what))) {
    canhear = 1;
  } else if (key & SWEEP_LISTEN) {
    if (is_monitor(evaluation->world->database, what))
      buff = alloc_lbuf("Hearer");
    else
      buff = nullptr;

    for (attr = attribute_list_first(evaluation->world->database, what, &as);
         attr; attr = attribute_list_next(&as)) {
      if (attr == A_LISTEN) {
        canhear = 1;
        break;
      }
      if (buff && is_monitor(evaluation->world->database, what)) {
        ap = attribute_by_number(evaluation->world->database, attr);
        if (!ap || (ap->flags & AF_NOPROG))
          continue;

        attribute_get_string(evaluation->world->database, buff, what, attr,
                             &aowner, &aflags);

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
  if ((key & SWEEP_COMMANDS) &&
      (typeof_obj(evaluation->world->database, what) != TYPE_EXIT)) {

    /*
     * Look for commands on the object and parents too
     */

    ITER_PARENTS(world->database, world->configuration, what, parent, lev) {
      if (has_commands(evaluation->world->database, parent)) {
        cancom = 1;
        if (lev) {
          is_parent = 1;
          break;
        }
      }
    }
  }
  if (key & SWEEP_CONNECT) {
    if (is_connected(evaluation->world->database, what) ||
        (is_puppet(evaluation->world->database, what) &&
         is_connected(evaluation->world->database,
                      game_object_owner(evaluation->world->database, what))) ||
        (world->configuration->player_listen &&
         (typeof_obj(evaluation->world->database, what) == TYPE_PLAYER) &&
         canhear &&
         is_connected(evaluation->world->database,
                      game_object_owner(evaluation->world->database, what))))
      isconnected = 1;
  }
  if (key & SWEEP_PLAYER || isconnected) {
    if (typeof_obj(evaluation->world->database, what) == TYPE_PLAYER)
      isplayer = 1;
    if (is_puppet(evaluation->world->database, what))
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
      safe_str("is_puppet(evaluation->world->database, ", buf, &bp);
      safe_str(game_object_name(
                   evaluation->world->database,
                   game_object_owner(evaluation->world->database, what)),
               buf, &bp);
      safe_str(") ", buf, &bp);
    }
    if (isconnected)
      safe_str("connected ", buf, &bp);
    if (is_parent)
      safe_str("parent ", buf, &bp);
    bp[-1] = '\0';
    if (typeof_obj(evaluation->world->database, what) != TYPE_EXIT) {
      notify_printf(evaluation, player, "  %s is listening. [%s]",
                    game_object_name(evaluation->world->database, what), buf);
    } else {
      buf2 = alloc_lbuf("sweep_check.name");
      StringCopy(buf2, game_object_name(evaluation->world->database, what));
      for (bp = buf2; *bp && (*bp != ';'); bp++)
        ;
      *bp = '\0';
      notify_printf(evaluation, player, "  %s is listening. [%s]", buf2, buf);
      free_lbuf(buf2);
    }
    free_lbuf(buf);
  }
}

void do_sweep(CommandInvocation *invocation) {
  WorldContext *world = invocation->context->world;
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *where = invocation->first;
  DbRef here, sweeploc;
  int where_key, what_key;

  where_key = key & (SWEEP_ME | SWEEP_HERE | SWEEP_EXITS);
  what_key =
      key & (SWEEP_COMMANDS | SWEEP_LISTEN | SWEEP_PLAYER | SWEEP_CONNECT);

  if (where && *where) {
    sweeploc = match_controlled(&invocation->context->match, player, where);
    if (!is_good_obj(evaluation->world->database, sweeploc))
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
    notify(evaluation, player, "Sweeping location...");
    if (has_location(evaluation->world->database, sweeploc)) {
      here = game_object_location(evaluation->world->database, sweeploc);
      if ((here == NOTHING) || (is_dark(evaluation->world->database, here) &&
                                !world->configuration->sweep_dark &&
                                !is_examinable(evaluation, player, here))) {
        notify_quiet(evaluation, player,
                     "Sorry, it is dark here and you can't search for bugs");
        sweep_check(evaluation, player, sweeploc, what_key, 0);
      } else {
        sweep_check(evaluation, player, here, what_key, 1);
        for (here = game_object_contents(evaluation->world->database, here);
             here != NOTHING;
             here = game_object_next(evaluation->world->database, here))
          sweep_check(evaluation, player, here, what_key, 0);
      }
    } else {
      sweep_check(evaluation, player, sweeploc, what_key, 0);
    }
  }
  /*
   * Check exits in my location
   */

  if ((where_key & SWEEP_EXITS) &&
      has_location(evaluation->world->database, sweeploc)) {
    notify(evaluation, player, "Sweeping exits...");
    for (here = game_object_exits(
             evaluation->world->database,
             game_object_location(evaluation->world->database, sweeploc));
         here != NOTHING;
         here = game_object_next(evaluation->world->database, here))
      sweep_check(evaluation, player, here, what_key, 0);
  }
  /*
   * Check my inventory
   */

  if ((where_key & SWEEP_ME) &&
      has_contents(evaluation->world->database, sweeploc)) {
    notify(evaluation, player, "Sweeping inventory...");
    for (here = game_object_contents(evaluation->world->database, sweeploc);
         here != NOTHING;
         here = game_object_next(evaluation->world->database, here))
      sweep_check(evaluation, player, here, what_key, 0);
  }
  /*
   * Check carried exits
   */

  if ((where_key & SWEEP_EXITS) &&
      has_exits(evaluation->world->database, sweeploc)) {
    notify(evaluation, player, "Sweeping carried exits...");
    for (here = game_object_exits(evaluation->world->database, sweeploc);
         here != NOTHING;
         here = game_object_next(evaluation->world->database, here))
      sweep_check(evaluation, player, here, what_key, 0);
  }
  notify(evaluation, player, "Sweep complete.");
}
