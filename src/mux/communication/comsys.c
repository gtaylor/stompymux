/* comsys.c - Player channel creation, membership, and message delivery. */

#include <ctype.h>
#include <sys/types.h>

#include "mux/commands/command_runtime.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/world/match.h"
#include "mux/world/player.h"
#include "mux/world/world_context.h"

#include "mux/commands/command_invocation.h"
#include "mux/commands/functions.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/comsys.h"
#include "mux/network/mux_event_alloc.h"

/* Static functions */
static void do_show_com(void *, void *);
static void do_comlast(EvaluationContext *, DbRef, struct channel *);
static void do_comsend(EvaluationContext *, struct channel *, char *);
static void do_comprintf(EvaluationContext *, struct channel *, const char *,
                         ...);
static void do_leavechannel(EvaluationContext *, DbRef, struct channel *);
static void do_comwho(EvaluationContext *, DbRef, struct channel *);
static void comlist_description(GameDatabase *, struct channel *, char *,
                                size_t);
static int do_test_access(EvaluationContext *, DbRef, long, struct channel *);
static char *get_channel_from_alias(EvaluationContext *, DbRef, char *);
static void do_processcom(EvaluationContext *, DbRef, char *, char *);
static void do_delcomchannel(EvaluationContext *, DbRef, char *);
static void do_listchannels(EvaluationContext *, DbRef);
static void do_comdisconnectraw_notify(EvaluationContext *, DbRef, char *);
static void do_comconnectraw_notify(EvaluationContext *, DbRef, char *);
static void do_comconnectchannel(EvaluationContext *, DbRef, char *, char *,
                                 int);
static void do_comdisconnectchannel(EvaluationContext *, DbRef, char *);
static void do_chclose(EvaluationContext *, DbRef, char *);
static void do_chloud(EvaluationContext *, DbRef, char *);
static void do_chsquelch(EvaluationContext *, DbRef, char *);
static void do_chtransparent(EvaluationContext *, DbRef, char *);
static void do_chopaque(EvaluationContext *, DbRef, char *);
static void do_chanobj(EvaluationContext *, DbRef, char *, char *);

/*
 * This is the hash table for channel names
 */

void init_chantab(ChannelRegistry *channels) {
  channel_registry_reset_statistics(channels);
}

void send_channel(EvaluationContext *evaluation, const char *chan,
                  const char *format, ...) {
  struct channel *ch;
  char buf[LBUF_SIZE];
  char data[LBUF_SIZE];
  char *bp = buf;
  char *newline;
  va_list ap;

  if (!(ch = select_channel(evaluation->runtime->channels, chan)))
    return;
  va_start(ap, format);
  vsnprintf(data, LBUF_SIZE, format, ap);
  va_end(ap);

  safe_chr('[', buf, &bp);
  safe_str(chan, buf, &bp);
  safe_str("] ", buf, &bp);
  safe_str(data, buf, &bp);
  *bp = '\0';
  while ((newline = strchr(buf, '\n')))
    *newline = ' ';
  do_comsend(evaluation, ch, buf);
}

static char *get_channel_from_alias(EvaluationContext *evaluation, DbRef player,
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
    dir = strcasecmp(alias, c->alias + 6 * current);
    if (dir < 0)
      last = current - 1;
    else
      first = current + 1;
  }

  if (!dir)
    return c->channels[current];
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
  notify(view->evaluation, player, buf);
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

static void do_processcom(EvaluationContext *evaluation, DbRef player,
                          char *arg1, char *arg2) {
  struct channel *ch;
  struct comuser *user;

  if ((strlen(arg1) + strlen(arg2)) > LBUF_SIZE / 2) {
    arg2[LBUF_SIZE / 2 - strlen(arg1)] = '\0';
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
    do_leavechannel(evaluation, player, ch);
    // Body matches the later bare `!user->on` branch, but this one fires
    // earlier so wizards/lurkers can still reach "who"/"last" below even
    // while not on the channel.
  } else if (!user->on && !is_wizard(evaluation->world->database, player) &&
             !evaluation->world->configuration
                  ->allow_chanlurking) { // NOLINT(bugprone-branch-clone)
    notify_printf(evaluation, player, "You must be on %s to do that.", arg1);
    return;
  } else if (!strcasecmp(arg2, "who")) {
    do_comwho(evaluation, player, ch);
  } else if (!strcasecmp(arg2, "last")) {
    do_comlast(evaluation, player, ch);
  } else if (!user->on) {
    notify_printf(evaluation, player, "You must be on %s to do that.", arg1);
    return;
  } else if (!do_test_access(evaluation, player, CHANNEL_TRANSMIT, ch)) {
    raw_notify(evaluation, player,
               "That channel type cannot be transmitted on.");
    return;
  } else {
    if ((*arg2) == ':')
      do_comprintf(evaluation, ch, "[%s] %s %s", arg1,
                   game_object_name(evaluation->world->database, player),
                   arg2 + 1);
    else if ((*arg2) == ';')
      do_comprintf(evaluation, ch, "[%s] %s%s", arg1,
                   game_object_name(evaluation->world->database, player),
                   arg2 + 1);
    else
      do_comprintf(evaluation, ch, "[%s] %s: %s", arg1,
                   game_object_name(evaluation->world->database, player), arg2);
  }
}

static void do_comsend(EvaluationContext *evaluation, struct channel *ch,
                       char *mess) {
  struct comuser *user;
  chmsg *c;

  ch->num_messages++;
  for (user = ch->on_users; user; user = user->on_next) {
    if (user->on &&
        do_test_access(evaluation, user->who, CHANNEL_RECIEVE, ch) &&
        (is_wizard(evaluation->world->database, user->who) ||
         !is_in_character_location(evaluation->world->database,
                                   evaluation->world->configuration,
                                   user->who))) {
      if (typeof_obj(evaluation->world->database, user->who) == TYPE_PLAYER &&
          is_connected(evaluation->world->database, user->who))
        raw_notify(evaluation, user->who, mess);
      else
        notify(evaluation, user->who, mess);
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

static void do_comprintf(EvaluationContext *evaluation, struct channel *ch,
                         const char *messfmt, ...)
    __attribute__((format(printf, 3, 4)));

static void do_comprintf(EvaluationContext *evaluation, struct channel *ch,
                         const char *messfmt, ...) {
  struct comuser *user;
  chmsg *c;
  va_list ap;
  char buffer[LBUF_SIZE];
  memset(buffer, 0, LBUF_SIZE);
  va_start(ap, messfmt);
  vsnprintf(buffer, LBUF_SIZE - 1, messfmt, ap);
  va_end(ap);

  ch->num_messages++;
  for (user = ch->on_users; user; user = user->on_next) {
    if (user->on &&
        do_test_access(evaluation, user->who, CHANNEL_RECIEVE, ch) &&
        (is_wizard(evaluation->world->database, user->who) ||
         !is_in_character_location(evaluation->world->database,
                                   evaluation->world->configuration,
                                   user->who))) {
      if (typeof_obj(evaluation->world->database, user->who) == TYPE_PLAYER &&
          is_connected(evaluation->world->database, user->who))
        raw_notify(evaluation, user->who, buffer);
      else
        notify(evaluation, user->who, buffer);
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

void do_joinchannel(EvaluationContext *evaluation, DbRef player,
                    struct channel *ch) {
  struct comuser *user;
  int i;

  user = select_user(ch, player);

  if (!user) {
    ch->num_users++;
    if (ch->num_users >= ch->max_users) {
      ch->max_users += 10;
      ch->users =
          realloc(ch->users, sizeof(struct comuser *) * (size_t)ch->max_users);
      memset(ch->users + (ch->num_users - 1), 0,
             sizeof(struct comuser *) *
                 (size_t)(ch->max_users - ch->num_users));
    }
    user = (struct comuser *)malloc(sizeof(struct comuser));

    for (i = ch->num_users - 1; i > 0 && ch->users[i - 1]->who > player; i--)
      ch->users[i] = ch->users[i - 1];
    ch->users[i] = user;

    user->who = player;
    user->on = 1;

    if (is_undead(evaluation->world->database, player)) {
      user->on_next = ch->on_users;
      ch->on_users = user;
    }
  } else if (!user->on) {
    user->on = 1;
  } else {
    notify_printf(evaluation, player, "You are already on channel %s.",
                  ch->name);
    return;
  }
  notify_printf(evaluation, player, "You have joined channel %s.", ch->name);

  if (!is_dark(evaluation->world->database, player)) {
    do_comprintf(evaluation, ch, "[%s] %s has joined this channel.", ch->name,
                 game_object_name(evaluation->world->database, player));
  }
}

static void do_leavechannel(EvaluationContext *evaluation, DbRef player,
                            struct channel *ch) {
  struct comuser *user;
  int i;

  user = select_user(ch, player);

  if (!user)
    return;

  /* Trigger ALEAVE of any channel objects on the channel */
  for (i = ch->num_users - 1; i > 0; i--) {
    if (typeof_obj(evaluation->world->database, ch->users[i]->who) ==
        TYPE_THING)
      did_it(evaluation, player, ch->users[i]->who, 0, nullptr, 0, nullptr,
             A_ALEAVE, (char **)nullptr, 0);
  }

  notify_printf(evaluation, player, "You have left channel %s.", ch->name);

  if ((user->on) && (!is_dark(evaluation->world->database, player))) {
    char *c = game_object_name(evaluation->world->database, player);

    if (c && *c) {
      do_comprintf(evaluation, ch, "[%s] %s has left this channel.", ch->name,
                   c);
    }
  }
  user->on = 0;
}

static void do_comwho(EvaluationContext *evaluation, DbRef player,
                      struct channel *ch) {
  struct comuser *user;
  char *buff;

  raw_notify(evaluation, player, "-- Players --");
  for (user = ch->on_users; user; user = user->on_next) {
    if (typeof_obj(evaluation->world->database, user->who) == TYPE_PLAYER &&
        user->on && is_connected(evaluation->world->database, user->who) &&
        (!is_hidden(evaluation->world->database, user->who) ||
         ((ch->type & CHANNEL_TRANSPARENT) &&
          !is_dark(evaluation->world->database, user->who)) ||
         is_wizard(evaluation->world->database, player)) &&
        (!is_in_character_location(evaluation->world->database,
                                   evaluation->world->configuration,
                                   user->who) ||
         is_wizard(evaluation->world->database, user->who))) {

      int i = fetch_idle(evaluation->runtime->descriptors,
                         evaluation->runtime->clock, user->who);

      buff = unparse_object(evaluation->world->database, evaluation, player,
                            user->who, 0);
      if (i > 30) {
        char *c = get_uptime_to_string(i);

        notify_printf(evaluation, player, "%s [idle %s]", buff, c);
        free_sbuf(c);
      } else
        notify_printf(evaluation, player, "%s", buff);
      free_lbuf(buff);
    }
  }

  raw_notify(evaluation, player, "-- Objects --");
  for (user = ch->on_users; user; user = user->on_next) {
    if (typeof_obj(evaluation->world->database, user->who) != TYPE_PLAYER &&
        user->on &&
        !(is_going(evaluation->world->database, user->who) &&
          is_god(evaluation->world->database,
                 game_object_owner(evaluation->world->database, user->who)))) {
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            user->who, 0);
      notify_printf(evaluation, player, "%s", buff);
      free_lbuf(buff);
    }
  }
  notify_printf(evaluation, player, "-- %s --", ch->name);
}

struct channel *select_channel(ChannelRegistry *channels, const char *channel) {
  return (struct channel *)hash_table_find(channel, &channels->channels);
}

struct comuser *select_user(struct channel *ch, DbRef player) {
  int last, current;
  int dir = 1, first = 0;

  if (!ch)
    return nullptr;

  last = ch->num_users - 1;
  current = (first + last) / 2;

  while (dir && (first <= last)) {
    current = (first + last) / 2;
    if (ch->users[current] == nullptr) {
      last--;
      continue;
    }
    if (ch->users[current]->who == player)
      dir = 0;
    else if (ch->users[current]->who < player) {
      dir = 1;
      first = current + 1;
    } else {
      dir = -1;
      last = current - 1;
    }
  }

  if (!dir)
    return ch->users[current];
  else
    return nullptr;
}

void comsys_add_alias(EvaluationContext *evaluation, DbRef player, char *arg1,
                      char *arg2) {
  char channel[200];
  struct channel *ch;
  int where;
  struct commac *c;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  if (!*arg1) {
    raw_notify(evaluation, player, "You need to specify an alias.");
    return;
  }
  if (!*arg2) {
    raw_notify(evaluation, player, "You need to specify a channel.");
    return;
  }

  if (strlen(arg2) >= sizeof(channel)) {
    raw_notify(evaluation, player, "Channel name too long.");
    return;
  }
  strlcpy(channel, arg2, sizeof(channel));

  if (strchr(channel, ' ')) {
    raw_notify(evaluation, player, "Channel name cannot contain spaces.");
    return;
  }

  if (!(ch = select_channel(evaluation->runtime->channels, channel))) {
    notify_printf(evaluation, player, "Channel %s does not exist yet.",
                  channel);
    return;
  }
  if (!do_test_access(evaluation, player, CHANNEL_JOIN, ch)) {
    raw_notify(evaluation, player,
               "Sorry, this channel type does not allow you to join.");
    return;
  }
  if (select_user(ch, player)) {
    raw_notify(evaluation, player,
               "Warning: you are already listed on that channel.");
  }
  c = get_commac(evaluation->runtime->channels, player);
  for (where = 0;
       where < c->numchannels && (strcasecmp(arg1, c->alias + where * 6) > 0);
       where++)
    ;
  if (where < c->numchannels && !strcasecmp(arg1, c->alias + where * 6)) {
    notify_printf(evaluation, player,
                  "That alias is already in use for channel %s.",
                  c->channels[where]);
    return;
  }
  if (c->numchannels >= c->maxchannels) {
    c->maxchannels += 10;
    c->alias = realloc(c->alias, sizeof(char) * 6 * (size_t)c->maxchannels);
    c->channels = realloc(c->channels, sizeof(char *) * (size_t)c->maxchannels);
  }
  if (where < c->numchannels) {
    memmove(c->alias + 6 * (where + 1), c->alias + 6 * where,
            (size_t)(6 * (c->numchannels - where)));
    memmove(c->channels + where + 1, c->channels + where,
            sizeof(c->channels) * (size_t)(c->numchannels - where));
  }

  c->numchannels++;

  strncpy(c->alias + 6 * where, arg1, 5);
  c->alias[where * 6 + 5] = '\0';
  c->channels[where] = strdup(ch->name);

  do_joinchannel(evaluation, player, ch);
  notify_printf(evaluation, player, "Channel %s added with alias %s.", ch->name,
                arg1);
}

void do_addcom(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  comsys_add_alias(evaluation, invocation->player, invocation->first,
                   invocation->second);
}

void do_delcom(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *arg1 = invocation->first;
  int i;
  struct commac *c;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  if (!arg1) {
    raw_notify(evaluation, player, "Need an alias to delete.");
    return;
  }
  c = get_commac(evaluation->runtime->channels, player);

  for (i = 0; i < c->numchannels; i++) {
    if (!strcasecmp(arg1, c->alias + i * 6)) {
      do_delcomchannel(evaluation, player, c->channels[i]);
      notify_printf(evaluation, player, "Channel %s deleted.", c->channels[i]);
      free(c->channels[i]);

      c->numchannels--;
      if (i < c->numchannels) {
        memmove(c->alias + 6 * i, c->alias + 6 * (i + 1),
                (size_t)(6 * (c->numchannels - i)));
        memmove(c->channels + i, c->channels + i + 1,
                sizeof(c->channels) * (size_t)(c->numchannels - i));
      }
      return;
    }
  }
  raw_notify(evaluation, player, "Unable to find that alias.");
}

static void do_delcomchannel(EvaluationContext *evaluation, DbRef player,
                             char *channel) {
  struct channel *ch;
  struct comuser *user;
  int i;

  if (!(ch = select_channel(evaluation->runtime->channels, channel))) {
    notify_printf(evaluation, player, "Unknown channel %s.", channel);
  } else {

    /* Trigger ALEAVE of any channel objects on the channel */
    for (i = ch->num_users - 1; i > 0; i--) {
      if (typeof_obj(evaluation->world->database, ch->users[i]->who) ==
          TYPE_THING)
        did_it(evaluation, player, ch->users[i]->who, 0, nullptr, 0, nullptr,
               A_ALEAVE, (char **)nullptr, 0);
    }

    for (i = 0; i < ch->num_users; i++) {
      user = ch->users[i];
      if (user->who == player) {
        do_comdisconnectchannel(evaluation, player, channel);
        if (user->on && !is_dark(evaluation->world->database, player)) {
          char *c = game_object_name(evaluation->world->database, player);

          if (c && *c)
            do_comprintf(evaluation, ch, "[%s] %s has left this channel.",
                         channel, c);
        }
        notify_printf(evaluation, player, "You have left channel %s.", channel);

        free(user);
        ch->num_users--;
        if (i < ch->num_users)
          memmove(ch->users + i, ch->users + i + 1,
                  sizeof(ch->users) * (size_t)(ch->num_users - i));
      }
    }
  }
}

void do_createchannel(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *channel = invocation->first;
  struct channel *newchannel;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  if (select_channel(evaluation->runtime->channels, channel)) {
    notify_printf(evaluation, player, "Channel %s already exists.", channel);
    return;
  }
  if (!*channel) {
    raw_notify(evaluation, player, "You must specify a channel to create.");
    return;
  }
  if (!(is_comm_all(evaluation->world->database, player))) {
    raw_notify(evaluation, player, "You do not have permission to do that.");
    return;
  }
  newchannel = (struct channel *)malloc(sizeof(struct channel));

  strncpy(newchannel->name, channel, CHAN_NAME_LEN - 1);
  newchannel->name[CHAN_NAME_LEN - 1] = '\0';
  newchannel->last_messages = nullptr;
  newchannel->type = 127;
  newchannel->temp1 = 0;
  newchannel->temp2 = 0;
  newchannel->charge = 0;
  newchannel->charge_who = (int)player;
  newchannel->amount_col = 0;
  newchannel->num_users = 0;
  newchannel->max_users = 0;
  newchannel->users = nullptr;
  newchannel->on_users = nullptr;
  newchannel->chan_obj = NOTHING;
  newchannel->num_messages = 0;

  evaluation->runtime->channels->count++;

  hash_table_add(newchannel->name, (int *)newchannel,
                 &evaluation->runtime->channels->channels);

  notify_printf(evaluation, player, "Channel %s created.", channel);
}

void do_destroychannel(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *channel = invocation->first;
  struct channel *ch;
  int j;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  ch = (struct channel *)hash_table_find(
      channel, &evaluation->runtime->channels->channels);

  if (!ch) {
    notify_printf(evaluation, player, "Could not find channel %s.", channel);
    return;
  } else if (!(is_comm_all(evaluation->world->database, player)) &&
             (player != ch->charge_who)) {
    raw_notify(evaluation, player, "You do not have permission to do that. ");
    return;
  }
  evaluation->runtime->channels->count--;
  hash_table_delete(channel, &evaluation->runtime->channels->channels);

  for (j = 0; j < ch->num_users; j++) {
    free(ch->users[j]);
  }
  free(ch->users);
  free(ch);
  notify_printf(evaluation, player, "Channel %s destroyed.", channel);
}

static void do_listchannels(EvaluationContext *evaluation, DbRef player) {
  struct channel *ch;
  int perm;

  if (!(perm = is_comm_all(evaluation->world->database, player))) {
    raw_notify(
        evaluation, player,
        "Warning: Only public channels and your channels will be shown.");
  }
  raw_notify(evaluation, player,
             "** Channel             --Flags--  Obj   Owner Users   Messages");

  for (ch = (struct channel *)hash_table_first_entry(
           &evaluation->runtime->channels->channels);
       ch; ch = (struct channel *)hash_table_next_entry(
               &evaluation->runtime->channels->channels))
    if (perm || (ch->type & CHANNEL_PUBLIC) || ch->charge_who == player) {

      notify_printf(
          evaluation, player, "%c%c %-20.20s %c%c%c/%c%c%c %5d %5d %6d %10d",
          (ch->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
          (ch->type & (CHANNEL_LOUD)) ? 'L' : '-', ch->name,
          (ch->type & (CHANNEL_PL_MULT * CHANNEL_JOIN)) ? 'J' : '-',
          (ch->type & (CHANNEL_PL_MULT * CHANNEL_TRANSMIT)) ? 'X' : '-',
          (ch->type & (CHANNEL_PL_MULT * CHANNEL_RECIEVE)) ? 'R' : '-',
          (ch->type & (CHANNEL_OBJ_MULT * CHANNEL_JOIN)) ? 'j' : '-',
          (ch->type & (CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT)) ? 'x' : '-',
          (ch->type & (CHANNEL_OBJ_MULT * CHANNEL_RECIEVE)) ? 'r' : '-',
          (ch->chan_obj != NOTHING) ? ch->chan_obj : -1, ch->charge_who,
          ch->num_users, ch->num_messages);
    }
  raw_notify(evaluation, player, "-- End of list of Channels --");
}

void do_comlist(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  struct channel *ch;
  struct comuser *user;
  struct commac *c;
  Descriptor *descriptor;
  char description[LBUF_SIZE];
  int description_width;
  int terminal_width;
  int i;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  c = get_commac(evaluation->runtime->channels, player);
  descriptor = evaluation->command->descriptor;
  terminal_width = 79;
  if (descriptor != nullptr && descriptor->terminal_width > terminal_width)
    terminal_width = descriptor->terminal_width;
  if (terminal_width > LBUF_SIZE)
    terminal_width = LBUF_SIZE;
  description_width = terminal_width - 37;

  raw_notify(evaluation, player,
             "Alias     Channel             Status Description");

  for (i = 0; i < c->numchannels; i++) {
    ch = select_channel(evaluation->runtime->channels, c->channels[i]);
    if ((user = select_user(ch, player))) {
      comlist_description(evaluation->world->database, ch, description,
                          (size_t)description_width + 1);
      notify_printf(evaluation, player, "%-9.9s %-19.19s %-6.6s %.*s",
                    c->alias + i * 6, c->channels[i], (user->on ? "on" : "off"),
                    description_width, description);
    } else {
      notify_printf(evaluation, player, "Bad Comsys Alias: %s for Channel: %s",
                    c->alias + i * 6, c->channels[i]);
    }
  }
  raw_notify(evaluation, player, "-- End of comlist --");
}

static void comlist_description(GameDatabase *database, struct channel *ch,
                                char *buffer, size_t buffer_size) {
  DbRef owner;
  long flags;
  char *description;
  char *source;
  char *destination;

  if (buffer_size == 0)
    return;
  if (ch->chan_obj == NOTHING) {
    strlcpy(buffer, "No description.", buffer_size);
    return;
  }

  description =
      attribute_parent_get(database, ch->chan_obj, A_DESC, &owner, &flags);
  if (!*description) {
    strlcpy(buffer, "No description.", buffer_size);
  } else {
    source = description;
    destination = buffer;
    while (*source && (size_t)(destination - buffer) < buffer_size - 1) {
      if (*source == '\r' || *source == '\n')
        *destination = ' ';
      else
        *destination = *source;
      destination++;
      source++;
    }
    *destination = '\0';
  }
  free_lbuf(description);
}

void do_channelnuke(EvaluationContext *evaluation, DbRef player) {
  struct channel *ch;
  int j;

  for (ch = (struct channel *)hash_table_first_entry(
           &evaluation->runtime->channels->channels);
       ch; ch = (struct channel *)hash_table_next_entry(
               &evaluation->runtime->channels->channels)) {
    if (ch->charge_who == player) {
      evaluation->runtime->channels->count--;
      hash_table_delete(ch->name, &evaluation->runtime->channels->channels);

      for (j = 0; j < ch->num_users; j++)
        free(ch->users[j]);
      free(ch->users);
      free(ch);
    }
  }
}

void comsys_clear_player(EvaluationContext *evaluation, DbRef player) {
  int i;
  struct commac *c;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  c = get_commac(evaluation->runtime->channels, player);

  for (i = (c->numchannels) - 1; i > -1; --i) {
    do_delcomchannel(evaluation, player, c->channels[i]);
    free(c->channels[i]);
    c->numchannels--;
  }
}

void do_clearcom(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  comsys_clear_player(evaluation, invocation->player);
}

void do_allcom(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *arg1 = invocation->first;
  int i;
  struct commac *c;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  c = get_commac(evaluation->runtime->channels, player);

  if ((strcasecmp(arg1, "who") != 0) && (strcasecmp(arg1, "on") != 0) &&
      (strcasecmp(arg1, "off") != 0)) {
    raw_notify(evaluation, player,
               "Only options available are: on, off and who.");
    return;
  }
  for (i = 0; i < c->numchannels; i++) {
    do_processcom(evaluation, player, c->channels[i], arg1);
    if (strcasecmp(arg1, "who") == 0)
      raw_notify(evaluation, player, "");
  }
}

void do_channelwho(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *arg1 = invocation->first;
  struct channel *ch;
  struct comuser *user;
  char channel[100];
  int flag = 0;
  char *cp;
  int i;
  char ansibuffer[LBUF_SIZE];

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  cp = strchr(arg1, '/');
  if (!cp) {
    strncpy(channel, arg1, 100);
    channel[99] = '\0';
  } else {
    /* channelname/all */
    if (cp - arg1 >= 100) {
      raw_notify(evaluation, player, "Channel name too long.");
      return;
    }
    strncpy(channel, arg1, (size_t)(cp - arg1));
    channel[cp - arg1] = '\0';
    if (*++cp == 'a')
      flag = 1;
  }

  if (!(ch = select_channel(evaluation->runtime->channels, channel))) {
    notify_printf(evaluation, player, "Unknown channel \"%s\".", channel);
    return;
  }
  if (!((is_comm_all(evaluation->world->database, player)) ||
        (player == ch->charge_who))) {
    raw_notify(evaluation, player, "You do not have permission to do that.");
    return;
  }
  notify_printf(evaluation, player, "-- %s --", ch->name);
  notify_printf(evaluation, player, "%-29.29s %-6.6s %-6.6s", "Name", "Status",
                "Player");
  for (i = 0; i < ch->num_users; i++) {
    user = ch->users[i];
    if ((flag || is_undead(evaluation->world->database, user->who)) &&
        (!is_hidden(evaluation->world->database, user->who) ||
         ((ch->type & CHANNEL_TRANSPARENT) &&
          !is_dark(evaluation->world->database, user->who)) ||
         is_wizard(evaluation->world->database, player))) {
      cp = unparse_object(evaluation->world->database, evaluation, player,
                          user->who, 0);
      strip_ansi_r(ansibuffer, cp, LBUF_SIZE);
      notify_printf(
          evaluation, player, "%-29.29s %-6.6s %-6.6s", ansibuffer,
          ((user->on) ? "on " : "off"),
          (typeof_obj(evaluation->world->database, user->who) == TYPE_PLAYER)
              ? "yes"
              : "no ");
      free_lbuf(cp);
    }
  }
  notify_printf(evaluation, player, "-- %s --", ch->name);
}

static void do_comdisconnectraw_notify(EvaluationContext *evaluation,
                                       DbRef player, char *chan) {
  struct channel *ch;
  struct comuser *cu;

  if (!(ch = select_channel(evaluation->runtime->channels, chan)))
    return;
  if (!(cu = select_user(ch, player)))
    return;

  if ((ch->type & CHANNEL_LOUD) && (cu->on) &&
      (!is_dark(evaluation->world->database, player))) {
    do_comprintf(evaluation, ch, "[%s] %s has disconnected.", ch->name,
                 game_object_name(evaluation->world->database, player));
  }
}

static void do_comconnectraw_notify(EvaluationContext *evaluation, DbRef player,
                                    char *chan) {
  struct channel *ch;
  struct comuser *cu;

  if (!(ch = select_channel(evaluation->runtime->channels, chan)))
    return;
  if (!(cu = select_user(ch, player)))
    return;

  if ((ch->type & CHANNEL_LOUD) && (cu->on) &&
      (!is_dark(evaluation->world->database, player))) {
    do_comprintf(evaluation, ch, "[%s] %s has connected.", ch->name,
                 game_object_name(evaluation->world->database, player));
  }
}

static void do_comconnectchannel(EvaluationContext *evaluation, DbRef player,
                                 char *channel, char *alias, int i) {
  struct channel *ch;
  struct comuser *user;

  if ((ch = select_channel(evaluation->runtime->channels, channel))) {
    for (user = ch->on_users; user && user->who != player; user = user->on_next)
      ;

    if (!user) {
      if ((user = select_user(ch, player))) {
        user->on_next = ch->on_users;
        ch->on_users = user;
      } else
        notify_printf(evaluation, player,
                      "Bad Comsys Alias: %s for Channel: %s", alias + i * 6,
                      channel);
    }
  } else
    notify_printf(evaluation, player, "Bad Comsys Alias: %s for Channel: %s",
                  alias + i * 6, channel);
}

void do_comdisconnect(EvaluationContext *evaluation, DbRef player) {
  int i;
  struct commac *c;

  c = get_commac(evaluation->runtime->channels, player);

  for (i = 0; i < c->numchannels; i++) {
    do_comdisconnectchannel(evaluation, player, c->channels[i]);
    do_comdisconnectraw_notify(evaluation, player, c->channels[i]);
  }
  send_channel(evaluation, "MUXConnections", "* %s has disconnected *",
               game_object_name(evaluation->world->database, player));
}

void do_comconnect(EvaluationContext *evaluation, DbRef player, Descriptor *d) {
  struct commac *c;
  int i;
  char *lsite;

  c = get_commac(evaluation->runtime->channels, player);

  for (i = 0; i < c->numchannels; i++) {
    do_comconnectchannel(evaluation, player, c->channels[i], c->alias, i);
    do_comconnectraw_notify(evaluation, player, c->channels[i]);
  }
  lsite = d->addr;
  if (lsite && *lsite)
    send_channel(evaluation, "MUXConnections", "* %s has connected from %s *",
                 game_object_name(evaluation->world->database, player), lsite);
  else
    send_channel(evaluation, "MUXConnections",
                 "* %s has connected from somewhere *",
                 game_object_name(evaluation->world->database, player));
}

static void do_comdisconnectchannel(EvaluationContext *evaluation, DbRef player,
                                    char *channel) {
  struct comuser *user, *prevuser = nullptr;
  struct channel *ch;

  if (!(ch = select_channel(evaluation->runtime->channels, channel)))
    return;
  for (user = ch->on_users; user;) {
    if (user->who == player) {
      if (prevuser)
        prevuser->on_next = user->on_next;
      else
        ch->on_users = user->on_next;
      return;
    } else {
      prevuser = user;
      user = user->on_next;
    }
  }
}

void do_editchannel(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int flag = invocation->key;
  char *arg1 = invocation->first;
  char *arg2 = invocation->second;
  char *s;
  struct channel *ch;
  int add_remove = 1;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  if (!(ch = select_channel(evaluation->runtime->channels, arg1))) {
    notify_printf(evaluation, player, "Unknown channel %s.", arg1);
    return;
  }
  if (!((is_comm_all(evaluation->world->database, player)) ||
        (player == ch->charge_who))) {
    raw_notify(evaluation, player, "Permission denied.");
    return;
  }
  s = arg2;
  if (*s == '!') {
    add_remove = 0;
    s++;
  }
  switch (flag) {
  case 0:
    if (lookup_player(evaluation->world, player, arg2, 1) != NOTHING) {
      ch->charge_who = (int)lookup_player(evaluation->world, player, arg2, 1);
      raw_notify(evaluation, player, "Set.");
      return;
    } else {
      raw_notify(evaluation, player, "Invalid player.");
      return;
    }
  case 3:
    if (strcasecmp(s, "join") == 0) {
      add_remove ? (ch->type |= (CHANNEL_PL_MULT * CHANNEL_JOIN))
                 : (ch->type &= ~(CHANNEL_PL_MULT * CHANNEL_JOIN));
      raw_notify(evaluation, player,
                 (add_remove) ? "@cpflags: Set." : "@cpflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "receive") == 0) {
      add_remove ? (ch->type |= (CHANNEL_PL_MULT * CHANNEL_RECIEVE))
                 : (ch->type &= ~(CHANNEL_PL_MULT * CHANNEL_RECIEVE));
      raw_notify(evaluation, player,
                 (add_remove) ? "@cpflags: Set." : "@cpflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "transmit") == 0) {
      add_remove ? (ch->type |= (CHANNEL_PL_MULT * CHANNEL_TRANSMIT))
                 : (ch->type &= ~(CHANNEL_PL_MULT * CHANNEL_TRANSMIT));
      raw_notify(evaluation, player,
                 (add_remove) ? "@cpflags: Set." : "@cpflags: Cleared.");
      return;
    }
    raw_notify(evaluation, player, "@cpflags: Unknown Flag.");
    break;
  case 4:
    if (strcasecmp(s, "join") == 0) {
      add_remove ? (ch->type |= (CHANNEL_OBJ_MULT * CHANNEL_JOIN))
                 : (ch->type &= ~(CHANNEL_OBJ_MULT * CHANNEL_JOIN));
      raw_notify(evaluation, player,
                 (add_remove) ? "@coflags: Set." : "@coflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "receive") == 0) {
      add_remove ? (ch->type |= (CHANNEL_OBJ_MULT * CHANNEL_RECIEVE))
                 : (ch->type &= ~(CHANNEL_OBJ_MULT * CHANNEL_RECIEVE));
      raw_notify(evaluation, player,
                 (add_remove) ? "@coflags: Set." : "@coflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "transmit") == 0) {
      add_remove ? (ch->type |= (CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT))
                 : (ch->type &= ~(CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT));
      raw_notify(evaluation, player,
                 (add_remove) ? "@coflags: Set." : "@coflags: Cleared.");
      return;
    }
    raw_notify(evaluation, player, "@coflags: Unknown Flag.");
    break;
  default:
    break;
  }
  return;
}

static int do_test_access(EvaluationContext *evaluation, DbRef player,
                          long access, struct channel *chan) {
  long flag_value = access;

  if (is_comm_all(evaluation->world->database, player))
    return (1);

  /*
   * Channel objects allow custom locks for channels.  The normal
   * lock is used to see if they can join that channel.  The
   * enterlock is checked to see if they can receive messages on
   * it. The Uselock is checked to see if they can transmit on
   * it. Note: These checks do not supercede the normal channel
   * flags. If a channel is set JOIN for players, ALL players can
   * join the channel, whether or not they pass the lock.  Same for
   * all channel object locks.
   */

  if ((flag_value & CHANNEL_JOIN) &&
      !((chan->chan_obj == NOTHING) || (chan->chan_obj == 0))) {
    if (could_doit_with_context(evaluation, player, chan->chan_obj, A_LOCK))
      return (1);
  }
  if ((flag_value & CHANNEL_TRANSMIT) &&
      !((chan->chan_obj == NOTHING) || (chan->chan_obj == 0))) {
    if (could_doit_with_context(evaluation, player, chan->chan_obj, A_LUSE))
      return (1);
  }
  if ((flag_value & CHANNEL_RECIEVE) &&
      !((chan->chan_obj == NOTHING) || (chan->chan_obj == 0))) {
    if (could_doit_with_context(evaluation, player, chan->chan_obj, A_LENTER))
      return (1);
  }
  if (typeof_obj(evaluation->world->database, player) == TYPE_PLAYER)
    flag_value *= CHANNEL_PL_MULT;
  else
    flag_value *= CHANNEL_OBJ_MULT;
  flag_value &= 0xFF; /*
                       * Mask out CHANNEL_PUBLIC and CHANNEL_LOUD
                       * just to be paranoid.
                       */

  return (int)(((long)chan->type & flag_value));
}

int do_comsystem(EvaluationContext *evaluation, DbRef who, char *cmd) {
  char *t;
  char *ch;
  char *alias;
  char *s;

  alias = alloc_lbuf("do_comsystem");
  s = alias;
  for (t = cmd; *t && *t != ' '; *s++ = *t++)
    /* nothing */;

  *s = '\0';

  if (*t)
    t++;

  ch = get_channel_from_alias(evaluation, who, alias);
  if (ch && *ch) {
    do_processcom(evaluation, who, ch, t);
    free_lbuf(alias);
    return 0;
  }
  free_lbuf(alias);
  return 1;
}

static void do_chclose(EvaluationContext *evaluation, DbRef player,
                       char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(evaluation->runtime->channels, chan))) {
    notify_printf(evaluation, player, "@cset: Channel %s does not exist.",
                  chan);
    return;
  }
  if ((player != ch->charge_who) &&
      (!is_comm_all(evaluation->world->database, player))) {
    raw_notify(evaluation, player, "@cset: Permission denied.");
    return;
  }
  ch->type &= (~(CHANNEL_PUBLIC));
  notify_printf(evaluation, player,
                "@cset: Channel %s taken off the public listings.", chan);
  return;
}

void do_cemit(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *chan = invocation->first;
  char *text = invocation->second;
  struct channel *ch;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  if (!(ch = select_channel(evaluation->runtime->channels, chan))) {
    notify_printf(evaluation, player, "Channel %s does not exist.", chan);
    return;
  }
  if ((player != ch->charge_who) &&
      (!is_comm_all(evaluation->world->database, player))) {
    raw_notify(evaluation, player, "Permission denied.");
    return;
  }
  if (key == CEMIT_NOHEADER)
    do_comsend(evaluation, ch, text);
  else
    do_comprintf(evaluation, ch, "[%s] %s", chan, text);
}

void do_chopen(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *chan = invocation->first;
  char *object = invocation->second;
  struct channel *ch;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  switch (key) {
  case CSET_PRIVATE:
    do_chclose(evaluation, player, chan);
    return;
  case CSET_LOUD:
    do_chloud(evaluation, player, chan);
    return;
  case CSET_QUIET:
    do_chsquelch(evaluation, player, chan);
    return;
  case CSET_LIST:
    CommandInvocation list_invocation = *invocation;
    list_invocation.key = 1;
    do_chanlist(&list_invocation);
    return;
  case CSET_OBJECT:
    do_chanobj(evaluation, player, chan, object);
    return;
  case CSET_STATUS:
    CommandInvocation status_invocation = *invocation;
    status_invocation.key = 1;
    status_invocation.first = chan;
    do_chanstatus(&status_invocation);
    return;
  case CSET_TRANSPARENT:
    do_chtransparent(evaluation, player, chan);
    return;
  case CSET_OPAQUE:
    do_chopaque(evaluation, player, chan);
    return;
  default:
    break;
  }

  if (!(ch = select_channel(evaluation->runtime->channels, chan))) {
    notify_printf(evaluation, player, "@cset: Channel %s does not exist.",
                  chan);
    return;
  }
  if ((player != ch->charge_who) &&
      (!is_comm_all(evaluation->world->database, player))) {
    raw_notify(evaluation, player, "@cset: Permission denied.");
    return;
  }
  ch->type |= (CHANNEL_PUBLIC);
  notify_printf(evaluation, player,
                "@cset: Channel %s placed on the public listings.", chan);
  return;
}

static void do_chloud(EvaluationContext *evaluation, DbRef player, char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(evaluation->runtime->channels, chan))) {
    notify_printf(evaluation, player, "@cset: Channel %s does not exist.",
                  chan);
    return;
  }
  if ((player != ch->charge_who) &&
      (!is_comm_all(evaluation->world->database, player))) {
    raw_notify(evaluation, player, "@cset: Permission denied.");
    return;
  }
  ch->type |= (CHANNEL_LOUD);
  notify_printf(evaluation, player,
                "@cset: Channel %s now sends connect/disconnect msgs.", chan);
  return;
}

static void do_chsquelch(EvaluationContext *evaluation, DbRef player,
                         char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(evaluation->runtime->channels, chan))) {
    notify_printf(evaluation, player, "@cset: Channel %s does not exist.",
                  chan);
    return;
  }
  if ((player != ch->charge_who) &&
      (!is_comm_all(evaluation->world->database, player))) {
    raw_notify(evaluation, player, "@cset: Permission denied.");
    return;
  }
  ch->type &= ~(CHANNEL_LOUD);
  notify_printf(evaluation, player,
                "@cset: Channel %s connect/disconnect msgs muted.", chan);
  return;
}

static void do_chtransparent(EvaluationContext *evaluation, DbRef player,
                             char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(evaluation->runtime->channels, chan))) {
    notify_printf(evaluation, player, "@cset: Channel %s does not exist.",
                  chan);
    return;
  }
  if ((player != ch->charge_who) &&
      (!is_comm_all(evaluation->world->database, player))) {
    raw_notify(evaluation, player, "@cset: Permission denied.");
    return;
  }
  ch->type |= CHANNEL_TRANSPARENT;
  notify_printf(evaluation, player,
                "@cset: Channel %s now shows all listeners to everyone.", chan);
  return;
}

static void do_chopaque(EvaluationContext *evaluation, DbRef player,
                        char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(evaluation->runtime->channels, chan))) {
    notify_printf(evaluation, player, "@cset: Channel %s does not exist.",
                  chan);
    return;
  }
  if ((player != ch->charge_who) &&
      (!is_comm_all(evaluation->world->database, player))) {
    raw_notify(evaluation, player, "@cset: Permission denied.");
    return;
  }
  ch->type &= ~CHANNEL_TRANSPARENT;
  notify_printf(
      evaluation, player,
      "@cset: Channel %s now does not show all listeners to everyone.", chan);
  return;
}

void do_chboot(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *channel = invocation->first;
  char *victim = invocation->second;
  struct comuser *user;
  struct channel *ch;
  struct comuser *vu;
  DbRef thing;

  /*
   * * I sure hope it's not going to be that *
   * *  * *  * *  * * long.
   */

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  if (!(ch = select_channel(evaluation->runtime->channels, channel))) {
    raw_notify(evaluation, player, "@cboot: Unknown channel.");
    return;
  }
  if (!(user = select_user(ch, player))) {
    raw_notify(evaluation, player, "@cboot: You are not on that channel.");
    return;
  }
  if (!((ch->charge_who == player) ||
        is_comm_all(evaluation->world->database, player))) {
    raw_notify(evaluation, player, "Permission denied.");
    return;
  }
  thing = match_thing(&evaluation->command->match, player, victim);

  if (thing == NOTHING) {
    return;
  }
  if (!(vu = select_user(ch, thing))) {
    notify_printf(evaluation, player, "@cboot: %s in not on the channel.",
                  game_object_name(evaluation->world->database, thing));
    return;
  }
  /*
   * We should be in the clear now. :)
   */
  do_comprintf(evaluation, ch, "[%s] %s boots %s off the channel.", ch->name,
               unparse_object_numonly(evaluation->world->database, player),
               unparse_object_numonly(evaluation->world->database, thing));
  do_delcomchannel(evaluation, thing, channel);
}

static void do_chanobj(EvaluationContext *evaluation, DbRef player,
                       char *channel, char *object) {
  struct channel *ch;
  DbRef thing;
  char *buff;

  init_match(&evaluation->command->match, player, object, NOTYPE);
  match_everything(&evaluation->command->match, 0);
  thing = match_result(&evaluation->command->match);

  if (!(ch = select_channel(evaluation->runtime->channels, channel))) {
    raw_notify(evaluation, player, "That channel does not exist.");
    return;
  }
  if (thing == NOTHING) {
    ch->chan_obj = NOTHING;
    raw_notify(evaluation, player, "Set.");
    return;
  }
  if (!(ch->charge_who == player) &&
      !is_comm_all(evaluation->world->database, player)) {
    raw_notify(evaluation, player, "Permission denied.");
    return;
  }
  ch->chan_obj = (int)thing;
  buff =
      unparse_object(evaluation->world->database, evaluation, player, thing, 0);
  notify_printf(evaluation, player,
                "Channel %s is now using %s as channel object.", ch->name,
                buff);
  free_lbuf(buff);
}

void do_chanlist(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  DbRef owner;
  struct channel *ch;
  long flags;
  char *temp;
  char *buf;
  char *atrstr;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }
  flags = 0;

  if (key & CLIST_FULL) {
    do_listchannels(evaluation, player);
    return;
  }
  temp = alloc_mbuf("do_chanlist_temp");
  buf = alloc_mbuf("do_chanlist_buf");

  raw_notify(evaluation, player,
             "** Channel       Owner           Description");

  for (ch = (struct channel *)hash_table_first_entry(
           &evaluation->runtime->channels->channels);
       ch; ch = (struct channel *)hash_table_next_entry(
               &evaluation->runtime->channels->channels)) {
    if (is_comm_all(evaluation->world->database, player) ||
        (ch->type & CHANNEL_PUBLIC) || ch->charge_who == player ||
        (do_test_access(evaluation, player, CHANNEL_JOIN, ch))) {

      atrstr = attribute_parent_get(evaluation->world->database, ch->chan_obj,
                                    A_DESC, &owner, &flags);
      if ((ch->chan_obj == NOTHING) || !*atrstr)
        snprintf(buf, MBUF_SIZE, "%s", "No description.");
      else
        snprintf(buf, MBUF_SIZE, "%-54.54s", atrstr);

      free_lbuf(atrstr);
      snprintf(temp, MBUF_SIZE, "%c%c %-13.13s %-15.15s %-45.45s",
               (ch->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
               (ch->type & (CHANNEL_LOUD)) ? 'L' : '-', ch->name,
               game_object_name(evaluation->world->database, ch->charge_who),
               buf);

      raw_notify(evaluation, player, temp);
    }
  }
  free_mbuf(temp);
  free_mbuf(buf);
  raw_notify(evaluation, player, "-- End of list of Channels --");
}

void do_chanstatus(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *chan = invocation->first;
  DbRef owner;
  struct channel *ch;
  long flags;
  char *temp;
  char *buf;
  char *atrstr;

  if (!evaluation->world->configuration->have_comsys) {
    raw_notify(evaluation, player, "Comsys disabled.");
    return;
  }

  if (key & CSTATUS_FULL) {
    struct channel *selected_channel;
    int perm;

    if (!(perm = is_comm_all(evaluation->world->database, player))) {
      raw_notify(
          evaluation, player,
          "Warning: Only public channels and your channels will be shown.");
    }
    raw_notify(evaluation, player,
               "** Channel             --Flags--  Obj   Own   Charge  "
               "Balance  Users   Messages");

    if (!(selected_channel =
              select_channel(evaluation->runtime->channels, chan))) {
      notify_printf(evaluation, player, "@cstatus: Channel %s does not exist.",
                    chan);
      return;
    }
    if (perm || (selected_channel->type & CHANNEL_PUBLIC) ||
        selected_channel->charge_who == player) {

      notify_printf(
          evaluation, player, "%c%c %-20.20s %c%c%c/%c%c%c %5d %5d %6d %10d",
          (selected_channel->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
          (selected_channel->type & (CHANNEL_LOUD)) ? 'L' : '-',
          selected_channel->name,
          (selected_channel->type & (CHANNEL_PL_MULT * CHANNEL_JOIN)) ? 'J'
                                                                      : '-',
          (selected_channel->type & (CHANNEL_PL_MULT * CHANNEL_TRANSMIT)) ? 'X'
                                                                          : '-',
          (selected_channel->type & (CHANNEL_PL_MULT * CHANNEL_RECIEVE)) ? 'R'
                                                                         : '-',
          (selected_channel->type & (CHANNEL_OBJ_MULT * CHANNEL_JOIN)) ? 'j'
                                                                       : '-',
          (selected_channel->type & (CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT))
              ? 'x'
              : '-',
          (selected_channel->type & (CHANNEL_OBJ_MULT * CHANNEL_RECIEVE)) ? 'r'
                                                                          : '-',
          (selected_channel->chan_obj != NOTHING) ? selected_channel->chan_obj
                                                  : -1,
          selected_channel->charge_who, selected_channel->num_users,
          selected_channel->num_messages);
    }
    raw_notify(evaluation, player, "-- End of list of Channels --");
    return;
  }
  temp = alloc_mbuf("do_chanstatus_temp");
  buf = alloc_mbuf("do_chanstatus_buf");

  raw_notify(evaluation, player,
             "** Channel       Owner           Description");
  if (!(ch = select_channel(evaluation->runtime->channels, chan))) {
    notify_printf(evaluation, player, "@cstatus: Channel %s does not exist.",
                  chan);
    return;
  }
  if (is_comm_all(evaluation->world->database, player) ||
      (ch->type & CHANNEL_PUBLIC) || ch->charge_who == player) {

    atrstr = attribute_parent_get(evaluation->world->database, ch->chan_obj,
                                  A_DESC, &owner, &flags);
    if ((ch->chan_obj == NOTHING) || !*atrstr)
      snprintf(buf, MBUF_SIZE, "%s", "No description.");
    else
      snprintf(buf, MBUF_SIZE, "%-54.54s", atrstr);

    free_lbuf(atrstr);
    snprintf(temp, MBUF_SIZE, "%c%c %-13.13s %-15.15s %-45.45s",
             (ch->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
             (ch->type & (CHANNEL_LOUD)) ? 'L' : '-', ch->name,
             game_object_name(evaluation->world->database, ch->charge_who),
             buf);

    raw_notify(evaluation, player, temp);
  }
  free_mbuf(temp);
  free_mbuf(buf);
  raw_notify(evaluation, player, "-- End of list of Channels --");
}

void fun_cemit(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context) {
  struct channel *ch;

  if (!(ch = select_channel(context->runtime->channels, fargs[0]))) {
    safe_str("#-1 CHANNEL NOT FOUND", buff, bufc);
    return;
  }

  if (!context->world->configuration->have_comsys ||
      (!is_comm_all(context->world->database, player) &&
       (player != ch->charge_who))) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }

  do_comprintf(context, ch, "[%s] %s", fargs[0], fargs[1]);
  *buff = '\0';
}
