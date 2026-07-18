/*
 * object_set.c -- Commands that manipulate object properties and attributes
 */

#include "mux/world/object_set.h"

#include "p.glue.h"

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/boolexp.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/world/match.h"
#include "mux/world/walkdb.h"

DbRef match_controlled(MatchContext *match, DbRef player, char *name) {
  DbRef mat;

  init_match(match, player, name, NOTYPE);
  match_everything(match, MAT_EXIT_PARENTS);
  mat = noisy_match_result(match);
  if (is_good_obj(match->evaluation->world->database, mat) &&
      !is_controls(match->evaluation, player, mat)) {
    notify_quiet(match->evaluation, player, "Permission denied.");
    return NOTHING;
  } else {
    return (mat);
  }
}

DbRef match_controlled_quiet(MatchContext *match, DbRef player, char *name) {
  DbRef mat;

  init_match(match, player, name, NOTYPE);
  match_everything(match, MAT_EXIT_PARENTS);
  mat = match_result(match);
  if (is_good_obj(match->evaluation->world->database, mat) &&
      !is_controls(match->evaluation, player, mat)) {
    return NOTHING;
  } else {
    return (mat);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_alias: Make an alias for a player or object.
 */

void do_alias(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *alias = invocation->second;
  DbRef thing, aowner;
  long aflags;
  Attribute *ap;
  char *oldalias, *trimalias;

  if ((thing = match_controlled(&invocation->context->match, player, name)) ==
      NOTHING)
    return;

  /*
   * check for renaming a player
   */

  ap = attribute_by_number(invocation->context->world->database, A_ALIAS);
  if (is_player(evaluation->world->database, thing)) {

    /*
     * Fetch the old alias
     */

    oldalias = attribute_parent_get(evaluation->world->database, thing, A_ALIAS,
                                    &aowner, &aflags);
    trimalias = trim_spaces(alias);

    if (!is_controls(evaluation, player, thing)) {

      /*
       * Make sure we have rights to do it.  We can't do *
       * * * * the normal Set_attr check because ALIAS is *
       * only * * * writable by GOD and we want to keep *
       * people * from * * doing &ALIAS and bypassing the *
       * player * name checks.
       */

      notify_quiet(evaluation, player, "Permission denied.");
    } else if (!*trimalias) {

      /*
       * New alias is null, just clear it
       */

      delete_player_name(invocation->context->world, thing, oldalias);
      attribute_clear(evaluation->world->database, thing, A_ALIAS);
      if (!is_quiet(evaluation->world->database, player))
        notify_quiet(evaluation, player, "Alias removed.");
    } else if (lookup_player(invocation->context->world, NOTHING, trimalias,
                             0) != NOTHING) {

      /*
       * Make sure new alias isn't already in use
       */

      notify_quiet(evaluation, player, "That name is already in use.");
    } else if (!(badname_check(invocation->context->world, trimalias) &&
                 ok_player_name(invocation->context->server->configuration,
                                trimalias))) {
      notify_quiet(evaluation, player, "That's a silly name for a player!");
    } else {

      /*
       * Remove the old name and add the new name
       */

      delete_player_name(invocation->context->world, thing, oldalias);
      attribute_add(evaluation->world->database, thing, A_ALIAS, trimalias,
                    game_object_owner(evaluation->world->database, player),
                    aflags);
      if (add_player_name(invocation->context->world, thing, trimalias)) {
        if (!is_quiet(evaluation->world->database, player))
          notify_quiet(evaluation, player, "Alias set.");
      } else {
        notify_quiet(
            evaluation, player,
            "That name is already in use or is illegal, alias cleared.");
        attribute_clear(evaluation->world->database, thing, A_ALIAS);
      }
    }
    free_lbuf(trimalias);
    free_lbuf(oldalias);
  } else {
    attribute_parent_get_info(evaluation->world->database, thing, A_ALIAS,
                              &aowner, &aflags);

    /*
     * Make sure we have rights to do it
     */

    if (!set_attr(evaluation, player, thing, ap, aflags)) {
      notify_quiet(evaluation, player, "Permission denied.");
    } else {
      attribute_add(evaluation->world->database, thing, A_ALIAS, alias,
                    game_object_owner(evaluation->world->database, player),
                    aflags);
      if (!is_quiet(evaluation->world->database, player))
        notify_quiet(evaluation, player, "Set.");
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_lock: Set a lock on an object or attribute.
 */

void do_lock(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  char *keytext = invocation->second;
  MatchContext *match = &invocation->context->match;
  DbRef thing, aowner;
  int atr;
  long aflags;
  Attribute *ap;
  struct BooleanExpression *okey;
  char lock_text[LBUF_SIZE];

  if (parse_attrib(match, player, name, &thing, &atr)) {
    if (atr != NOTHING) {
      if (!attribute_get_info(evaluation->world->database, thing, atr, &aowner,
                              &aflags)) {
        notify_quiet(evaluation, player, "Attribute not present on object.");
        return;
      }
      ap = attribute_by_number(invocation->context->world->database, atr);

      /*
       * You may lock an attribute if: * you could write *
       * * * the attribute if it were stored on * yourself
       * * * * * --and-- * you own the attribute or are a *
       * wizard as  *  * * long as * you are not #1 and are
       *
       * * trying to do * * something to #1.
       */

      if (ap && (is_god(evaluation->world->database, player) ||
                 (!is_god(evaluation->world->database, thing) &&
                  (set_attr(evaluation, player, player, ap, 0) &&
                   (is_wizard(evaluation->world->database, player) ||
                    aowner == game_object_owner(evaluation->world->database,
                                                player)))))) {
        aflags |= AF_LOCK;
        attribute_set_flags(evaluation->world->database, thing, atr, aflags);
        if (!is_quiet(evaluation->world->database, player) &&
            !is_quiet(evaluation->world->database, thing))
          notify_quiet(evaluation, player, "Attribute locked.");
      } else {
        notify_quiet(evaluation, player, "Permission denied.");
      }
      return;
    }
  }
  init_match(match, player, name, NOTYPE);
  match_everything(match, MAT_EXIT_PARENTS);
  thing = match_result(match);

  switch (thing) {
  case NOTHING:
    notify_quiet(evaluation, player, "I don't see what you want to lock!");
    return;
  case AMBIGUOUS:
    notify_quiet(evaluation, player,
                 "I don't know which one you want to lock!");
    return;
  default:
    if (!is_controls(evaluation, player, thing)) {
      notify_quiet(evaluation, player, "You can't lock that!");
      return;
    }
  }

  okey = boolean_expression_parse(invocation->context->world->database,
                                  &invocation->context->evaluation, player,
                                  keytext, 0);
  if (okey == TRUE_BOOLEXP) {
    notify_quiet(evaluation, player, "I don't understand that key.");
  } else {

    /*
     * everything ok, do it
     */

    if (!key)
      key = A_LOCK;
    boolean_expression_unparse_quiet(invocation->context->world->database,
                                     evaluation, lock_text, player, okey);
    attribute_add_raw(evaluation->world->database, thing, key, lock_text);
    if (!is_quiet(evaluation->world->database, player) &&
        !is_quiet(evaluation->world->database, thing))
      notify_quiet(evaluation, player, "Locked.");
  }
  boolean_expression_free(okey);
}

/*
 * ---------------------------------------------------------------------------
 * * Remove a lock from an object of attribute.
 */

void do_unlock(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  MatchContext *match = &invocation->context->match;
  DbRef thing, aowner;
  int atr;
  long aflags;
  Attribute *ap;

  if (parse_attrib(match, player, name, &thing, &atr)) {
    if (atr != NOTHING) {
      if (!attribute_get_info(evaluation->world->database, thing, atr, &aowner,
                              &aflags)) {
        notify_quiet(evaluation, player, "Attribute not present on object.");
        return;
      }
      ap = attribute_by_number(invocation->context->world->database, atr);

      /*
       * You may unlock an attribute if: * you could write
       * * * * the attribute if it were stored on *
       * yourself * * * * --and-- * you own the attribute
       * or are a * wizard * as  * long as * you are not #1
       * and are * trying to * do * something to #1.
       */

      if (ap && (is_god(evaluation->world->database, player) ||
                 ((!is_god(evaluation->world->database, thing)) &&
                  (set_attr(evaluation, player, player, ap, 0) &&
                   (is_wizard(evaluation->world->database, player) ||
                    aowner == game_object_owner(evaluation->world->database,
                                                player)))))) {
        aflags &= ~AF_LOCK;
        attribute_set_flags(evaluation->world->database, thing, atr, aflags);
        if (game_object_owner(
                evaluation->world->database,
                player !=
                    game_object_owner(evaluation->world->database, thing)))
          if (!is_quiet(evaluation->world->database, player) &&
              !is_quiet(evaluation->world->database, thing))
            notify_quiet(evaluation, player, "Attribute unlocked.");
      } else {
        notify_quiet(evaluation, player, "Permission denied.");
      }
      return;
    }
  }
  if (!key)
    key = A_LOCK;
  if ((thing = match_controlled(match, player, name)) != NOTHING) {
    attribute_clear(evaluation->world->database, thing, key);
    if (!is_quiet(evaluation->world->database, player) &&
        !is_quiet(evaluation->world->database, thing))
      notify_quiet(evaluation, player, "Unlocked.");
  }
}

void object_attribute_set(EvaluationContext *evaluation,
                          const ServerConfiguration *configuration,
                          DbRef player, DbRef thing, int attrnum,
                          char *attrtext, int key) {
  DbRef aowner;
  long aflags;
  int could_hear, have_xcode;
  Attribute *attr;

  attr = attribute_by_number(evaluation->world->database, attrnum);
  attribute_parent_get_info(evaluation->world->database, thing, attrnum,
                            &aowner, &aflags);
  if (attr && set_attr(evaluation, player, thing, attr, aflags)) {
    if ((attr->check != nullptr) &&
        (!(*attr->check)(evaluation, 0, player, thing, attrnum, attrtext)))
      return;
    have_xcode = is_hardcode(evaluation->world->database, thing);
    attribute_add(evaluation->world->database, thing, attrnum, attrtext,
                  game_object_owner(evaluation->world->database, player),
                  aflags);
    if (configuration->have_specials)
      handle_xcode(player, thing, have_xcode,
                   is_hardcode(evaluation->world->database, thing));
    if (!(key & SET_QUIET) && !is_quiet(evaluation->world->database, player) &&
        !is_quiet(evaluation->world->database, thing))
      notify_printf(evaluation, player, "%s/%s - %s",
                    game_object_name(evaluation->world->database, thing),
                    attr->name, strlen(attrtext) ? "Set." : "Cleared.");
    could_hear = is_hearer(evaluation, thing);
    handle_ears(evaluation, thing, could_hear, is_hearer(evaluation, thing));
  } else {
    notify_quiet(evaluation, player, "Permission denied.");
  }
}

void do_power(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *flag = invocation->second;
  DbRef thing;

  if (!flag || !*flag) {
    notify_quiet(evaluation, player, "I don't know what you want to set!");
    return;
  }
  /*
   * find thing
   */

  if ((thing = match_controlled(&invocation->context->match, player, name)) ==
      NOTHING)
    return;

  power_set(&invocation->context->evaluation,
            &invocation->context->server->world_indexes, thing, player, flag,
            invocation->key);
}

void do_setattr(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  int attrnum = invocation->key;
  char *name = invocation->first;
  char *attrtext = invocation->second;
  DbRef thing;

  init_match(&invocation->context->match, player, name, NOTYPE);
  match_everything(&invocation->context->match, MAT_EXIT_PARENTS);
  thing = noisy_match_result(&invocation->context->match);

  if (thing == NOTHING)
    return;
  object_attribute_set(&invocation->context->evaluation,
                       invocation->context->server->configuration, player,
                       thing, attrnum, attrtext, 0);
}

/*
 * ---------------------------------------------------------------------------
 * * parse_attrib, parse_attrib_wild: parse <obj>/<attr> tokens.
 */

int parse_attrib(MatchContext *match, DbRef player, char *str, DbRef *thing,
                 int *atr) {
  Attribute *attr;
  char *buff;
  DbRef aowner;
  long aflags;

  if (!str)
    return 0;

  /*
   * Break apart string into obj and attr.  Return on failure
   */

  buff = alloc_lbuf("parse_attrib");
  StringCopy(buff, str);
  if (!parse_thing_slash(match, player, buff, &str, thing)) {
    free_lbuf(buff);
    return 0;
  }
  /*
   * Get the named attribute from the object if we can
   */

  attr = attribute_by_name(match->evaluation->world->database, str);
  free_lbuf(buff);
  if (!attr) {
    *atr = NOTHING;
  } else {
    attribute_parent_get_info(match->evaluation->world->database, *thing,
                              attr->number, &aowner, &aflags);
    if (!see_attr(match->evaluation, player, *thing, attr, aowner, aflags)) {
      *atr = NOTHING;
    } else {
      *atr = attr->number;
    }
  }
  return 1;
}

static void find_wild_attrs(EvaluationContext *evaluation, DbRef player,
                            DbRef thing, char *str, int check_exclude,
                            int hash_insert, int get_locks,
                            ObjectList *attributes,
                            const ServerConfiguration *configuration,
                            WorldIndexes *indexes) {
  Attribute *attr;
  char *as;
  DbRef aowner;
  int ca, ok;
  long aflags;

  /*
   * Walk the attribute list of the object
   */

  for (ca = attribute_list_first(evaluation->world->database, thing, &as); ca;
       ca = attribute_list_next(&as)) {
    attr = attribute_by_number(evaluation->world->database, ca);

    /*
     * Discard bad attributes and ones we've seen before.
     */

    if (!attr)
      continue;

    if (check_exclude &&
        ((attr->flags & AF_PRIVATE) ||
         numeric_hash_table_find(ca, &indexes->parent_commands)))
      continue;

    /*
     * If we aren't the top level remember this attr so we * * *
     * exclude * it in any parents.
     */

    attribute_get_info(evaluation->world->database, thing, ca, &aowner,
                       &aflags);
    if (check_exclude && (aflags & AF_PRIVATE))
      continue;

    if (get_locks)
      ok = read_attr(evaluation, player, thing, attr, aowner, aflags);
    else
      ok = see_attr(evaluation, player, thing, attr, aowner, aflags);

    /*
     * Enforce locality restriction on descriptions
     */

    if (ok && (attr->number == A_DESC) && !configuration->read_rem_desc &&
        !is_examinable(evaluation, player, thing) &&
        !nearby(evaluation->world->database, player, thing))
      ok = 0;

    if (ok && quick_wild(str, attr->name)) {
      object_list_add(attributes, ca);
      if (hash_insert) {
        numeric_hash_table_add(ca, (int *)attr, &indexes->parent_commands);
      }
    }
  }
}

int parse_attrib_wild(MatchContext *match, DbRef player, char *str,
                      DbRef *thing, int check_parents, int get_locks,
                      int df_star, ObjectList *attributes,
                      const ServerConfiguration *configuration,
                      WorldIndexes *indexes) {
  char *buff;
  DbRef parent;
  int check_exclude, hash_insert, lev;

  if (!str)
    return 0;

  buff = alloc_lbuf("parse_attrib_wild");
  StringCopy(buff, str);

  /*
   * Separate name and attr portions at the first /
   */

  if (!parse_thing_slash(match, player, buff, &str, thing)) {

    /*
     * Not in obj/attr format, return if not defaulting to *
     */

    if (!df_star) {
      free_lbuf(buff);
      return 0;
    }
    /*
     * Look for the object, return failure if not found
     */

    init_match(match, player, buff, NOTYPE);
    match_everything(match, MAT_EXIT_PARENTS);
    *thing = match_result(match);

    if (!is_good_obj(match->evaluation->world->database, *thing)) {
      free_lbuf(buff);
      return 0;
    }
    /* str's declared type isn't const-correct (it's a cursor reassigned
       by parse_thing_slash(&invocation->context->match, ) above); "*" is only
       read from here on. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    str = (char *)"*";
#pragma clang diagnostic pop
  }
  /*
   * Check the object (and optionally all parents) for attributes
   */

  if (check_parents) {
    check_exclude = 0;
    hash_insert = check_parents;
    numeric_hash_table_flush(&indexes->parent_commands, 0);
    ITER_PARENTS(match->evaluation->world->database, configuration, *thing,
                 parent, lev) {
      if (!is_good_obj(
              match->evaluation->world->database,
              game_object_parent(match->evaluation->world->database, parent)))
        hash_insert = 0;
      find_wild_attrs(match->evaluation, player, parent, str, check_exclude,
                      hash_insert, get_locks, attributes, configuration,
                      indexes);
      check_exclude = 1;
    }
  } else {
    find_wild_attrs(match->evaluation, player, *thing, str, 0, 0, get_locks,
                    attributes, configuration, indexes);
  }
  free_lbuf(buff);
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * edit_string, edit_string_ansi, do_edit: Modify attributes.
 */

void edit_string(char *src, char **dst, const char *from, const char *to) {
  char *cp;

  /*
   * Do the substitution.  Idea for prefix/suffix from R'nice@TinyTIM
   */

  if (!strcmp(from, "^")) {
    /*
     * Prepend 'to' to string
     */

    *dst = alloc_lbuf("edit_string.^");
    cp = *dst;
    safe_str(to, *dst, &cp);
    safe_str(src, *dst, &cp);
    *cp = '\0';
  } else if (!strcmp(from, "$")) {
    /*
     * Append 'to' to string
     */

    *dst = alloc_lbuf("edit_string.$");
    cp = *dst;
    safe_str(src, *dst, &cp);
    safe_str(to, *dst, &cp);
    *cp = '\0';
  } else {
    /*
     * replace all occurances of 'from' with 'to'.  Handle the *
     * * * * special cases of from = \$ and \^.
     */

    if (((from[0] == '\\') || (from[0] == '%')) &&
        ((from[1] == '$') || (from[1] == '^')) && (from[2] == '\0'))
      from++;
    *dst = replace_string(from, to, src);
  }
}

static void edit_string_ansi(char *src, char **dst, char **returnstr,
                             const char *from, const char *to) {
  char *cp, *rp;

  /*
   * Do the substitution.  Idea for prefix/suffix from R'nice@TinyTIM
   */

  if (!strcmp(from, "^")) {
    /*
     * Prepend 'to' to string
     */

    *dst = alloc_lbuf("edit_string.^");
    cp = *dst;
    safe_str(to, *dst, &cp);
    safe_str(src, *dst, &cp);
    *cp = '\0';

    /*
     * Do the ansi string used to notify
     */
    *returnstr = alloc_lbuf("edit_string_ansi.^");
    rp = *returnstr;
    safe_str(ANSI_HILITE, *returnstr, &rp);
    safe_str(to, *returnstr, &rp);
    safe_str(ANSI_NORMAL, *returnstr, &rp);
    safe_str(src, *returnstr, &rp);
    *rp = '\0';

  } else if (!strcmp(from, "$")) {
    /*
     * Append 'to' to string
     */

    *dst = alloc_lbuf("edit_string.$");
    cp = *dst;
    safe_str(src, *dst, &cp);
    safe_str(to, *dst, &cp);
    *cp = '\0';

    /*
     * Do the ansi string used to notify
     */

    *returnstr = alloc_lbuf("edit_string_ansi.$");
    rp = *returnstr;
    safe_str(src, *returnstr, &rp);
    safe_str(ANSI_HILITE, *returnstr, &rp);
    safe_str(to, *returnstr, &rp);
    safe_str(ANSI_NORMAL, *returnstr, &rp);
    *rp = '\0';

  } else {
    /*
     * replace all occurances of 'from' with 'to'.  Handle the *
     * * * * special cases of from = \$ and \^.
     */

    if (((from[0] == '\\') || (from[0] == '%')) &&
        ((from[1] == '$') || (from[1] == '^')) && (from[2] == '\0'))
      from++;

    *dst = replace_string(from, to, src);
    *returnstr = replace_string(
        from, tprintf("%s%s%s", ANSI_HILITE, to, ANSI_NORMAL), src);
  }
}

void do_edit(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *it = invocation->first;
  char **args = invocation->vector;
  int nargs = invocation->vector_count;
  DbRef thing, aowner;
  int attr, got_one, doit;
  long aflags;
  char *from, *result, *returnstr, *atext;
  const char *to;
  Attribute *ap;
  ObjectList attributes;

  /*
   * Make sure we have something to do.
   */

  if ((nargs < 1) || !*args[0]) {
    notify_quiet(evaluation, player, "Nothing to do.");
    return;
  }
  from = args[0];
  to = (nargs >= 2) ? args[1] : "";

  /*
   * Look for the object and get the attribute (possibly wildcarded)
   */

  object_list_initialize(&attributes);
  if (!it || !*it ||
      !parse_attrib_wild(&invocation->context->match, player, it, &thing, 0, 0,
                         0, &attributes,
                         invocation->context->server->configuration,
                         &invocation->context->server->world_indexes)) {
    notify_quiet(evaluation, player, "No match.");
    object_list_destroy(&attributes);
    return;
  }
  /*
   * Iterate through matching attributes, performing edit
   */

  got_one = 0;
  atext = alloc_lbuf("do_edit.atext");

  for (attr = (int)object_list_first(&attributes); attr != NOTHING;
       attr = (int)object_list_next(&attributes)) {
    ap = attribute_by_number(invocation->context->world->database, attr);
    if (ap) {

      /*
       * Get the attr and make sure we can modify it.
       */

      attribute_get_string(evaluation->world->database, atext, thing,
                           ap->number, &aowner, &aflags);
      if (set_attr(evaluation, player, thing, ap, aflags)) {

        /*
         * Do the edit and save the result
         */

        got_one = 1;
        edit_string_ansi(atext, &result, &returnstr, from, to);
        if (ap->check != nullptr) {
          doit = (*ap->check)(&invocation->context->evaluation, 0, player,
                              thing, ap->number, result);
        } else {
          doit = 1;
        }
        if (doit) {
          attribute_add(evaluation->world->database, thing, ap->number, result,
                        game_object_owner(evaluation->world->database, player),
                        aflags);
          if (!is_quiet(evaluation->world->database, player))
            notify_quiet(evaluation, player,
                         tprintf("Set - %s: %s", ap->name, returnstr));
        }
        free_lbuf(result);
        free_lbuf(returnstr);
      } else {

        /*
         * No rights to change the attr
         */

        notify_quiet(evaluation, player,
                     tprintf("%s: Permission denied.", ap->name));
      }
    }
  }

  /*
   * Clean up
   */

  free_lbuf(atext);
  object_list_destroy(&attributes);

  if (!got_one) {
    notify_quiet(evaluation, player, "No matching attributes.");
  }
}

void do_trigger(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *object = invocation->first;
  char **argv = invocation->vector;
  int nargs = invocation->vector_count;
  DbRef thing, attrOwner;
  int attrib;
  long attrFlags;
  char objectName[MBUF_SIZE];
  char attributeName[MBUF_SIZE];
  Attribute *attribute;

  memset(objectName, 0, MBUF_SIZE);
  memset(attributeName, 0, MBUF_SIZE);

  if (!parse_attrib(&invocation->context->match, player, object, &thing,
                    &attrib) ||
      (attrib == NOTHING)) {
    notify_quiet(evaluation, player, "No match.");
    return;
  }
  if (!is_controls(evaluation, player, thing)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }

  attribute_get_string(evaluation->world->database, objectName, thing, A_NAME,
                       &attrOwner, &attrFlags);

  attribute = attribute_by_number(invocation->context->world->database, attrib);

  if (!attribute) {
    dprintk("braindamage, missing ATTR structure for dbref #%ld, attr %d.",
            thing, attrib);
  } else {
    strncpy(attributeName, attribute->name, MBUF_SIZE - 1);
  }

  did_it(&invocation->context->evaluation, player, thing, 0, nullptr, 0,
         nullptr, attrib, argv, nargs);

  /*
   * XXX be more descriptive as to what was triggered?
   */
  if (!(key & TRIG_QUIET) && !is_quiet(evaluation->world->database, player))
    notify_printf(&invocation->context->evaluation, player,
                  "%s/%s - Triggered.", objectName, attributeName);
}

void do_use(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  char *object = invocation->first;
  char *df_use, *df_ouse, *temp;
  DbRef thing, aowner;
  long aflags;
  int doit;

  init_match(&invocation->context->match, player, object, NOTYPE);
  match_neighbor(&invocation->context->match);
  match_possession(&invocation->context->match);
  if (is_wizard(evaluation->world->database, player)) {
    match_absolute(&invocation->context->match);
    match_player(&invocation->context->match);
  }
  match_me(&invocation->context->match);
  match_here(&invocation->context->match);
  thing = noisy_match_result(&invocation->context->match);
  if (thing == NOTHING)
    return;

  /*
   * Make sure player can use it
   */

  if (!could_doit_with_context(&invocation->context->evaluation, player, thing,
                               A_LUSE)) {
    did_it(&invocation->context->evaluation, player, thing, A_UFAIL,
           "You can't figure out how to use that.", A_OUFAIL, nullptr, A_AUFAIL,
           (char **)nullptr, 0);
    return;
  }
  temp = alloc_lbuf("do_use");
  doit = 0;
  if (*attribute_parent_get_string(evaluation->world->database, temp, thing,
                                   A_USE, &aowner, &aflags) ||
      *attribute_parent_get_string(evaluation->world->database, temp, thing,
                                   A_OUSE, &aowner, &aflags) ||
      *attribute_parent_get_string(evaluation->world->database, temp, thing,
                                   A_AUSE, &aowner, &aflags))
    doit = 1;
  free_lbuf(temp);

  if (doit) {
    df_use = alloc_lbuf("do_use.use");
    df_ouse = alloc_lbuf("do_use.ouse");
    snprintf(df_use, LBUF_SIZE, "You use %s",
             game_object_name(evaluation->world->database, thing));
    snprintf(df_ouse, LBUF_SIZE, "uses %s",
             game_object_name(evaluation->world->database, thing));
    did_it(&invocation->context->evaluation, player, thing, A_USE, df_use,
           A_OUSE, df_ouse, A_AUSE, (char **)nullptr, 0);
    free_lbuf(df_use);
    free_lbuf(df_ouse);
  } else {
    notify_quiet(evaluation, player, "You can't figure out how to use that.");
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_setvattr: Set a user-named (or possibly a predefined) attribute.
 */

void do_setvattr(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *arg1 = invocation->first;
  char *arg2 = invocation->second;
  char *s;
  int anum;

  arg1++; /*
           * skip the '&'
           */
  for (s = arg1; *s && !isspace(*s); s++)
    ; /*
       * take to the space
       */
  if (*s)
    *s++ = '\0'; /*
                  * split it
                  */

  anum = mkattr(invocation->context->world->database,
                arg1); /*
                        * Get or make attribute
                        */
  if (anum <= 0) {
    notify_quiet(evaluation, player,
                 "That's not a good name for an attribute.");
    return;
  }
  CommandInvocation set = *invocation;
  set.key = anum;
  set.first = s;
  set.second = arg2;
  do_setattr(&set);
}
