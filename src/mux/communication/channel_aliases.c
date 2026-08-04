/* comsys.c - Player channel creation, membership, and message delivery. */

#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/commands/action_messages.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_helpers.h"
#include "mux/communication/access_policy.h"
#include "mux/communication/commac.h"
#include "mux/communication/comsys.h"
#include "mux/communication/comsys_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/connection_commands.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/utf8.h"
#include "mux/world/player.h"

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
    comsys_channel_printf(
        evaluation, ch, "[%s] %s has joined this channel.", ch->name,
        game_object_name(evaluation->world->database, player));
  }
}

void comsys_leave_channel(EvaluationContext *evaluation, DbRef player,
                          struct channel *ch) {
  struct comuser *user;
  int i;

  user = select_user(ch, player);

  if (!user)
    return;

  /* Trigger ALEAVE of any channel objects on the channel */
  for (i = ch->num_users - 1; i > 0; i--) {
    if (typeof_obj(evaluation->world->database, ch->users[i]->who) ==
        OBJECT_TYPE_THING)
      notify_event(evaluation, nullptr, player, player, ch->users[i]->who,
                   LUA_EVENT_LEAVE, (char **)nullptr, 0);
  }

  notify_printf(evaluation, player, "You have left channel %s.", ch->name);

  if ((user->on) && (!is_dark(evaluation->world->database, player))) {
    char *c = game_object_name(evaluation->world->database, player);

    if (c && *c) {
      comsys_channel_printf(evaluation, ch, "[%s] %s has left this channel.",
                            ch->name, c);
    }
  }
  user->on = 0;
}

void comsys_show_channel_who(EvaluationContext *evaluation, DbRef player,
                             struct channel *ch) {
  struct comuser *user;
  char *buff;

  raw_notify(evaluation, player, "-- Players --");
  for (user = ch->on_users; user; user = user->on_next) {
    if (typeof_obj(evaluation->world->database, user->who) ==
            OBJECT_TYPE_PLAYER &&
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
                            user->who);
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
    if (typeof_obj(evaluation->world->database, user->who) !=
            OBJECT_TYPE_PLAYER &&
        user->on && user->on &&
        !is_going(evaluation->world->database, user->who)) {
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            user->who);
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

  if (!*arg1) {
    raw_notify(evaluation, player, "You need to specify an alias.");
    return;
  }
  if (strlen(arg1) > 5 || !utf8_is_printable_ascii(arg1, strlen(arg1)) ||
      strchr(arg1, ' ')) {
    raw_notify(evaluation, player,
               "Channel aliases must be 1-5 printable ASCII characters "
               "without spaces.");
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
  if (!comsys_test_access(evaluation, player, CHANNEL_JOIN, ch)) {
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

  if (!arg1) {
    raw_notify(evaluation, player, "Need an alias to delete.");
    return;
  }
  c = get_commac(evaluation->runtime->channels, player);

  for (i = 0; i < c->numchannels; i++) {
    if (!strcasecmp(arg1, c->alias + i * 6)) {
      comsys_delete_channel_alias(evaluation, player, c->channels[i]);
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

void comsys_delete_channel_alias(EvaluationContext *evaluation, DbRef player,
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
          OBJECT_TYPE_THING)
        notify_event(evaluation, nullptr, player, player, ch->users[i]->who,
                     LUA_EVENT_LEAVE, (char **)nullptr, 0);
    }

    for (i = 0; i < ch->num_users; i++) {
      user = ch->users[i];
      if (user->who == player) {
        comsys_disconnect_channel(evaluation, player, channel);
        if (user->on && !is_dark(evaluation->world->database, player)) {
          char *c = game_object_name(evaluation->world->database, player);

          if (c && *c)
            comsys_channel_printf(evaluation, ch,
                                  "[%s] %s has left this channel.", channel, c);
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
