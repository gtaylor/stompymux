/*
 * builder_commands.c -- Commands that create and configure world objects
 */

#include <string.h>
#include <strings.h>

#include "mux/commands/builder_commands_internal.h"
#include "mux/commands/command_handlers.h"
#include "mux/communication/comsys.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/name_table.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/validation.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include "mux/world/player.h"

extern NameTable indiv_attraccess_nametab[];

void do_chzone(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *newobj = invocation->second;
  DbRef thing;
  DbRef zone;

  init_match(&invocation->context->match, player, name, OBJECT_TYPE_NOTYPE);
  match_everything(&invocation->context->match, 0);
  if ((thing = noisy_match_result(&invocation->context->match)) == NOTHING)
    return;

  if (!strcasecmp(newobj, "none"))
    zone = NOTHING;
  else {
    init_match(&invocation->context->match, player, newobj, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    if ((zone = noisy_match_result(&invocation->context->match)) == NOTHING)
      return;

    if ((typeof_obj(evaluation->world->database, zone) != OBJECT_TYPE_THING) &&
        (typeof_obj(evaluation->world->database, zone) != OBJECT_TYPE_ROOM)) {
      notify_checked(evaluation, player, player, "Invalid zone object type.",
                     MSG_ME_ALL | MSG_F_DOWN);
      return;
    }
  }

  if (!is_controls(evaluation->world->database, player, thing)) {
    notify_checked(evaluation, player, player,
                   "You don't have the power to shift reality.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  /* The target zone must also be controllable by the actor. */
  if ((zone != NOTHING) &&
      !is_controls(evaluation->world->database, player, zone)) {
    notify_checked(evaluation, player, player,
                   "You cannot move that object to that zone.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  /*
   * only rooms may be zoned to other rooms
   */
  if ((zone != NOTHING) &&
      (typeof_obj(evaluation->world->database, zone) == OBJECT_TYPE_ROOM) &&
      typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_ROOM) {
    notify_checked(evaluation, player, player,
                   "Only rooms may be zoned to rooms.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  /*
   * everything is okay, do the change
   */
  game_object_set_zone(invocation->context->world->database, thing, zone);
  if (typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_PLAYER) {
    /*
     * if the object is a player, resetting these flags is rather
     * * * * * inconvenient -- although this may pose a bit of a *
     * *  * security * risk. Be careful when @chzone'ing wizard players.
     */
    game_object_set_flag(evaluation->world->database, thing, OBJECT_FLAG_WIZARD,
                         false);
    game_object_clear_powers(evaluation->world->database, thing);
  }
  notify_checked(evaluation, player, player, "Zone changed.",
                 MSG_ME_ALL | MSG_F_DOWN);
}
void do_name(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *newname = invocation->second;
  DbRef thing;
  char *buff;
  char new[LBUF_SIZE];
  char *compiled_name;

  if ((thing = match_controlled(&invocation->context->match, player, name)) ==
      NOTHING)
    return;
  compiled_name = builder_compile_object_name(evaluation, player, newname);
  if (!compiled_name)
    return;
  newname = compiled_name;

  /*
   * check for bad name
   */
  styled_text_strip(evaluation->world->styled_text_palette, newname, new,
                    sizeof(new));
  if (*newname == '\0' || strlen(new) == 0) {
    notify_checked(evaluation, player, player, "Give it what new name?",
                   MSG_ME);
    free_lbuf(compiled_name);
    return;
  }
  /*
   * check for renaming a player
   */
  if (is_player(evaluation->world->database, thing)) {

    styled_text_strip(evaluation->world->styled_text_palette, newname, new,
                      sizeof(new));
    buff = trim_spaces(new);
    if (!ok_player_name(invocation->context->world->configuration, buff) ||
        !badname_check(invocation->context->world, buff)) {
      notify_checked(evaluation, player, player, "You can't use that name.",
                     MSG_ME);
      free_lbuf(buff);
      free_lbuf(compiled_name);
      return;
    } else if (string_compare(
                   invocation->context->world->configuration, buff,
                   game_object_pure_name(invocation->context->world->database,
                                         thing)) &&
               (lookup_player(invocation->context->world, NOTHING, buff, 0) !=
                NOTHING)) {

      /*
       * string_compare allows changing foo to Foo, etc.
       */

      notify_checked(evaluation, player, player, "That name is already in use.",
                     MSG_ME);
      free_lbuf(buff);
      free_lbuf(compiled_name);
      return;
    }

    /*
     * everything ok, notify
     */
    STARTLOG(evaluation->log, LOG_SECURITY, "SEC", "CNAME") {
      log_name(evaluation->log, thing), log_text(" renamed to ");
      log_text(buff);
      ENDLOG(evaluation->log);
    }
    if (is_suspect(evaluation->world->database, thing)) {
      send_channel(
          evaluation, "Suspect", "%s",
          tprintf("%s renamed to %s",
                  game_object_name(invocation->context->world->database, thing),
                  buff));
    }
    delete_player_name(
        invocation->context->world, thing,
        game_object_pure_name(invocation->context->world->database, thing));

    object_name_set(invocation->context->world->database, thing, newname);
    add_player_name(
        invocation->context->world, thing,
        game_object_pure_name(invocation->context->world->database, thing));
    notify_checked(evaluation, player, player, "Name set.", MSG_ME);
    free_lbuf(buff);
    free_lbuf(compiled_name);
    return;
  } else {
    styled_text_strip(evaluation->world->styled_text_palette, newname, new,
                      sizeof(new));
    if (!ok_name(invocation->context->world->configuration, new)) {
      notify_checked(evaluation, player, player,
                     "That is not a reasonable name.", MSG_ME);
      free_lbuf(compiled_name);
      return;
    }
    /*
     * everything ok, change the name
     */
    object_name_set(invocation->context->world->database, thing, newname);
    notify_checked(evaluation, player, player, "Name set.", MSG_ME);
  }
  free_lbuf(compiled_name);
}
/*
 * ---------------------------------------------------------------------------
 * * do_unlink: Unlink an exit from its destination or remove a dropto.
 */

void do_unlink(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  DbRef exit;

  init_match(&invocation->context->match, player, name, OBJECT_TYPE_EXIT);
  match_everything(&invocation->context->match, 0);
  exit = match_result(&invocation->context->match);

  switch (exit) {
  case NOTHING:
    notify_checked(evaluation, player, player, "Unlink what?", MSG_ME);
    break;
  case AMBIGUOUS:
    notify_checked(evaluation, player, player,
                   "I don't know which one you mean!", MSG_ME);
    break;
  default:
    if (!is_controls(evaluation->world->database, player, exit)) {
      notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    } else {
      switch (typeof_obj(evaluation->world->database, exit)) {
      case OBJECT_TYPE_EXIT:
        game_object_set_location(evaluation->world->database, exit, NOTHING);
        notify_checked(evaluation, player, player, "Unlinked.", MSG_ME);
        break;
      case OBJECT_TYPE_ROOM:
        game_object_set_location(evaluation->world->database, exit, NOTHING);
        notify_checked(evaluation, player, player, "Dropto removed.", MSG_ME);
        break;
      default:
        notify_checked(evaluation, player, player, "You can't unlink that!",
                       MSG_ME);
        break;
      }
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_flag: Set flags on objects.
 */
void do_flag(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *flag = invocation->second;
  DbRef thing = match_controlled(&invocation->context->match, player, name);
  if (thing != NOTHING)
    flag_set(evaluation, invocation->context->world->indexes, thing, player,
             flag, 0);
}
