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
#include "mux/network/mux_event_alloc.h"
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
  send_channel_v(evaluation, chan, format, arguments);
  va_end(arguments);
}

void send_channel_v(EvaluationContext *evaluation, const char *chan,
                    const char *format, va_list arguments) {
  struct channel *ch;
  char buf[LBUF_SIZE];
  char data[LBUF_SIZE];
  char *bp = buf;
  char *newline;

  if (!(ch = select_channel(evaluation->runtime->channels, chan)))
    return;
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  vsnprintf(data, LBUF_SIZE, format, arguments);

  safe_chr('[', buf, &bp);
  safe_str(chan, buf, &bp);
  safe_str("] ", buf, &bp);
  safe_str(data, buf, &bp);
  *bp = '\0';
  while ((newline = strchr(buf, '\n')))
    *newline = ' ';
  comsys_send_channel_message(evaluation, ch, buf);
}

char *comsys_channel_from_alias(EvaluationContext *evaluation, DbRef player,
                                char *alias) {
  struct commac *c;
  int first, last, current = 0;
  int dir;

  c = get_commac(evaluation->runtime->channels, player);

  first = 0;
  last = c->numchannels - 1;
  dir = 1;

  while (dir && (first <= last)) {
    current = (first + last) / 2;
    dir = strcasecmp(alias, commac_alias_at(c, (size_t)current));
    if (dir < 0)
      last = current - 1;
    else
      first = current + 1;
  }

  if (!dir)
    return commac_channel_at(c, (size_t)current);
  else {
    /* This function's other branch returns a genuinely mutable char *
       from c->channels[]; the return type can't be const. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    return (char *)"";
#pragma clang diagnostic pop
  }
}

typedef struct ComHistoryView ComHistoryView;
struct ComHistoryView {
  EvaluationContext *evaluation;
  DbRef player;
};

static void do_show_com(void *data, void *context) {
  chmsg *d = data;
  ComHistoryView *view = context;
  DbRef player = view->player;
  struct tm *t;
  int day;
  char buf[LBUF_SIZE];

  t = localtime(&view->evaluation->runtime->clock->now);
  day = t->tm_mday;
  t = localtime(&d->time);
  if (day == t->tm_mday) {
    snprintf(buf, sizeof(buf), "[%02d:%02d] %s", t->tm_hour, t->tm_min, d->msg);
  } else
    snprintf(buf, sizeof(buf), "[%02d.%02d / %02d:%02d] %s", t->tm_mon + 1,
             t->tm_mday, t->tm_hour, t->tm_min, d->msg);
  notify_checked(view->evaluation, player, player, buf,
                 MSG_ME_ALL | MSG_F_DOWN);
}

static void do_comlast(EvaluationContext *evaluation, DbRef player,
                       struct channel *ch) {
  if (!fifo_length(&ch->last_messages)) {
    notify_printf(evaluation, player, "There haven't been any messages on %s.",
                  ch->name);
    return;
  }
  ComHistoryView view = {.evaluation = evaluation, .player = player};
  fifo_traverse_reverse(&ch->last_messages, do_show_com, &view);
}

void comsys_process_alias_command(EvaluationContext *evaluation, DbRef player,
                                  char *arg1, char *arg2) {
  struct channel *ch;
  struct comuser *user;

  if ((strlen(arg1) + strlen(arg2)) > LBUF_SIZE / 2) {
    const size_t name_length = strlen(arg1);
    const size_t limit =
        name_length < LBUF_SIZE / 2 ? LBUF_SIZE / 2 - name_length : 0;

    *(char *)checked_storage_at(arg2, LBUF_SIZE, sizeof(char), limit) = '\0';
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

  if (!(ch = select_channel(evaluation->runtime->channels, arg1))) {
    notify_printf(evaluation, player, "Unknown channel %s.", arg1);
    return;
  }
  if (!(user = select_user(ch, player))) {
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
  } else if (!comsys_test_access(evaluation, player, CHANNEL_TRANSMIT, ch)) {
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
                                 struct channel *ch, char *mess) {
  struct comuser *user;
  chmsg *c;

  ch->num_messages++;
  for (user = ch->on_users; user; user = user->on_next) {
    if (user->on &&
        comsys_test_access(evaluation, user->who, CHANNEL_RECIEVE, ch) &&
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
  } else
    Create(c, chmsg, 1);
  c->msg = strdup(mess);
  c->time = evaluation->runtime->clock->now;
  fifo_push(&ch->last_messages, c);
}

void comsys_channel_printf(EvaluationContext *evaluation, struct channel *ch,
                           const char *messfmt, ...) {
  struct comuser *user;
  chmsg *c;
  va_list ap;
  char buffer[LBUF_SIZE];
  memset(buffer, 0, LBUF_SIZE);
  va_start(ap, messfmt);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  vsnprintf(buffer, LBUF_SIZE - 1, messfmt, ap);
  va_end(ap);

  ch->num_messages++;
  for (user = ch->on_users; user; user = user->on_next) {
    if (user->on &&
        comsys_test_access(evaluation, user->who, CHANNEL_RECIEVE, ch) &&
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
  } else
    Create(c, chmsg, 1);
  c->msg = strdup(buffer);
  c->time = evaluation->runtime->clock->now;
  fifo_push(&ch->last_messages, c);
}
