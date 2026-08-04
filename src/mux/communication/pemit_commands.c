/*
 * speech.c -- Commands which involve speaking
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_helpers.h"
#include "mux/communication/comsys.h"
#include "mux/communication/speech.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include "mux/world/world_context.h"

void do_pemit_list(EvaluationContext *evaluation, DbRef player, char *list,
                   const char *message) {
  char *p;
  DbRef who;
  int ok_to_do;

  if (!message || !*message || !list || !*list)
    return;

  for (p = (char *)strtok(list, " "); p != nullptr;
       p = (char *)strtok(nullptr, " ")) {

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
      notify(evaluation, player, "Emit to whom?");
      break;
    case AMBIGUOUS:
      notify(evaluation, player, "I don't know who you mean!");
      break;
    default:
      if (!ok_to_do) {
        notify(evaluation, player, "You cannot do that.");
        break;
      }
      if (is_good_obj(evaluation->world->database, who))
        notify_with_cause(evaluation, who, player, message);
    }
  }
}

void do_pemit(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  int key = invocation->key;
  char *recipient = invocation->first;
  char *message = invocation->second;
  DbRef target, loc;
  int do_contents, ok_to_do, depth, pemit_flags;

  if (key & PEMIT_LIST) {
    do_pemit_list(evaluation, player, recipient, message);
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
    target = match_controlled(&evaluation->command->match, player, recipient);
    if (target == NOTHING)
      return;
    ok_to_do = 1;
    break;
  default:
    init_match(&evaluation->command->match, player, recipient,
               OBJECT_TYPE_PLAYER);
    match_everything(&evaluation->command->match, 0);
    target = match_result(&evaluation->command->match);
  }

  switch (target) {
  case NOTHING:
    switch (key) {
    case PEMIT_PEMIT:
      notify(evaluation, player, "Emit to whom?");
      break;
    case PEMIT_OEMIT:
      notify(evaluation, player, "Emit except to whom?");
      break;
    default:
      notify(evaluation, player, "Sorry.");
    }
    break;
  case AMBIGUOUS:
    notify(evaluation, player, "I don't know who you mean!");
    break;
  default:
    /*
     * Enforce locality constraints
     */

    if (!ok_to_do &&
        (nearby(evaluation->world->database, player, target) ||
         is_controls(evaluation->world->database, player, target))) {
      ok_to_do = 1;
    }
    if (!ok_to_do) {
      notify(evaluation, player, "You are too far away to do that.");
      return;
    }
    if (do_contents &&
        !is_controls(evaluation->world->database, player, target)) {
      notify(evaluation, player, "Permission denied.");
      return;
    }
    loc = where_is(evaluation->world->database, target);

    switch (key) {
    case PEMIT_PEMIT:
      if (do_contents) {
        if (has_contents(evaluation->world->database, target)) {
          notify_all_from_inside(evaluation, target, player, message);
        }
      } else {
        notify_with_cause(evaluation, target, player, message);
      }
      break;
    case PEMIT_OEMIT:
      notify_except(evaluation,
                    game_object_location(evaluation->world->database, target),
                    player, target, message);
      break;
    case PEMIT_FSAY:
      notify_printf(evaluation, target, "You say \"%s\"", message);
      if (loc != NOTHING) {
        notify_except(
            evaluation, loc, player, target,
            tprintf("%s says \"%s\"",
                    game_object_name(evaluation->world->database, target),
                    message));
      }
      break;
    case PEMIT_FPOSE:
      notify_all_from_inside(
          evaluation, loc, player,
          tprintf("%s %s",
                  game_object_name(evaluation->world->database, target),
                  message));
      break;
    case PEMIT_FPOSE_NS:
      notify_all_from_inside(
          evaluation, loc, player,
          tprintf("%s%s", game_object_name(evaluation->world->database, target),
                  message));
      break;
    case PEMIT_FEMIT:
      if ((pemit_flags & PEMIT_HERE) || !pemit_flags)
        notify_all_from_inside(evaluation, loc, player, message);
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
          notify_all_from_inside(evaluation, loc, player, message);
        }
      }
      break;
    default:
      break;
    }
  }
}
