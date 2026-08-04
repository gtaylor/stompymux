/*
 * speech.c -- Commands which involve speaking
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_helpers.h"
#include "mux/communication/access_policy.h"
#include "mux/communication/comsys.h"
#include "mux/communication/speech.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/match.h"
#include "mux/world/world_context.h"

static int sp_ok(EvaluationContext *evaluation,
                 const ServerConfiguration *configuration, DbRef player) {
  LuaLockInvocation lock;
  LuaLockResult result;

  if (is_gagged(evaluation->world->database, player) &&
      (!(is_wizard(evaluation->world->database, player)))) {
    notify(evaluation, player, "Sorry. Gagged players cannot speak.");
    return 0;
  }

  if (is_auditorium(
          evaluation->world->database,
          game_object_location(evaluation->world->database, player))) {
    if (!lock_test(evaluation, player, player, player,
                   game_object_location(evaluation->world->database, player),
                   LUA_LOCK_SPEECH, LUA_LOCK_OPERATION_SPEAK, false, &lock,
                   &result)) {
      notify_lock_failure(evaluation, &lock, &result,
                          "Sorry, you may not speak in this place.", nullptr,
                          LUA_EVENT_NONE);
      return 0;
    }
  }
  return 1;
}

static void say_shout(EvaluationContext *evaluation, int target,
                      const char *prefix, int flags, DbRef player,
                      char *message) {
  if (flags & SAY_NOTAG)
    raw_broadcast(evaluation->runtime->descriptors, target, "%s%s",
                  game_object_name(evaluation->world->database, player),
                  message);
  else
    raw_broadcast(evaluation->runtime->descriptors, target, "%s%s%s", prefix,
                  game_object_name(evaluation->world->database, player),
                  message);
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
    switch (*message++) {
    case '"':
      key = SAY_SAY;
      break;
    case ':':
      if (*message == ' ') {
        message++;
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
    notify_except(evaluation, loc, player, player,
                  tprintf("%s says \"%s\"",
                          game_object_name(evaluation->world->database, player),
                          message));
    break;
  case SAY_POSE:
    notify_all_from_inside(
        evaluation, loc, player,
        tprintf("%s %s", game_object_name(evaluation->world->database, player),
                message));
    break;
  case SAY_POSE_NOSPC:
    notify_all_from_inside(
        evaluation, loc, player,
        tprintf("%s%s", game_object_name(evaluation->world->database, player),
                message));
    break;
  case SAY_EMIT:
    if ((say_flags & SAY_HERE) || !say_flags) {
      notify_all_from_inside(evaluation, loc, player, message);
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
        notify_all_from_inside(evaluation, loc, player, message);
      }
    }
    break;
  case SAY_SHOUT:
    switch (*message) {
    case ':':
      message[0] = ' ';
      say_shout(&invocation->context->evaluation, 0, announce_msg, say_flags,
                player, message);
      break;
    case ';':
      message++;
      say_shout(&invocation->context->evaluation, 0, announce_msg, say_flags,
                player, message);
      break;
    case '"':
      message++;
      [[fallthrough]];
    default:
      buf2 = alloc_lbuf("do_say.shout");
      bp = buf2;
      safe_str(" shouts \"", buf2, &bp);
      safe_str(message, buf2, &bp);
      safe_chr('"', buf2, &bp);
      *bp = '\0';
      say_shout(&invocation->context->evaluation, 0, announce_msg, say_flags,
                player, buf2);
      free_lbuf(buf2);
    }
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "SHOUT") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.shout");
      snprintf(buf2, LBUF_SIZE, " shouts: '%s'", message);
      log_text(buf2);
      free_lbuf(buf2);
      ENDLOG(evaluation->log);
    }
    break;

  case SAY_WIZSHOUT:
    switch (*message) {
    case ':':
      message[0] = ' ';
      say_shout(&invocation->context->evaluation, OBJECT_FLAG_WIZARD,
                broadcast_msg, say_flags, player, message);
      break;
    case ';':
      message++;
      say_shout(&invocation->context->evaluation, OBJECT_FLAG_WIZARD,
                broadcast_msg, say_flags, player, message);
      break;
    case '"':
      message++;
      [[fallthrough]];
    default:
      buf2 = alloc_lbuf("do_say.wizshout");
      bp = buf2;
      safe_str(" says \"", buf2, &bp);
      safe_str(message, buf2, &bp);
      safe_chr('"', buf2, &bp);
      *bp = '\0';
      say_shout(&invocation->context->evaluation, OBJECT_FLAG_WIZARD,
                broadcast_msg, say_flags, player, buf2);
      free_lbuf(buf2);
    }
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "BCAST") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.wizshout");
      snprintf(buf2, LBUF_SIZE, " broadcasts: '%s'", message);
      log_text(buf2);
      free_lbuf(buf2);
      ENDLOG(evaluation->log);
    }
    break;

  case SAY_ADMINSHOUT:
    switch (*message) {
    case ':':
      message[0] = ' ';
      say_shout(&invocation->context->evaluation, OBJECT_FLAG_WIZARD, admin_msg,
                say_flags, player, message);
      break;
    case ';':
      message++;
      say_shout(&invocation->context->evaluation, OBJECT_FLAG_WIZARD, admin_msg,
                say_flags, player, message);
      break;
    case '"':
      message++;
      [[fallthrough]];
    default:
      buf2 = alloc_lbuf("do_say.adminshout");
      bp = buf2;
      safe_str(" says \"", buf2, &bp);
      safe_str(message, buf2, &bp);
      safe_chr('"', buf2, &bp);
      *bp = '\0';
      say_shout(&invocation->context->evaluation, OBJECT_FLAG_WIZARD, admin_msg,
                say_flags, player, buf2);
      free_lbuf(buf2);
    }
    STARTLOG(evaluation->log, LOG_SHOUTS, "WIZ", "ASHOUT") {
      log_name(evaluation->log, player);
      buf2 = alloc_lbuf("do_say.LOG.adminshout");
      snprintf(buf2, LBUF_SIZE, " yells: '%s'", message);
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
      snprintf(buf2, LBUF_SIZE, " WALLposes: '%s'", message);
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
      snprintf(buf2, LBUF_SIZE, " WIZposes: '%s'", message);
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
      snprintf(buf2, LBUF_SIZE, " WALLemits: '%s'", message);
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
      snprintf(buf2, LBUF_SIZE, " WIZemit: '%s'", message);
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
