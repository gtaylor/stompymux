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
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/owned_text.h"
#include "mux/support/utf8.h"
#include "mux/world/player.h"

struct Comuser *channel_user_at(const struct Channel *channel, size_t index) {
  return *(struct Comuser *const *)checked_storage_at_const(
      (const void *)channel->users, (size_t)channel->num_users,
      sizeof(*channel->users), index);
}

struct Comuser **channel_user_slot(struct Channel *channel, size_t index) {
  return (struct Comuser **)checked_storage_at((void *)channel->users,
                                               (size_t)channel->max_users,
                                               sizeof(*channel->users), index);
}

void do_joinchannel(EvaluationContext *evaluation, DbRef player,
                    struct Channel *ch, bool quiet) {
  struct Comuser *user;
  int i;

  user = select_user(ch, player);

  if (!user) {
    ch->num_users++;
    if (ch->num_users >= ch->max_users) {
      const int CAPACITY = ch->max_users + 10;
      struct Comuser **users =
          (struct Comuser **)checked_storage_try_reallocate_array(
              (void *)ch->users, (size_t)CAPACITY, sizeof(*ch->users));

      if (users == nullptr) {
        ch->num_users--;
        raw_notify(evaluation, player, "Unable to add you to that channel.");
        return;
      }
      ch->users = users;
      ch->max_users = CAPACITY;
      memset(checked_storage_at((void *)ch->users, (size_t)ch->max_users,
                                sizeof(*ch->users), (size_t)ch->num_users - 1),
             0,
             sizeof(struct Comuser *) *
                 (size_t)(ch->max_users - ch->num_users));
    }
    user =
        (struct Comuser *)checked_storage_try_allocate(sizeof(struct Comuser));

    for (i = ch->num_users - 1;
         i > 0 && channel_user_at(ch, (size_t)i - 1)->who > player; i--)
      *channel_user_slot(ch, (size_t)i) = channel_user_at(ch, (size_t)i - 1);
    *channel_user_slot(ch, (size_t)i) = user;

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

  if (!quiet && !is_dark(evaluation->world->database, player)) {
    comsys_channel_printf(
        evaluation, ch, "[%s] %s has joined this channel.", ch->name,
        game_object_name(evaluation->world->database, player));
  }
}

void comsys_leave_channel(EvaluationContext *evaluation, DbRef player,
                          struct Channel *ch) {
  struct Comuser *user;
  int i;

  user = select_user(ch, player);

  if (!user)
    return;

  /* Trigger ALEAVE of any channel objects on the channel */
  for (i = ch->num_users - 1; i > 0; i--) {
    struct Comuser *member = channel_user_at(ch, (size_t)i);

    if (typeof_obj(evaluation->world->database, member->who) ==
        OBJECT_TYPE_THING)
      notify_event(evaluation, nullptr, player, player, member->who,
                   LUA_EVENT_LEAVE, (char **)nullptr, 0);
  }

  notify_printf(evaluation, player, "You have left channel %s.", ch->name);

  if ((user->on) && (!is_dark(evaluation->world->database, player))) {
    const char *c = game_object_name(evaluation->world->database, player);

    if (c && *c) {
      comsys_channel_printf(evaluation, ch, "[%s] %s has left this channel.",
                            ch->name, c);
    }
  }
  user->on = 0;
}

void comsys_show_channel_who(EvaluationContext *evaluation, DbRef player,
                             struct Channel *ch) {
  struct Comuser *user;
  OwnedText buff;

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
        OwnedText c = get_uptime_to_string(i);

        notify_printf(evaluation, player, "%s [idle %s]", buff.text, c.text);
        owned_text_release(&c);
      } else {
        notify_printf(evaluation, player, "%s", buff.text);
      }
      owned_text_release(&buff);
    }
  }

  raw_notify(evaluation, player, "-- Objects --");
  for (user = ch->on_users; user; user = user->on_next) {
    if (typeof_obj(evaluation->world->database, user->who) !=
            OBJECT_TYPE_PLAYER &&
        user->on && !is_going(evaluation->world->database, user->who)) {
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            user->who);
      notify_printf(evaluation, player, "%s", buff.text);
      owned_text_release(&buff);
    }
  }
  notify_printf(evaluation, player, "-- %s --", ch->name);
}

struct Channel *select_channel(ChannelRegistry *channels, const char *channel) {
  return (struct Channel *)hash_table_find(channel, &channels->channels);
}

struct Comuser *select_user(struct Channel *ch, DbRef player) {
  int last;
  int current;
  int dir = 1;
  int first = 0;

  if (!ch)
    return nullptr;

  last = ch->num_users - 1;
  current = (first + last) / 2;

  while (dir && (first <= last)) {
    current = (first + last) / 2;
    struct Comuser *candidate = channel_user_at(ch, (size_t)current);

    if (candidate == nullptr) {
      last--;
      continue;
    }
    if (candidate->who == player) {
      dir = 0;
    } else if (candidate->who < player) {
      dir = 1;
      first = current + 1;
    } else {
      dir = -1;
      last = current - 1;
    }
  }

  if (!dir)
    return channel_user_at(ch, (size_t)current);
  return nullptr;
}

ChannelAddPlayerResult comsys_channel_add_player(EvaluationContext *evaluation,
                                                 DbRef player,
                                                 struct Channel *channel,
                                                 const char *alias,
                                                 bool quiet) {
  int where;
  struct Commac *commac;

  if (!*alias)
    return CHANNEL_ADD_PLAYER_ALIAS_REQUIRED;
  if (strlen(alias) > COMMAC_ALIAS_MAX_LENGTH ||
      !utf8_is_printable_ascii(alias, strlen(alias)) || strchr(alias, ' '))
    return CHANNEL_ADD_PLAYER_ALIAS_INVALID;

  commac = get_commac(evaluation->runtime->channels, player);
  for (where = 0; where < commac->numchannels &&
                  strcasecmp(alias, commac_alias_at(commac, (size_t)where)) > 0;
       where++)
    ;
  if (where < commac->numchannels &&
      !strcasecmp(alias, commac_alias_at(commac, (size_t)where)))
    return CHANNEL_ADD_PLAYER_ALIAS_IN_USE;
  if (commac->numchannels >= commac->maxchannels) {
    const size_t CAPACITY = (size_t)commac->maxchannels + 10;

    if (!commac_reserve_aliases(commac, CAPACITY))
      return CHANNEL_ADD_PLAYER_CAPACITY_FAILURE;
  }
  if (where < commac->numchannels) {
    memmove(commac_alias_at(commac, (size_t)where + 1),
            commac_alias_at(commac, (size_t)where),
            COMMAC_ALIAS_SIZE * (size_t)(commac->numchannels - where));
    memmove((void *)commac_channel_slot(commac, (size_t)where + 1),
            (const void *)commac_channel_slot(commac, (size_t)where),
            sizeof(*commac->channels) * (size_t)(commac->numchannels - where));
  }

  commac->numchannels++;
  (void)string_copy_bounded(commac_alias_at(commac, (size_t)where),
                            COMMAC_ALIAS_SIZE, alias);
  *commac_channel_slot(commac, (size_t)where) = strdup(channel->name);

  do_joinchannel(evaluation, player, channel, quiet);
  notify_printf(evaluation, player, "Channel %s added with alias %s.",
                channel->name, alias);
  return CHANNEL_ADD_PLAYER_OK;
}

void comsys_add_alias(EvaluationContext *evaluation, DbRef player,
                      const char *arg1, const char *arg2) {
  char channel[200];
  struct Channel *ch;
  struct Commac *c;

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
  (void)string_copy_bounded(channel, sizeof(channel), arg2);

  if (strchr(channel, ' ')) {
    raw_notify(evaluation, player, "Channel name cannot contain spaces.");
    return;
  }

  ch = select_channel(evaluation->runtime->channels, channel);
  if (!ch) {
    notify_printf(evaluation, player, "Channel %s does not exist yet.",
                  channel);
    return;
  }
  if (!comsys_test_access(&(ChannelAccessRequest){.evaluation = evaluation,
                                                  .player = player,
                                                  .access = CHANNEL_JOIN,
                                                  .channel = ch})) {
    raw_notify(evaluation, player,
               "Sorry, this channel type does not allow you to join.");
    return;
  }
  if (select_user(ch, player)) {
    raw_notify(evaluation, player,
               "Warning: you are already listed on that channel.");
  }
  c = get_commac(evaluation->runtime->channels, player);
  const char *existing_channel = commac_channel_for_alias(c, arg1);
  if (existing_channel != nullptr) {
    notify_printf(evaluation, player,
                  "That alias is already in use for channel %s.",
                  existing_channel);
    return;
  }
  ChannelAddPlayerResult result =
      comsys_channel_add_player(evaluation, player, ch, arg1, false);
  if (result == CHANNEL_ADD_PLAYER_CAPACITY_FAILURE)
    raw_notify(evaluation, player, "Unable to add that channel alias.");
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
  struct Commac *c;

  if (!arg1) {
    raw_notify(evaluation, player, "Need an alias to delete.");
    return;
  }
  c = get_commac(evaluation->runtime->channels, player);

  for (i = 0; i < c->numchannels; i++) {
    if (!strcasecmp(arg1, commac_alias_at(c, (size_t)i))) {
      char *channel = commac_channel_at(c, (size_t)i);

      comsys_delete_channel_alias(evaluation, player, channel);
      notify_printf(evaluation, player, "Channel %s deleted.", channel);
      free(channel);

      c->numchannels--;
      if (i < c->numchannels) {
        memmove(commac_alias_at(c, (size_t)i),
                commac_alias_at(c, (size_t)i + 1),
                COMMAC_ALIAS_SIZE * (size_t)(c->numchannels - i));
        memmove((void *)commac_channel_slot(c, (size_t)i),
                (const void *)commac_channel_slot(c, (size_t)i + 1),
                sizeof(*c->channels) * (size_t)(c->numchannels - i));
      }
      return;
    }
  }
  raw_notify(evaluation, player, "Unable to find that alias.");
}

void comsys_delete_channel_alias(EvaluationContext *evaluation, DbRef player,
                                 char *channel) {
  struct Channel *ch;
  struct Comuser *user;
  int i;

  ch = select_channel(evaluation->runtime->channels, channel);
  if (!ch) {
    notify_printf(evaluation, player, "Unknown channel %s.", channel);
  } else {

    /* Trigger ALEAVE of any channel objects on the channel */
    for (i = ch->num_users - 1; i > 0; i--) {
      struct Comuser *member = channel_user_at(ch, (size_t)i);

      if (typeof_obj(evaluation->world->database, member->who) ==
          OBJECT_TYPE_THING)
        notify_event(evaluation, nullptr, player, player, member->who,
                     LUA_EVENT_LEAVE, (char **)nullptr, 0);
    }

    for (i = 0; i < ch->num_users; i++) {
      user = channel_user_at(ch, (size_t)i);
      if (user->who == player) {
        comsys_disconnect_channel(evaluation, player, channel);
        if (user->on && !is_dark(evaluation->world->database, player)) {
          const char *c = game_object_name(evaluation->world->database, player);

          if (c && *c)
            comsys_channel_printf(evaluation, ch,
                                  "[%s] %s has left this channel.", channel, c);
        }
        notify_printf(evaluation, player, "You have left channel %s.", channel);

        free(user);
        ch->num_users--;
        if (i < ch->num_users)
          memmove((void *)channel_user_slot(ch, (size_t)i),
                  (const void *)channel_user_slot(ch, (size_t)i + 1),
                  sizeof(*ch->users) * (size_t)(ch->num_users - i));
      }
    }
  }
}
