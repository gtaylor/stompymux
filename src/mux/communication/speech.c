/*
 * speech.c -- Commands which involve speaking
 */

#include <stdio.h>

#include "mux/commands/action_messages.h"
#include "mux/commands/command_handlers.h"
#include "mux/communication/speech.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/access.h"
#include "mux/world/object_spatial.h"

static int sp_ok(EvaluationContext *evaluation,
                 const ServerConfiguration *configuration, DbRef player) {
  LuaLockInvocation lock;
  LuaLockResult result;

  if (is_gagged(evaluation->world->database, player) &&
      (!(is_wizard(evaluation->world->database, player)))) {
    notify_checked(evaluation, player, player,
                   "Sorry. Gagged players cannot speak.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return 0;
  }

  if (is_auditorium(
          evaluation->world->database,
          game_object_location(evaluation->world->database, player))) {
    if (!lock_test(evaluation, player, player, player,
                   game_object_location(evaluation->world->database, player),
                   LUA_LOCK_SPEECH, LUA_LOCK_OPERATION_SPEAK, false, &lock,
                   &result)) {
      notify_lock_failure(&(LockFailureNotification){
          .evaluation = evaluation,
          .invocation = &lock,
          .result = &result,
          .enactor_default = "Sorry, you may not speak in this place."});
      return 0;
    }
  }
  return 1;
}

typedef struct ShoutRequest {
  EvaluationContext *evaluation;
  int target;
  const char *prefix;
  int flags;
  DbRef player;
  char *message;
} ShoutRequest;

static void say_shout(const ShoutRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  if (request->flags & SAY_NOTAG)
    raw_broadcast(
        evaluation->runtime->descriptors, request->target, "%s%s",
        game_object_name(evaluation->world->database, request->player),
        request->message);
  else
    raw_broadcast(
        evaluation->runtime->descriptors, request->target, "%s%s%s",
        request->prefix,
        game_object_name(evaluation->world->database, request->player),
        request->message);
}

static const char *announce_msg = "Announcement: ";
static const char *broadcast_msg = "Broadcast: ";
static const char *admin_msg = "Admin: ";

void do_say(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  int key = invocation->key;
  char *message = invocation->first;
  char plain_message[LBUF_SIZE];
  DbRef loc;
  char *buf2, *bp;
  int say_flags, depth;

  /*
   * Convert prefix-coded messages into the normal type
   */

  say_flags = key & (SAY_NOTAG | SAY_HERE | SAY_ROOM);
  key &= ~(SAY_NOTAG | SAY_HERE | SAY_ROOM);

  if (key == SAY_PREFIX) {
    const char prefix = *message;

    message = checked_mutable_string_suffix(message, *message ? 1 : 0);
    switch (prefix) {
    case '"':
      key = SAY_SAY;
      break;
    case ':':
      if (*message == ' ') {
        message = checked_mutable_string_suffix(message, 1);
        key = SAY_POSE_NOSPC;
      } else {
        key = SAY_POSE;
      }
      break;
    case ';':
      key = SAY_POSE_NOSPC;
      break;
    case '\\':
      key = SAY_EMIT;
      break;
    default:
      return;
    }
  }
  /*
   * Make sure speaker is somewhere if speaking in a place
   */

  loc = where_is(evaluation->world->database, player);
  switch (key) {
  case SAY_SAY:
  case SAY_POSE:
  case SAY_POSE_NOSPC:
  case SAY_EMIT:
    if (loc == NOTHING)
      return;
    if (!sp_ok(&invocation->context->evaluation,
               invocation->context->world->configuration, player))
      return;
    break;
  default:
    break;
  }

  /*
   * Send the message on its way
   */

  styled_text_strip(evaluation->world->styled_text_palette, message,
                    plain_message, sizeof(plain_message));
  message = plain_message;

  switch (key) {
  case SAY_SAY:
    notify_printf(evaluation, player, "You say \"%s\"", message);
    notify_excluding(&(ExcludingNotification){
        .evaluation = evaluation,
        .location = loc,
        .sender = player,
        .exceptions = {player},
        .exception_count = 1,
        .message = tprintf(
            "%s says \"%s\"",
            game_object_name(evaluation->world->database, player), message)});
    break;
  case SAY_POSE:
    notify_checked(
        evaluation, loc, player,
        tprintf("%s %s", game_object_name(evaluation->world->database, player),
                message),
        MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP | MSG_F_CONTENTS |
            MSG_S_INSIDE);
    break;
  case SAY_POSE_NOSPC:
    notify_checked(
        evaluation, loc, player,
        tprintf("%s%s", game_object_name(evaluation->world->database, player),
                message),
        MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP | MSG_F_CONTENTS |
            MSG_S_INSIDE);
    break;
  case SAY_EMIT:
    if ((say_flags & SAY_HERE) || !say_flags) {
      notify_checked(evaluation, loc, player, message,
                     MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP | MSG_F_CONTENTS |
                         MSG_S_INSIDE);
    }
    if (say_flags & SAY_ROOM) {
      if ((typeof_obj(evaluation->world->database, loc) == OBJECT_TYPE_ROOM) &&
          (say_flags & SAY_HERE)) {
        return;
      }
      depth = 0;
      while (
          (typeof_obj(evaluation->world->database, loc) != OBJECT_TYPE_ROOM) &&
          (depth++ < 20)) {
        loc = game_object_location(evaluation->world->database, loc);
        if ((loc == NOTHING) ||
            (loc == game_object_location(evaluation->world->database, loc)))
          return;
      }
      if (typeof_obj(evaluation->world->database, loc) == OBJECT_TYPE_ROOM) {
        notify_checked(evaluation, loc, player, message,
                       MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP |
                           MSG_F_CONTENTS | MSG_S_INSIDE);
      }
    }
    break;
  case SAY_SHOUT:
    switch (*message) {
    case ':':
      message[0] = ' ';
      say_shout(&(ShoutRequest){.evaluation = evaluation,
                                .prefix = announce_msg,
                                .flags = say_flags,
                                .player = player,
                                .message = message});
      break;
    case ';':
      message = checked_mutable_string_suffix(message, 1);
      say_shout(&(ShoutRequest){.evaluation = evaluation,
                                .prefix = announce_msg,
                                .flags = say_flags,
                                .player = player,
                                .message = message});
      break;
    case '"':
      message = checked_mutable_string_suffix(message, 1);
      [[fallthrough]];
    default:
      buf2 = alloc_lbuf("do_say.shout");
      bp = buf2;
      safe_str(" shouts \"", buf2, &bp);
      safe_str(message, buf2, &bp);
      safe_chr('"', buf2, &bp);
      *bp = '\0';
      say_shout(&(ShoutRequest){.evaluation = evaluation,
                                .prefix = announce_msg,
                                .flags = say_flags,
                                .player = player,
                                .message = buf2});
      free_lbuf(buf2);
    }
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "SHOUT") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.shout");
      (void)snprintf(buf2, LBUF_SIZE, " shouts: '%s'", message);
      log_text(buf2);
      free_lbuf(buf2);
      ENDLOG(evaluation->log);
    }
    break;

  case SAY_WIZSHOUT:
    switch (*message) {
    case ':':
      message[0] = ' ';
      say_shout(&(ShoutRequest){.evaluation = evaluation,
                                .target = OBJECT_FLAG_WIZARD,
                                .prefix = broadcast_msg,
                                .flags = say_flags,
                                .player = player,
                                .message = message});
      break;
    case ';':
      message = checked_mutable_string_suffix(message, 1);
      say_shout(&(ShoutRequest){.evaluation = evaluation,
                                .target = OBJECT_FLAG_WIZARD,
                                .prefix = broadcast_msg,
                                .flags = say_flags,
                                .player = player,
                                .message = message});
      break;
    case '"':
      message = checked_mutable_string_suffix(message, 1);
      [[fallthrough]];
    default:
      buf2 = alloc_lbuf("do_say.wizshout");
      bp = buf2;
      safe_str(" says \"", buf2, &bp);
      safe_str(message, buf2, &bp);
      safe_chr('"', buf2, &bp);
      *bp = '\0';
      say_shout(&(ShoutRequest){.evaluation = evaluation,
                                .target = OBJECT_FLAG_WIZARD,
                                .prefix = broadcast_msg,
                                .flags = say_flags,
                                .player = player,
                                .message = buf2});
      free_lbuf(buf2);
    }
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "BCAST") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.wizshout");
      (void)snprintf(buf2, LBUF_SIZE, " broadcasts: '%s'", message);
      log_text(buf2);
      free_lbuf(buf2);
      ENDLOG(evaluation->log);
    }
    break;

  case SAY_ADMINSHOUT:
    switch (*message) {
    case ':':
      message[0] = ' ';
      say_shout(&(ShoutRequest){.evaluation = evaluation,
                                .target = OBJECT_FLAG_WIZARD,
                                .prefix = admin_msg,
                                .flags = say_flags,
                                .player = player,
                                .message = message});
      break;
    case ';':
      message = checked_mutable_string_suffix(message, 1);
      say_shout(&(ShoutRequest){.evaluation = evaluation,
                                .target = OBJECT_FLAG_WIZARD,
                                .prefix = admin_msg,
                                .flags = say_flags,
                                .player = player,
                                .message = message});
      break;
    case '"':
      message = checked_mutable_string_suffix(message, 1);
      [[fallthrough]];
    default:
      buf2 = alloc_lbuf("do_say.adminshout");
      bp = buf2;
      safe_str(" says \"", buf2, &bp);
      safe_str(message, buf2, &bp);
      safe_chr('"', buf2, &bp);
      *bp = '\0';
      say_shout(&(ShoutRequest){.evaluation = evaluation,
                                .target = OBJECT_FLAG_WIZARD,
                                .prefix = admin_msg,
                                .flags = say_flags,
                                .player = player,
                                .message = buf2});
      free_lbuf(buf2);
    }
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "ASHOUT") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.adminshout");
      (void)snprintf(buf2, LBUF_SIZE, " yells: '%s'", message);
      log_text(buf2);
      free_lbuf(buf2);
      ENDLOG(evaluation->log);
    }
    break;

  case SAY_WALLPOSE:
    if (say_flags & SAY_NOTAG)
      raw_broadcast(invocation->context->runtime->descriptors, 0, "%s %s",
                    game_object_name(evaluation->world->database, player),
                    message);
    else
      raw_broadcast(
          invocation->context->runtime->descriptors, 0, "Announcement: %s %s",
          game_object_name(evaluation->world->database, player), message);
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "SHOUT") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.wallpose");
      (void)snprintf(buf2, LBUF_SIZE, " WALLposes: '%s'", message);
      log_text(buf2);
      free_lbuf(buf2);
      ENDLOG(evaluation->log);
    }
    break;

  case SAY_WIZPOSE:
    if (say_flags & SAY_NOTAG)
      raw_broadcast(invocation->context->runtime->descriptors,
                    OBJECT_FLAG_WIZARD, "%s %s",
                    game_object_name(evaluation->world->database, player),
                    message);
    else
      raw_broadcast(invocation->context->runtime->descriptors,
                    OBJECT_FLAG_WIZARD, "Broadcast: %s %s",
                    game_object_name(evaluation->world->database, player),
                    message);
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "BCAST") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.wizpose");
      (void)snprintf(buf2, LBUF_SIZE, " WIZposes: '%s'", message);
      log_text(buf2);
      free_lbuf(buf2);
      ENDLOG(evaluation->log);
    }
    break;

  case SAY_WALLEMIT:
    if (say_flags & SAY_NOTAG)
      raw_broadcast(invocation->context->runtime->descriptors, 0, "%s",
                    message);
    else
      raw_broadcast(invocation->context->runtime->descriptors, 0,
                    "Announcement: %s", message);
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "SHOUT") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.wallemit");
      (void)snprintf(buf2, LBUF_SIZE, " WALLemits: '%s'", message);
      log_text(buf2);
      free_lbuf(buf2);
      ENDLOG(evaluation->log);
    }
    break;

  case SAY_WIZEMIT:
    if (say_flags & SAY_NOTAG)
      raw_broadcast(invocation->context->runtime->descriptors,
                    OBJECT_FLAG_WIZARD, "%s", message);
    else
      raw_broadcast(invocation->context->runtime->descriptors,
                    OBJECT_FLAG_WIZARD, "Broadcast: %s", message);
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "BCAST") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.wizemit");
      (void)snprintf(buf2, LBUF_SIZE, " WIZemit: '%s'", message);
      log_text(buf2);
      free_lbuf(buf2);
      ENDLOG(evaluation->log);
    }
    break;
  default:
    break;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_page: Handle the page command.
 * * Page-pose code from shadow@prelude.cc.purdue.
 */
