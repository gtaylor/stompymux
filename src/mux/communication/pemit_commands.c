/*
 * speech.c -- Commands which involve speaking
 */

#include <string.h>

#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
#include "mux/communication/speech.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include "mux/world/object_spatial.h"

void do_pemit_list(EvaluationContext *evaluation, DbRef player, char *list,
                   const char *message) {
  char *p;
  DbRef who;
  int ok_to_do;
  char *token_context = nullptr;

  if (!message || !*message || !list || !*list)
    return;

  for (p = strtok_r(list, " ", &token_context); p != nullptr;
       p = strtok_r(nullptr, " ", &token_context)) {

    ok_to_do = 0;
    init_match(&evaluation->command->match, player, p, OBJECT_TYPE_PLAYER);
    match_everything(&evaluation->command->match, 0);
    who = match_result(&evaluation->command->match);

    if (!ok_to_do && (nearby(evaluation->world->database, player, who) ||
                      is_controls(evaluation->world->database, player, who))) {
      ok_to_do = 1;
    }
    switch (who) {
    case NOTHING:
      notify_checked(evaluation, player, player, "Emit to whom?",
                     MSG_ME_ALL | MSG_F_DOWN);
      break;
    case AMBIGUOUS:
      notify_checked(evaluation, player, player, "I don't know who you mean!",
                     MSG_ME_ALL | MSG_F_DOWN);
      break;
    default:
      if (!ok_to_do) {
        notify_checked(evaluation, player, player, "You cannot do that.",
                       MSG_ME_ALL | MSG_F_DOWN);
        break;
      }
      if (is_good_obj(evaluation->world->database, who))
        notify_checked(evaluation, who, player, message,
                       MSG_ME_ALL | MSG_F_DOWN);
    }
  }
}

void do_pemit(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef PLAYER = invocation->player;
  int key = invocation->key;
  char *recipient = invocation->first;
  char *message = invocation->second;
  DbRef target;
  DbRef loc;
  int do_contents;
  int ok_to_do;
  int depth;
  int pemit_flags;

  if (key & PEMIT_LIST) {
    do_pemit_list(evaluation, PLAYER, recipient, message);
    return;
  }
  if (key & PEMIT_CONTENTS) {
    do_contents = 1;
    key &= ~PEMIT_CONTENTS;
  } else {
    do_contents = 0;
  }
  pemit_flags = key & (PEMIT_HERE | PEMIT_ROOM);
  key &= ~(PEMIT_HERE | PEMIT_ROOM);
  ok_to_do = 0;

  switch (key) {
  case PEMIT_FSAY:
  case PEMIT_FPOSE:
  case PEMIT_FPOSE_NS:
  case PEMIT_FEMIT:
    target = match_controlled(&evaluation->command->match, PLAYER, recipient);
    if (target == NOTHING)
      return;
    ok_to_do = 1;
    break;
  default:
    init_match(&evaluation->command->match, PLAYER, recipient,
               OBJECT_TYPE_PLAYER);
    match_everything(&evaluation->command->match, 0);
    target = match_result(&evaluation->command->match);
  }

  switch (target) {
  case NOTHING:
    switch (key) {
    case PEMIT_PEMIT:
      notify_checked(evaluation, PLAYER, PLAYER, "Emit to whom?",
                     MSG_ME_ALL | MSG_F_DOWN);
      break;
    case PEMIT_OEMIT:
      notify_checked(evaluation, PLAYER, PLAYER, "Emit except to whom?",
                     MSG_ME_ALL | MSG_F_DOWN);
      break;
    default:
      notify_checked(evaluation, PLAYER, PLAYER, "Sorry.",
                     MSG_ME_ALL | MSG_F_DOWN);
    }
    break;
  case AMBIGUOUS:
    notify_checked(evaluation, PLAYER, PLAYER, "I don't know who you mean!",
                   MSG_ME_ALL | MSG_F_DOWN);
    break;
  default:
    /*
     * Enforce locality constraints
     */

    if (!ok_to_do &&
        (nearby(evaluation->world->database, PLAYER, target) ||
         is_controls(evaluation->world->database, PLAYER, target))) {
      ok_to_do = 1;
    }
    if (!ok_to_do) {
      notify_checked(evaluation, PLAYER, PLAYER,
                     "You are too far away to do that.",
                     MSG_ME_ALL | MSG_F_DOWN);
      return;
    }
    if (do_contents &&
        !is_controls(evaluation->world->database, PLAYER, target)) {
      notify_checked(evaluation, PLAYER, PLAYER, "Permission denied.",
                     MSG_ME_ALL | MSG_F_DOWN);
      return;
    }
    loc = where_is(evaluation->world->database, target);

    switch (key) {
    case PEMIT_PEMIT:
      if (do_contents) {
        if (has_contents(evaluation->world->database, target)) {
          notify_checked(evaluation, target, PLAYER, message,
                         MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP |
                             MSG_F_CONTENTS | MSG_S_INSIDE);
        }
      } else {
        notify_checked(evaluation, target, PLAYER, message,
                       MSG_ME_ALL | MSG_F_DOWN);
      }
      break;
    case PEMIT_OEMIT:
      notify_excluding(&(ExcludingNotification){
          .evaluation = evaluation,
          .location = game_object_location(evaluation->world->database, target),
          .sender = PLAYER,
          .exceptions = {target},
          .exception_count = 1,
          .message = message});
      break;
    case PEMIT_FSAY:
      notify_printf(evaluation, target, "You say \"%s\"", message);
      if (loc != NOTHING) {
        notify_excluding(&(ExcludingNotification){
            .evaluation = evaluation,
            .location = loc,
            .sender = PLAYER,
            .exceptions = {target},
            .exception_count = 1,
            .message =
                tprintf("%s says \"%s\"",
                        game_object_name(evaluation->world->database, target),
                        message)});
      }
      break;
    case PEMIT_FPOSE:
      notify_checked(
          evaluation, loc, PLAYER,
          tprintf("%s %s",
                  game_object_name(evaluation->world->database, target),
                  message),
          MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP | MSG_F_CONTENTS |
              MSG_S_INSIDE);
      break;
    case PEMIT_FPOSE_NS:
      notify_checked(
          evaluation, loc, PLAYER,
          tprintf("%s%s", game_object_name(evaluation->world->database, target),
                  message),
          MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP | MSG_F_CONTENTS |
              MSG_S_INSIDE);
      break;
    case PEMIT_FEMIT:
      if ((pemit_flags & PEMIT_HERE) || !pemit_flags)
        notify_checked(evaluation, loc, PLAYER, message,
                       MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP |
                           MSG_F_CONTENTS | MSG_S_INSIDE);
      if (pemit_flags & PEMIT_ROOM) {
        if ((typeof_obj(evaluation->world->database, loc) ==
             OBJECT_TYPE_ROOM) &&
            (pemit_flags & PEMIT_HERE)) {
          return;
        }
        depth = 0;
        while ((typeof_obj(evaluation->world->database, loc) !=
                OBJECT_TYPE_ROOM) &&
               (depth++ < 20)) {
          loc = game_object_location(evaluation->world->database, loc);
          if ((loc == NOTHING) ||
              (loc == game_object_location(evaluation->world->database, loc)))
            return;
        }
        if (typeof_obj(evaluation->world->database, loc) == OBJECT_TYPE_ROOM) {
          notify_checked(evaluation, loc, PLAYER, message,
                         MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP |
                             MSG_F_CONTENTS | MSG_S_INSIDE);
        }
      }
      break;
    default:
      break;
    }
  }
}
