/*
 * speech.c -- Commands which involve speaking
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/commands/functions.h"
#include "mux/communication/comsys.h"
#include "mux/communication/speech.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"
#include "mux/world/world_context.h"

static int sp_ok(EvaluationContext *evaluation,
                 const ServerConfiguration *configuration, DbRef player) {
  if (is_gagged(evaluation->world->database, player) &&
      (!(is_wizard(evaluation->world->database, player)))) {
    notify(evaluation, player, "Sorry. Gagged players cannot speak.");
    return 0;
  }

  if (!configuration->robot_speak) {
    if (is_robot_player(evaluation->world->database, player) &&
        !is_controls(
            evaluation, player,
            game_object_location(evaluation->world->database, player))) {
      notify(evaluation, player, "Sorry robots may not speak in public.");
      return 0;
    }
  }
  if (is_auditorium(
          evaluation->world->database,
          game_object_location(evaluation->world->database, player))) {
    if (!could_doit_with_context(
            evaluation, player,
            game_object_location(evaluation->world->database, player),
            A_LSPEECH)) {
      notify(evaluation, player, "Sorry, you may not speak in this place.");
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

void do_think(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const DbRef cause = invocation->cause;
  char *message = invocation->first;
  char *str, buf[LBUF_SIZE], *bp;
  int output_length;

  bp = buf;
  str = message;
  exec(&invocation->context->evaluation, buf, &bp, 0, player, cause,
       EV_FCHECK | EV_EVAL | EV_TOP, &str, (char **)nullptr, 0);
  output_length = (int)strnlen(buf, LBUF_SIZE - 1);
  buf[output_length] = '\0';
  notify(evaluation, player, buf);
}

void do_say(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  int key = invocation->key;
  char *message = invocation->first;
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
      if ((typeof_obj(evaluation->world->database, loc) == TYPE_ROOM) &&
          (say_flags & SAY_HERE)) {
        return;
      }
      depth = 0;
      while ((typeof_obj(evaluation->world->database, loc) != TYPE_ROOM) &&
             (depth++ < 20)) {
        loc = game_object_location(evaluation->world->database, loc);
        if ((loc == NOTHING) ||
            (loc == game_object_location(evaluation->world->database, loc)))
          return;
      }
      if (typeof_obj(evaluation->world->database, loc) == TYPE_ROOM) {
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
      say_shout(&invocation->context->evaluation, WIZARD, broadcast_msg,
                say_flags, player, message);
      break;
    case ';':
      message++;
      say_shout(&invocation->context->evaluation, WIZARD, broadcast_msg,
                say_flags, player, message);
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
      say_shout(&invocation->context->evaluation, WIZARD, broadcast_msg,
                say_flags, player, buf2);
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
      say_shout(&invocation->context->evaluation, WIZARD, admin_msg, say_flags,
                player, message);
      break;
    case ';':
      message++;
      say_shout(&invocation->context->evaluation, WIZARD, admin_msg, say_flags,
                player, message);
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
      say_shout(&invocation->context->evaluation, WIZARD, admin_msg, say_flags,
                player, buf2);
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
      raw_broadcast(invocation->context->runtime->descriptors, WIZARD, "%s %s",
                    game_object_name(evaluation->world->database, player),
                    message);
    else
      raw_broadcast(
          invocation->context->runtime->descriptors, WIZARD, "Broadcast: %s %s",
          game_object_name(evaluation->world->database, player), message);
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
      raw_broadcast(invocation->context->runtime->descriptors, WIZARD, "%s",
                    message);
    else
      raw_broadcast(invocation->context->runtime->descriptors, WIZARD,
                    "Broadcast: %s", message);
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

static void page_return(EvaluationContext *evaluation,
                        const ServerConfiguration *configuration, DbRef player,
                        DbRef target, const char *tag, int anum,
                        const char *dflt) {
  DbRef aowner;
  long aflags;
  char *str, *str2, *buf, *bp;
  struct tm *tp;
  time_t t;

  str = attribute_parent_get(evaluation->world->database, target, anum, &aowner,
                             &aflags);
  if (*str) {
    str2 = bp = alloc_lbuf("page_return");
    buf = str;
    exec(evaluation, str2, &bp, 0, target, player,
         EV_FCHECK | EV_EVAL | EV_TOP | EV_NO_LOCATION, &buf, (char **)nullptr,
         0);
    *bp = '\0';
    if (*str2) {
      t = time(nullptr);
      tp = localtime(&t);
      if (is_wizard(evaluation->world->database, target) ||
          !is_in_character_location(evaluation->world->database, configuration,
                                    target))
        notify_with_cause(
            evaluation, player, target,
            tprintf("%s message from %s: %s", tag,
                    game_object_name(evaluation->world->database, target),
                    str2));
      notify_with_cause(
          evaluation, target, player,
          tprintf("[%d:%02d] %s message sent to %s.", tp->tm_hour, tp->tm_min,
                  tag, game_object_name(evaluation->world->database, player)));
    }
    free_lbuf(str2);
  } else if (dflt && *dflt) {
    notify_with_cause(evaluation, player, target, dflt);
  }
  free_lbuf(str);
}

static int page_check(EvaluationContext *evaluation,
                      const ServerConfiguration *configuration, DbRef player,
                      DbRef target) {
  if (is_in_character_location(evaluation->world->database, configuration,
                               player) &&
      !is_wizard(evaluation->world->database, target) &&
      !is_wizard(evaluation->world->database, player)) {
    notify(evaluation, player, "Permission denied.");
    return 0;
  }
  if (!is_connected(evaluation->world->database, target)) {
    page_return(evaluation, configuration, player, target, "Away", A_AWAY,
                tprintf("Sorry, %s is not connected.",
                        game_object_name(evaluation->world->database, target)));
    return 0;
  }
  if (!is_wizard(evaluation->world->database, player) &&
      is_in_character_location(evaluation->world->database, configuration,
                               target) &&
      !is_wizard(evaluation->world->database, target)) {
    if (is_wizard(evaluation->world->database, target) &&
        is_dark(evaluation->world->database, target))
      page_return(
          evaluation, configuration, player, target, "Away", A_AWAY,
          tprintf("Sorry, %s is not connected.",
                  game_object_name(evaluation->world->database, target)));
    else
      page_return(
          evaluation, configuration, player, target, "Reject", A_REJECT,
          tprintf("Sorry, %s is not accepting pages.",
                  game_object_name(evaluation->world->database, target)));
    return 0;
  }
  return 1;
}

/*
 * Used in do_page
 */
static char *dbrefs_to_names(WorldContext *world, DbRef player, char *list,
                             char *namelist, int ismessage) {
  char *bp, *p;
  char oldlist[LBUF_SIZE];

  StringCopy(oldlist, list);
  bp = namelist;
  for (p = (char *)strtok(oldlist, " "); p != nullptr;
       p = (char *)strtok(nullptr, " ")) {
    if (ismessage)
      safe_str(tprintf("%s, ", game_object_name(world->database, atoi(p))),
               namelist, &bp);
    else {
      if (lookup_player(world, player, p, 1) != NOTHING) {
        safe_str(tprintf("%s, ",
                         game_object_name(world->database,
                                          lookup_player(world, player, p, 1))),
                 namelist, &bp);
      }
    }
  }
  *(bp - 2) = '\0';
  return bp;
}

void do_page(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const ServerConfiguration *configuration =
      invocation->context->world->configuration;
  const DbRef player = invocation->player;
  char *tname = invocation->first;
  char *message = invocation->second;
  DbRef target, aowner;
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

  mp = message;

  if (!*message) {
    attribute_get_string(evaluation->world->database, targetname, player,
                         A_LASTPAGE, &aowner, &aflags);
    if (!*tname) {
      if (!*targetname)
        notify(evaluation, player, "You have not paged anyone.");
      else
        for (p = (char *)strtok(targetname, " "); p != nullptr;
             p = (char *)strtok(nullptr, " ")) {
          target = atoi(p);
          notify_printf(evaluation, player, "You last paged %s.",
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

  attribute_get_string(evaluation->world->database, alias, player, A_ALIAS,
                       &aowner, &aflags);
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

  if (((target = lookup_player(evaluation->world, player, tname, 1)) ==
       NOTHING) &&
      n > 1) {
    bp = dbrefs_to_names(evaluation->world, player, tname, buf1, ismessage);
    for (p = (char *)strtok(tname, " "); p != nullptr;
         p = (char *)strtok(nullptr, " ")) {

      /*
       * If it's a memory page, grab the number from the *
       * * * list
       */
      if (ismessage) {
        target = atoi(p);
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
          notify_with_cause(
              evaluation, target, player,
              tprintf("From afar, to (%s):%s %s %s", buf1, aladd,
                      game_object_name(evaluation->world->database, player),
                      message + 1));
          break;
        case ';':
          message++;
          notify_with_cause(
              evaluation, target, player,
              tprintf("From afar, to (%s):%s %s%s", buf1, aladd,
                      game_object_name(evaluation->world->database, player),
                      message));
          break;
        case '"':
          message++;
          [[fallthrough]];
        default:
          notify_with_cause(
              evaluation, target, player,
              tprintf("To (%s), %s%s pages you: %s", buf1,
                      game_object_name(evaluation->world->database, player),
                      aladd, message));
        }
        page_return(evaluation, configuration, player, target, "Idle", A_IDLE,
                    nullptr);

        safe_str(tprintf("%ld ", target), buf2, &bp2);
        count++;
      }
    }
  } else {
    if (ismessage)
      target = atoi(tname);
    if (target == NOTHING) {
      notify_printf(evaluation, player, "I don't recognize \"%s\".", tname);
    } else if (!page_check(evaluation, configuration, player, target)) {
      ;
    } else {

      switch (*message) {
      case ':':
        notify_with_cause(
            evaluation, target, player,
            tprintf("From afar,%s %s %s", aladd,
                    game_object_name(evaluation->world->database, player),
                    message + 1));
        break;
      case ';':
        message++;
        notify_with_cause(
            evaluation, target, player,
            tprintf("From afar,%s %s%s", aladd,
                    game_object_name(evaluation->world->database, player),
                    message));
        break;
      case '"':
        message++;
        [[fallthrough]];
      default:
        notify_with_cause(
            evaluation, target, player,
            tprintf("%s%s pages: %s",
                    game_object_name(evaluation->world->database, player),
                    aladd, message));
      }
      page_return(evaluation, configuration, player, target, "Idle", A_IDLE,
                  nullptr);

      safe_str(tprintf("%ld ", target), buf2, &bp2);
      safe_str(tprintf("%s, ",
                       game_object_name(evaluation->world->database, target)),
               buf1, &bp);

      /* this is terminating the string above when there is no more to add to
       * the list removing the ", "
       */
      *(bp - 2) = '\0';
      count++;
    }
  }

  if (count == 0) {
    free_lbuf(buf1);
    free_lbuf(buf2);
    return;
  }
  *(bp2 - 1) = '\0';
  attribute_add(evaluation->world->database, player, A_LASTPAGE, buf2,
                game_object_owner(evaluation->world->database, player), aflags);

  if (count == 1) {
    if (*buf1) {
      if (ispose != 1) {
        notify_printf(evaluation, player, "You paged %s with '%s'.", buf1, mp);
      } else {
        if (mp[0] == ':')
          notify_printf(evaluation, player, "Long distance to %s: %s %s", buf1,
                        game_object_name(evaluation->world->database, player),
                        mp + 1);
        else
          notify_printf(evaluation, player, "Long distance to %s: %s%s", buf1,
                        game_object_name(evaluation->world->database, player),
                        mp + 1);
      }
    }
  } else {
    *(bp - 2) = ')';
    *(bp - 1) = '\0';

    if (*buf1) {
      if (ispose != 1) {
        notify_printf(evaluation, player, "You paged (%s with '%s'.", buf1, mp);
      } else {
        if (mp[0] == ':')
          notify_printf(evaluation, player, "Long distance to (%s: %s %s", buf1,
                        game_object_name(evaluation->world->database, player),
                        mp + 1);
        else
          notify_printf(evaluation, player, "Long distance to (%s: %s%s", buf1,
                        game_object_name(evaluation->world->database, player),
                        mp + 1);
      }
    }
  }

  free_lbuf(buf1);
  free_lbuf(buf2);
}

void do_pemit_list(EvaluationContext *evaluation,
                   const ServerConfiguration *configuration, DbRef player,
                   char *list, const char *message) {
  /*
   * Send a message to a list of dbrefs. To avoid repeated generation *
   * of the NOSPOOF string, we set it up the first time we
   * encounter something Nospoof, and then check for it
   * thereafter. The list is destructively modified.
   */

  char *p;
  DbRef who;
  int ok_to_do;

  if (!message || !*message || !list || !*list)
    return;

  for (p = (char *)strtok(list, " "); p != nullptr;
       p = (char *)strtok(nullptr, " ")) {

    ok_to_do = 0;
    init_match(&evaluation->command->match, player, p, TYPE_PLAYER);
    match_everything(&evaluation->command->match, 0);
    who = match_result(&evaluation->command->match);

    if (!ok_to_do && (is_long_fingers(evaluation->world->database, player) ||
                      nearby(evaluation->world->database, player, who) ||
                      is_controls(evaluation, player, who))) {
      ok_to_do = 1;
    }
    if (!ok_to_do && (is_player(evaluation->world->database, who)) &&
        configuration->pemit_players) {
      if (!page_check(evaluation, configuration, player, who))
        return;
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
  const ServerConfiguration *configuration =
      invocation->context->world->configuration;
  const DbRef player = invocation->player;
  int key = invocation->key;
  char *recipient = invocation->first;
  char *message = invocation->second;
  DbRef target, loc;
  int do_contents, ok_to_do, depth, pemit_flags;

  if (key & PEMIT_LIST) {
    do_pemit_list(evaluation, configuration, player, recipient, message);
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
    init_match(&evaluation->command->match, player, recipient, TYPE_PLAYER);
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

    if (!ok_to_do && (nearby(evaluation->world->database, player, target) ||
                      is_long_fingers(evaluation->world->database, player) ||
                      is_controls(evaluation, player, target))) {
      ok_to_do = 1;
    }
    if (!ok_to_do && (key == PEMIT_PEMIT) &&
        (typeof_obj(evaluation->world->database, target) == TYPE_PLAYER) &&
        configuration->pemit_players) {
      if (!page_check(evaluation, configuration, player, target))
        return;
      ok_to_do = 1;
    }
    if (!ok_to_do && (!configuration->pemit_any || (key != PEMIT_PEMIT))) {
      notify(evaluation, player, "You are too far away to do that.");
      return;
    }
    if (do_contents && !is_controls(evaluation, player, target) &&
        !configuration->pemit_any) {
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
        if ((typeof_obj(evaluation->world->database, loc) == TYPE_ROOM) &&
            (pemit_flags & PEMIT_HERE)) {
          return;
        }
        depth = 0;
        while ((typeof_obj(evaluation->world->database, loc) != TYPE_ROOM) &&
               (depth++ < 20)) {
          loc = game_object_location(evaluation->world->database, loc);
          if ((loc == NOTHING) ||
              (loc == game_object_location(evaluation->world->database, loc)))
            return;
        }
        if (typeof_obj(evaluation->world->database, loc) == TYPE_ROOM) {
          notify_all_from_inside(evaluation, loc, player, message);
        }
      }
      break;
    default:
      break;
    }
  }
}
