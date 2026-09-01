#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/* comsys.c - Player channel creation, membership, and message delivery. */

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>

#include "mux/communication/access_policy.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/commac.h"
#include "mux/communication/comsys.h"
#include "mux/communication/comsys_internal.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/fifo.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/player.h"

void init_chantab(ChannelRegistry *channels) {
  channel_registry_reset_statistics(channels);
}

void send_channel(EvaluationContext *evaluation, const char *chan,
                  const char *format, ...) {
  va_list arguments;

  va_start(arguments, format);
  send_channel_v(
      &(ChannelMessageTarget){.evaluation = evaluation, .channel = chan},
      format, arguments);
  va_end(arguments);
}

void send_channel_v(const ChannelMessageTarget *target, const char *format,
                    va_list arguments) {
  EvaluationContext *evaluation = target->evaluation;
  const char *chan = target->channel;
  struct Channel *ch;
  char *buf;
  char *data;
  char *bp;
  char *newline;

  ch = select_channel(evaluation->runtime->channels, chan);
  if (!ch)
    return;
  buf = alloc_lbuf("send_channel_v.message");
  data = alloc_lbuf("send_channel_v.data");
  bp = buf;
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(data, LBUF_SIZE, format, arguments);

  safe_chr('[', buf, &bp);
  safe_str(chan, buf, &bp);
  safe_str("] ", buf, &bp);
  safe_str(data, buf, &bp);
  *bp = '\0';
  while ((newline = strchr(buf, '\n')))
    *newline = ' ';
  comsys_send_channel_message(evaluation, ch, buf);
  free_buf(data);
  free_buf(buf);
}

typedef struct ComHistoryView ComHistoryView;
struct ComHistoryView {
  EvaluationContext *evaluation;
  DbRef player;
};

static void do_show_com(const FifoVisit *visit) {
  Chmsg *d = visit->item;
  ComHistoryView *view = visit->context;
  DbRef player = view->player;
  struct tm now_time;
  struct tm message_time;
  char buf[LBUF_SIZE];

  const bool NOW_CONVERTED =
      localtime_r(&view->evaluation->runtime->clock->now, &now_time) != nullptr;
  const bool MESSAGE_CONVERTED =
      localtime_r(&d->time, &message_time) != nullptr;
  if (!MESSAGE_CONVERTED) {
    (void)snprintf(buf, sizeof(buf), "[??.?? / ??:??] %s", d->msg);
  } else if (NOW_CONVERTED && now_time.tm_mday == message_time.tm_mday) {
    (void)snprintf(buf, sizeof(buf), "[%02d:%02d] %s", message_time.tm_hour,
                   message_time.tm_min, d->msg);
  } else {
    (void)snprintf(buf, sizeof(buf), "[%02d.%02d / %02d:%02d] %s",
                   message_time.tm_mon + 1, message_time.tm_mday,
                   message_time.tm_hour, message_time.tm_min, d->msg);
  }
  notify_checked(view->evaluation, player, player, buf,
                 MSG_ME_ALL | MSG_F_DOWN);
}

static void do_comlast(EvaluationContext *evaluation, DbRef player,
                       struct Channel *ch) {
  if (!fifo_length(&ch->last_messages)) {
    notify_printf(evaluation, player, "There haven't been any messages on %s.",
                  ch->name);
    return;
  }
  ComHistoryView view = {.evaluation = evaluation, .player = player};
  fifo_traverse_reverse(&ch->last_messages, do_show_com, &view);
}

void comsys_process_alias_command(EvaluationContext *evaluation, DbRef player,
                                  const char *arg1, char *arg2) {
  struct Channel *ch;
  struct Comuser *user;

  if ((strlen(arg1) + strlen(arg2)) > LBUF_SIZE / 2) {
    const size_t NAME_LENGTH = strlen(arg1);
    const size_t LIMIT =
        NAME_LENGTH < LBUF_SIZE / 2 ? (LBUF_SIZE / 2) - NAME_LENGTH : 0;

    *(char *)checked_storage_at(arg2, LBUF_SIZE, sizeof(char), LIMIT) = '\0';
  }
  if (!*arg2) {
    raw_notify(evaluation, player, "No message.");
    return;
  }

  if (!is_wizard(evaluation->world->database, player) &&
      is_in_character_location(evaluation->world->database,
                               evaluation->world->configuration, player)) {
    raw_notify(evaluation, player, "Permission denied.");
    return;
  }

  ch = select_channel(evaluation->runtime->channels, arg1);
  if (!ch) {
    notify_printf(evaluation, player, "Unknown channel %s.", arg1);
    return;
  }
  user = select_user(ch, player);
  if (!user) {
    raw_notify(evaluation, player,
               "You are not listed as on that channel.  Delete this "
               "alias and re-add.");
    return;
  }
  if (!strcasecmp(arg2, "on")) {
    do_joinchannel(evaluation, player, ch);
  } else if (!strcasecmp(arg2, "off")) {
    comsys_leave_channel(evaluation, player, ch);
    // Body matches the later bare `!user->on` branch, but this one fires
    // earlier so wizards/lurkers can still reach "who"/"last" below even
    // while not on the channel.
  } else if (!user->on && !is_wizard(evaluation->world->database, player) &&
             !evaluation->world->configuration
                  ->allow_chanlurking) { // NOLINT(bugprone-branch-clone)
    notify_printf(evaluation, player, "You must be on %s to do that.", arg1);
    return;
  } else if (!strcasecmp(arg2, "who")) {
    comsys_show_channel_who(evaluation, player, ch);
  } else if (!strcasecmp(arg2, "last")) {
    do_comlast(evaluation, player, ch);
  } else if (!user->on) {
    notify_printf(evaluation, player, "You must be on %s to do that.", arg1);
    return;
  } else if (!comsys_test_access(
                 &(ChannelAccessRequest){.evaluation = evaluation,
                                         .player = player,
                                         .access = CHANNEL_TRANSMIT,
                                         .channel = ch})) {
    raw_notify(evaluation, player,
               "That channel type cannot be transmitted on.");
    return;
  } else {
    char plain_message[LBUF_SIZE];
    const char *message =
        (*arg2 == ':' || *arg2 == ';') ? checked_string_suffix(arg2, 1) : arg2;

    styled_text_strip(evaluation->world->styled_text_palette, message,
                      plain_message, sizeof(plain_message));

    if ((*arg2) == ':')
      comsys_channel_printf(
          evaluation, ch, "[%s] %s %s", arg1,
          game_object_name(evaluation->world->database, player), plain_message);
    else if ((*arg2) == ';')
      comsys_channel_printf(
          evaluation, ch, "[%s] %s%s", arg1,
          game_object_name(evaluation->world->database, player), plain_message);
    else
      comsys_channel_printf(
          evaluation, ch, "[%s] %s: %s", arg1,
          game_object_name(evaluation->world->database, player), plain_message);
  }
}

void comsys_send_channel_message(EvaluationContext *evaluation,
                                 struct Channel *ch, const char *mess) {
  struct Comuser *user;
  Chmsg *c;

  ch->num_messages++;
  for (user = ch->on_users; user; user = user->on_next) {
    if (user->on &&
        comsys_test_access(&(ChannelAccessRequest){.evaluation = evaluation,
                                                   .player = user->who,
                                                   .access = CHANNEL_RECIEVE,
                                                   .channel = ch}) &&
        (is_wizard(evaluation->world->database, user->who) ||
         !is_in_character_location(evaluation->world->database,
                                   evaluation->world->configuration,
                                   user->who))) {
      if (typeof_obj(evaluation->world->database, user->who) ==
              OBJECT_TYPE_PLAYER &&
          is_connected(evaluation->world->database, user->who))
        raw_notify(evaluation, user->who, mess);
      else
        notify_checked(evaluation, user->who, user->who, mess,
                       MSG_ME_ALL | MSG_F_DOWN);
    }
  }
  /* Also, add it to the history of channel */
  if (fifo_length(&ch->last_messages) >= CHANNEL_HISTORY_LEN) {
    c = fifo_pop(&ch->last_messages);
    free((void *)c->msg);
  } else {
    c = checked_storage_allocate(sizeof(*c));
  }
  c->msg = strdup(mess);
  c->time = evaluation->runtime->clock->now;
  fifo_push(&ch->last_messages, c);
}

void comsys_channel_printf(EvaluationContext *evaluation, struct Channel *ch,
                           const char *messfmt, ...) {
  struct Comuser *user;
  Chmsg *c;
  va_list ap;
  char buffer[LBUF_SIZE];
  memset(buffer, 0, LBUF_SIZE);
  va_start(ap, messfmt);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(buffer, LBUF_SIZE - 1, messfmt, ap);
  va_end(ap);

  ch->num_messages++;
  for (user = ch->on_users; user; user = user->on_next) {
    if (user->on &&
        comsys_test_access(&(ChannelAccessRequest){.evaluation = evaluation,
                                                   .player = user->who,
                                                   .access = CHANNEL_RECIEVE,
                                                   .channel = ch}) &&
        (is_wizard(evaluation->world->database, user->who) ||
         !is_in_character_location(evaluation->world->database,
                                   evaluation->world->configuration,
                                   user->who))) {
      if (typeof_obj(evaluation->world->database, user->who) ==
              OBJECT_TYPE_PLAYER &&
          is_connected(evaluation->world->database, user->who))
        raw_notify(evaluation, user->who, buffer);
      else
        notify_checked(evaluation, user->who, user->who, buffer,
                       MSG_ME_ALL | MSG_F_DOWN);
    }
  }
  /* Also, add it to the history of channel */
  if (fifo_length(&ch->last_messages) >= CHANNEL_HISTORY_LEN) {
    c = fifo_pop(&ch->last_messages);
    free((void *)c->msg);
  } else {
    c = checked_storage_allocate(sizeof(*c));
  }
  c->msg = strdup(buffer);
  c->time = evaluation->runtime->clock->now;
  fifo_push(&ch->last_messages, c);
}
