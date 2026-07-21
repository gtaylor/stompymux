/*
 * object_set.c -- Commands that manipulate object properties and attributes
 */

#include "mux/world/object_set.h"

#include "p.glue.h"

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/support/validation.h"
#include "mux/world/match.h"
#include "mux/world/walkdb.h"

DbRef match_controlled(MatchContext *match, DbRef player, char *name) {
  DbRef mat;

  init_match(match, player, name, NOTYPE);
  match_everything(match, 0);
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
  match_everything(match, 0);
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

    oldalias = attribute_get(evaluation->world->database, thing, A_ALIAS,
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
                 ok_player_name(invocation->context->world->configuration,
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
    attribute_get_info(evaluation->world->database, thing, A_ALIAS, &aowner,
                       &aflags);

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

void object_attribute_set(EvaluationContext *evaluation,
                          const ServerConfiguration *configuration,
                          DbRef player, DbRef thing, int attrnum,
                          char *attrtext, int key) {
  DbRef aowner;
  long aflags;
  int have_xcode;
  Attribute *attr;

  attr = attribute_by_number(evaluation->world->database, attrnum);
  attribute_get_info(evaluation->world->database, thing, attrnum, &aowner,
                     &aflags);
  if (attr && set_attr(evaluation, player, thing, attr, aflags)) {
    have_xcode = is_hardcode(evaluation->world->database, thing);
    attribute_add(evaluation->world->database, thing, attrnum, attrtext,
                  game_object_owner(evaluation->world->database, player),
                  aflags);
    if (configuration->have_specials)
      handle_xcode(evaluation->btech, player, thing, have_xcode,
                   is_hardcode(evaluation->world->database, thing));
    if (!(key & SET_QUIET) && !is_quiet(evaluation->world->database, player) &&
        !is_quiet(evaluation->world->database, thing))
      notify_printf(evaluation, player, "%s/%s - %s",
                    game_object_name(evaluation->world->database, thing),
                    attr->name, strlen(attrtext) ? "Set." : "Cleared.");
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
            invocation->context->runtime->world_indexes, thing, player, flag,
            invocation->key);
}

void do_setattr(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  int attrnum = invocation->key;
  char *name = invocation->first;
  char *attrtext = invocation->second;
  DbRef thing;

  init_match(&invocation->context->match, player, name, NOTYPE);
  match_everything(&invocation->context->match, 0);
  thing = noisy_match_result(&invocation->context->match);

  if (thing == NOTHING)
    return;
  object_attribute_set(&invocation->context->evaluation,
                       invocation->context->world->configuration, player, thing,
                       attrnum, attrtext, 0);
}

/*
 * ---------------------------------------------------------------------------
 * * parse_attrib: parse a hardcoded native <obj>/<field> token.
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
    attribute_get_info(match->evaluation->world->database, *thing, attr->number,
                       &aowner, &aflags);
    if (!see_attr(match->evaluation, player, *thing, attr, aowner, aflags)) {
      *atr = NOTHING;
    } else {
      *atr = attr->number;
    }
  }
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
  if (nargs < 1 || !args[0] || !*args[0]) {
    notify_quiet(evaluation, player, "Nothing to do.");
    return;
  }
  char *pattern = strchr(it, '/');
  if (!pattern) {
    notify_quiet(evaluation, player, "Use @edit object/pattern=from,to.");
    return;
  }
  *pattern++ = '\0';
  DbRef dynamic_thing =
      match_controlled(&invocation->context->match, player, it);
  if (dynamic_thing == NOTHING)
    return;
  GameObject *object =
      game_database_object(evaluation->world->database, dynamic_thing);
  const char *replacement = nargs >= 2 ? args[1] : "";
  int edited = 0;
  for (int index = 0; index < object->at_count; index++) {
    if (!quick_wild(pattern, object->ahead[index].name))
      continue;
    char *result;
    char *display;
    edit_string_ansi(object->ahead[index].data, &result, &display, args[0],
                     replacement);
    dynamic_attribute_set(evaluation->world->database, dynamic_thing,
                          object->ahead[index].name, result);
    free_lbuf(result);
    free_lbuf(display);
    edited++;
  }
  notify_printf(evaluation, player, "%d attribute%s edited.", edited,
                edited == 1 ? "" : "s");
  return;
}

void do_use(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  char *object = invocation->first;
  char *df_use, *df_ouse;
  DbRef thing;
  int doit;
  LuaLockInvocation lock;
  LuaLockResult result;

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

  if (!lock_test(evaluation, player, invocation->cause, player, thing,
                 LUA_LOCK_USE, LUA_LOCK_OPERATION_USE, false, &lock, &result)) {
    notify_lock_failure(evaluation, &lock, &result,
                        "You can't figure out how to use that.", nullptr,
                        LUA_EVENT_USE_FAIL);
    return;
  }
  doit = 0;
  if (lua_message_defined(evaluation->runtime->lua_owner->runtime, thing,
                          LUA_MESSAGE_USE) ||
      lua_event_defined(evaluation->runtime->lua_owner->runtime, thing,
                        LUA_EVENT_USE))
    doit = 1;
  if (doit) {
    df_use = alloc_lbuf("do_use.use");
    df_ouse = alloc_lbuf("do_use.ouse");
    snprintf(df_use, LBUF_SIZE, "You use %s",
             game_object_name(evaluation->world->database, thing));
    snprintf(df_ouse, LBUF_SIZE, "uses %s",
             game_object_name(evaluation->world->database, thing));
    notify_action(&invocation->context->evaluation,
                  &(ActionMessageInvocation){
                      .message = {.type = LUA_MESSAGE_USE,
                                  .operation = LUA_MESSAGE_OPERATION_USE,
                                  .descriptor = invocation->context->descriptor,
                                  .object = thing,
                                  .enactor = player,
                                  .cause = invocation->cause,
                                  .source = NOTHING,
                                  .destination = NOTHING},
                      .enactor_default = df_use,
                      .other_default = df_ouse,
                      .event = LUA_EVENT_USE});
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
  DbRef thing;

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

  if (!ok_attr_name(arg1) || strlen(arg1) >= SBUF_SIZE) {
    notify_quiet(evaluation, player,
                 "That's not a good name for an attribute.");
    return;
  }
  thing = match_controlled(&invocation->context->match, player, s);
  if (thing == NOTHING)
    return;
  if (!dynamic_attribute_set(invocation->context->world->database, thing, arg1,
                             arg2)) {
    notify_quiet(evaluation, player, "Attribute update failed.");
    return;
  }
  if (!is_quiet(evaluation->world->database, player))
    notify_printf(evaluation, player, "%s/%s - %s.",
                  game_object_name(evaluation->world->database, thing), arg1,
                  arg2 && *arg2 ? "Set" : "Cleared");
}
