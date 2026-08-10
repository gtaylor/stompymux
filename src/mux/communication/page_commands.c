/*
 * speech.c -- Commands which involve speaking
 */

#include <stdlib.h>
#include <string.h>

#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_helpers.h"
#include "mux/communication/access_policy.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/player_account.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/match.h"
#include "mux/world/player.h"

static int page_check(EvaluationContext *evaluation,
                      const ServerConfiguration *configuration, DbRef player,
                      DbRef target) {
  if (is_in_character_location(evaluation->world->database, configuration,
                               player) &&
      !is_wizard(evaluation->world->database, target) &&
      !is_wizard(evaluation->world->database, player)) {
    notify_checked(evaluation, player, player, "Permission denied.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return 0;
  }
  if (!is_connected(evaluation->world->database, target)) {
    notify_checked(
        evaluation, player, target,
        tprintf("Sorry, %s is not connected.",
                game_object_name(evaluation->world->database, target)),
        MSG_ME_ALL | MSG_F_DOWN);
    return 0;
  }
  if (!is_wizard(evaluation->world->database, player) &&
      is_in_character_location(evaluation->world->database, configuration,
                               target) &&
      !is_wizard(evaluation->world->database, target)) {
    notify_checked(
        evaluation, player, target,
        tprintf("Sorry, %s is not accepting pages.",
                game_object_name(evaluation->world->database, target)),
        MSG_ME_ALL | MSG_F_DOWN);
    return 0;
  }
  return 1;
}

/*
 * Used in do_page
 */
typedef struct PageNameListRequest {
  WorldContext *world;
  DbRef player;
  char *list;
  char *names;
  bool dbrefs;
} PageNameListRequest;

static char *dbrefs_to_names(const PageNameListRequest *request) {
  WorldContext *world = request->world;
  DbRef player = request->player;
  char *list = request->list;
  char *namelist = request->names;
  bool ismessage = request->dbrefs;
  char *bp, *p;
  char oldlist[LBUF_SIZE];

  StringCopy(oldlist, list);
  bp = namelist;
  for (p = (char *)strtok(oldlist, " "); p != nullptr;
       p = (char *)strtok(nullptr, " ")) {
    if (ismessage) {
      DbRef target;
      if (parse_long_checked(p, &target))
        safe_str(tprintf("%s, ", game_object_name(world->database, target)),
                 namelist, &bp);
    } else {
      if (lookup_player(world, player, p, 1) != NOTHING) {
        safe_str(tprintf("%s, ",
                         game_object_name(world->database,
                                          lookup_player(world, player, p, 1))),
                 namelist, &bp);
      }
    }
  }
  if (bp != namelist) {
    const size_t length = strlen(namelist);

    if (length >= 2)
      *(char *)checked_storage_at(namelist, LBUF_SIZE, sizeof(char),
                                  length - 2) = '\0';
  }
  return bp;
}

void do_page(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const ServerConfiguration *configuration =
      invocation->context->world->configuration;
  const DbRef player = invocation->player;
  char *tname = invocation->first;
  char *message = invocation->second;
  char plain_message[LBUF_SIZE];
  DbRef target;
  char *p, *buf1, *bp, *buf2, *bp2, *mp, *str;
  char targetname[LBUF_SIZE];
  char alias[LBUF_SIZE];
  char aladd[LBUF_SIZE];
  int ispose = 0;
  int ismessage = 0;
  int count = 0;
  int n = 0;
  long aflags = 0;

  buf1 = alloc_lbuf("page_return_list");
  bp = buf1;

  buf2 = alloc_lbuf("page_list");
  bp2 = buf2;

  if ((tname[0] == ':') || (tname[0] == ';') || (message[0] == ':') ||
      (message[0] == ';'))
    ispose = 1;

  if (!*message) {
    char *target_cursor = targetname;
    for (size_t index = 0; index < player_account_last_page_count(
                                       evaluation->world->database, player);
         index++) {
      PlayerPageRecipientResult recipient =
          player_account_last_page_recipient(&(PlayerPageRecipientRequest){
              .account = {.database = evaluation->world->database,
                          .player = player},
              .position = index});
      if (!recipient.found)
        continue;
      if (index > 0)
        safe_chr(' ', targetname, &target_cursor);
      safe_str(tprintf("%ld", recipient.recipient), targetname, &target_cursor);
    }
    *target_cursor = '\0';
    if (!*tname) {
      if (!*targetname)
        notify_checked(evaluation, player, player, "You have not paged anyone.",
                       MSG_ME_ALL | MSG_F_DOWN);
      else
        for (p = (char *)strtok(targetname, " "); p != nullptr;
             p = (char *)strtok(nullptr, " ")) {
          if (parse_long_checked(p, &target))
            notify_printf(
                evaluation, player, "You last paged %s.",
                game_object_name(evaluation->world->database, target));
        }

      free_lbuf(buf1);
      free_lbuf(buf2);
      return;
    }
    StringCopy(message, tname);
    StringCopy(tname, targetname);
    ismessage = 1;
  }

  styled_text_strip(evaluation->world->styled_text_palette, message,
                    plain_message, sizeof(plain_message));
  message = plain_message;
  mp = message;

  attribute_get_string(evaluation->world->database, alias, player, A_ALIAS,
                       &aflags);
  if (*alias) {
    char *ap = aladd;

    safe_str(" (", aladd, &ap);
    safe_str(alias, aladd, &ap);
    safe_chr(')', aladd, &ap);
    *ap = '\0';
  } else
    aladd[0] = 0;

  /*
   * Count the words
   */
  for (n = 0, str = tname; str; str = (char *)next_token(str, ' '), n++)
    ;

  target = lookup_player(evaluation->world, player, tname, 1);
  if (target == NOTHING && n > 1) {
    bp = dbrefs_to_names(&(PageNameListRequest){.world = evaluation->world,
                                                .player = player,
                                                .list = tname,
                                                .names = buf1,
                                                .dbrefs = ismessage});
    for (p = (char *)strtok(tname, " "); p != nullptr;
         p = (char *)strtok(nullptr, " ")) {

      /*
       * If it's a memory page, grab the number from the *
       * * * list
       */
      if (ismessage) {
        if (!parse_long_checked(p, &target))
          continue;
      } else
        target = lookup_player(evaluation->world, player, p, 1);

      message = mp;

      if (target == NOTHING) {
        notify_printf(evaluation, player, "I don't recognize \"%s\".", p);
      } else if (!page_check(evaluation, configuration, player, target)) {
        ;
      } else {
        switch (*message) {
        case ':':
          notify_checked(
              evaluation, target, player,
              tprintf("From afar, to (%s):%s %s %s", buf1, aladd,
                      game_object_name(evaluation->world->database, player),
                      checked_string_suffix(message, 1)),
              MSG_ME_ALL | MSG_F_DOWN);
          break;
        case ';':
          message = checked_mutable_string_suffix(message, 1);
          notify_checked(
              evaluation, target, player,
              tprintf("From afar, to (%s):%s %s%s", buf1, aladd,
                      game_object_name(evaluation->world->database, player),
                      message),
              MSG_ME_ALL | MSG_F_DOWN);
          break;
        case '"':
          message = checked_mutable_string_suffix(message, 1);
          [[fallthrough]];
        default:
          notify_checked(
              evaluation, target, player,
              tprintf("To (%s), %s%s pages you: %s", buf1,
                      game_object_name(evaluation->world->database, player),
                      aladd, message),
              MSG_ME_ALL | MSG_F_DOWN);
        }
        safe_str(tprintf("%ld ", target), buf2, &bp2);
        count++;
      }
    }
  } else {
    if (ismessage && !parse_long_checked(tname, &target))
      target = NOTHING;
    if (target == NOTHING) {
      notify_printf(evaluation, player, "I don't recognize \"%s\".", tname);
    } else if (!page_check(evaluation, configuration, player, target)) {
      ;
    } else {

      switch (*message) {
      case ':':
        notify_checked(
            evaluation, target, player,
            tprintf("From afar,%s %s %s", aladd,
                    game_object_name(evaluation->world->database, player),
                    checked_string_suffix(message, 1)),
            MSG_ME_ALL | MSG_F_DOWN);
        break;
      case ';':
        message = checked_mutable_string_suffix(message, 1);
        notify_checked(
            evaluation, target, player,
            tprintf("From afar,%s %s%s", aladd,
                    game_object_name(evaluation->world->database, player),
                    message),
            MSG_ME_ALL | MSG_F_DOWN);
        break;
      case '"':
        message = checked_mutable_string_suffix(message, 1);
        [[fallthrough]];
      default:
        notify_checked(
            evaluation, target, player,
            tprintf("%s%s pages: %s",
                    game_object_name(evaluation->world->database, player),
                    aladd, message),
            MSG_ME_ALL | MSG_F_DOWN);
      }
      safe_str(tprintf("%ld ", target), buf2, &bp2);
      safe_str(tprintf("%s, ",
                       game_object_name(evaluation->world->database, target)),
               buf1, &bp);

      /* this is terminating the string above when there is no more to add to
       * the list removing the ", "
       */
      const size_t name_list_length = strlen(buf1);

      if (name_list_length >= 2)
        *(char *)checked_storage_at(buf1, LBUF_SIZE, sizeof(char),
                                    name_list_length - 2) = '\0';
      count++;
    }
  }

  if (count == 0) {
    free_lbuf(buf1);
    free_lbuf(buf2);
    return;
  }
  const size_t reference_list_length = strlen(buf2);

  if (reference_list_length > 0)
    *(char *)checked_storage_at(buf2, LBUF_SIZE, sizeof(char),
                                reference_list_length - 1) = '\0';
  DbRef *recipients = malloc((size_t)count * sizeof(*recipients));
  if (recipients) {
    size_t recipient_count = 0;
    for (char *token = strtok(buf2, " ");
         token && recipient_count < (size_t)count; token = strtok(nullptr, " "))
      *(DbRef *)checked_storage_at(recipients, (size_t)count,
                                   sizeof(*recipients), recipient_count++) =
          parse_dbref(token);
    player_account_last_page_set(evaluation->world->database, player,
                                 recipients, recipient_count);
    free(recipients);
  }

  if (count == 1) {
    if (*buf1) {
      if (ispose != 1) {
        notify_printf(evaluation, player, "You paged %s with '%s'.", buf1, mp);
      } else {
        if (*mp == ':')
          notify_printf(evaluation, player, "Long distance to %s: %s %s", buf1,
                        game_object_name(evaluation->world->database, player),
                        checked_string_suffix(mp, 1));
        else
          notify_printf(evaluation, player, "Long distance to %s: %s%s", buf1,
                        game_object_name(evaluation->world->database, player),
                        checked_string_suffix(mp, 1));
      }
    }
  } else {
    const size_t name_list_length = strlen(buf1);

    if (name_list_length >= 2) {
      *(char *)checked_storage_at(buf1, LBUF_SIZE, sizeof(char),
                                  name_list_length - 2) = ')';
      *(char *)checked_storage_at(buf1, LBUF_SIZE, sizeof(char),
                                  name_list_length - 1) = '\0';
    }

    if (*buf1) {
      if (ispose != 1) {
        notify_printf(evaluation, player, "You paged (%s with '%s'.", buf1, mp);
      } else {
        if (*mp == ':')
          notify_printf(evaluation, player, "Long distance to (%s: %s %s", buf1,
                        game_object_name(evaluation->world->database, player),
                        checked_string_suffix(mp, 1));
        else
          notify_printf(evaluation, player, "Long distance to (%s: %s%s", buf1,
                        game_object_name(evaluation->world->database, player),
                        checked_string_suffix(mp, 1));
      }
    }
  }

  free_lbuf(buf1);
  free_lbuf(buf2);
}
