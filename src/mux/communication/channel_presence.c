/* comsys.c - Player channel creation, membership, and message delivery. */

#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
#include "mux/communication/commac.h"
#include "mux/communication/comsys.h"
#include "mux/communication/comsys_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/owned_text.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/player.h"

void comsys_clear_player(EvaluationContext *evaluation, DbRef player) {
  int i;
  struct Commac *c;

  c = get_commac(evaluation->runtime->channels, player);

  for (i = (c->numchannels) - 1; i > -1; --i) {
    char *channel = commac_channel_at(c, (size_t)i);

    comsys_delete_channel_alias(evaluation, player, channel);
    free(channel);
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
  struct Commac *c;

  c = get_commac(evaluation->runtime->channels, player);

  if ((strcasecmp(arg1, "who") != 0) && (strcasecmp(arg1, "on") != 0) &&
      (strcasecmp(arg1, "off") != 0)) {
    raw_notify(evaluation, player,
               "Only options available are: on, off and who.");
    return;
  }
  for (i = 0; i < c->numchannels; i++) {
    comsys_process_alias_command(evaluation, player,
                                 commac_channel_at(c, (size_t)i), arg1);
    if (strcasecmp(arg1, "who") == 0)
      raw_notify(evaluation, player, "");
  }
}

void do_channelwho(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *arg1 = invocation->first;
  struct Channel *ch;
  struct Comuser *user;
  char channel[100];
  int flag = 0;
  char *option;
  int i;
  char ansibuffer[LBUF_SIZE];

  const size_t ARGUMENT_LENGTH = strlen(arg1);
  size_t slash_offset = 0;

  while (slash_offset < ARGUMENT_LENGTH &&
         *(const char *)checked_storage_at_const(
             arg1, ARGUMENT_LENGTH + 1, sizeof(char), slash_offset) != '/')
    slash_offset++;
  if (slash_offset == ARGUMENT_LENGTH) {
    (void)string_copy_bounded(channel, sizeof(channel), arg1);
  } else {
    /* channelname/all */
    if (slash_offset >= sizeof(channel)) {
      raw_notify(evaluation, player, "Channel name too long.");
      return;
    }
    (void)string_copy_bounded(channel, slash_offset + 1, arg1);
    option = checked_storage_at(arg1, ARGUMENT_LENGTH + 1, sizeof(char),
                                slash_offset + 1);
    if (*option == 'a')
      flag = 1;
  }

  ch = select_channel(evaluation->runtime->channels, channel);
  if (!ch) {
    raw_notify(evaluation, player, "@chan/who: Unknown channel.");
    return;
  }
  if (!is_wizard(evaluation->world->database, player)) {
    raw_notify(evaluation, player, "You do not have permission to do that.");
    return;
  }
  notify_printf(evaluation, player, "-- %s --", ch->name);
  notify_printf(evaluation, player, "%-29.29s %-6.6s %-6.6s", "Name", "Status",
                "Player");
  for (i = 0; i < ch->num_users; i++) {
    user = channel_user_at(ch, (size_t)i);
    if ((flag || is_undead(evaluation->world->database, user->who)) &&
        (!is_hidden(evaluation->world->database, user->who) ||
         ((ch->type & CHANNEL_TRANSPARENT) &&
          !is_dark(evaluation->world->database, user->who)) ||
         is_wizard(evaluation->world->database, player))) {
      OwnedText rendered = unparse_object(evaluation->world->database,
                                          evaluation, player, user->who);
      styled_text_strip(evaluation->world->styled_text_palette, rendered.text,
                        ansibuffer, LBUF_SIZE);
      notify_printf(evaluation, player, "%-29.29s %-6.6s %-6.6s", ansibuffer,
                    ((user->on) ? "on " : "off"),
                    (typeof_obj(evaluation->world->database, user->who) ==
                     OBJECT_TYPE_PLAYER)
                        ? "yes"
                        : "no ");
      owned_text_release(&rendered);
    }
  }
  notify_printf(evaluation, player, "-- %s --", ch->name);
}

static void do_comdisconnectraw_notify(EvaluationContext *evaluation,
                                       DbRef player, char *chan) {
  struct Channel *ch;
  struct Comuser *cu;

  ch = select_channel(evaluation->runtime->channels, chan);
  if (!ch)
    return;
  cu = select_user(ch, player);
  if (!cu)
    return;

  if ((ch->type & CHANNEL_LOUD) && (cu->on) &&
      (!is_dark(evaluation->world->database, player))) {
    comsys_channel_printf(
        evaluation, ch, "[%s] %s has disconnected.", ch->name,
        game_object_name(evaluation->world->database, player));
  }
}

static void do_comconnectraw_notify(EvaluationContext *evaluation, DbRef player,
                                    char *chan) {
  struct Channel *ch;
  struct Comuser *cu;

  ch = select_channel(evaluation->runtime->channels, chan);
  if (!ch)
    return;
  cu = select_user(ch, player);
  if (!cu)
    return;

  if ((ch->type & CHANNEL_LOUD) && (cu->on) &&
      (!is_dark(evaluation->world->database, player))) {
    comsys_channel_printf(
        evaluation, ch, "[%s] %s has connected.", ch->name,
        game_object_name(evaluation->world->database, player));
  }
}

static void do_comconnectchannel(EvaluationContext *evaluation, DbRef player,
                                 char *channel, const char *alias) {
  struct Channel *ch;
  struct Comuser *user;

  ch = select_channel(evaluation->runtime->channels, channel);
  if (ch) {
    for (user = ch->on_users; user && user->who != player; user = user->on_next)
      ;

    if (!user) {
      user = select_user(ch, player);
      if (user) {
        user->on_next = ch->on_users;
        ch->on_users = user;
      } else {
        notify_printf(evaluation, player,
                      "Bad Comsys Alias: %s for Channel: %s", alias, channel);
      }
    }
  } else {
    notify_printf(evaluation, player, "Bad Comsys Alias: %s for Channel: %s",
                  alias, channel);
  }
}

void do_comdisconnect(EvaluationContext *evaluation, DbRef player) {
  int i;
  struct Commac *c;

  c = get_commac(evaluation->runtime->channels, player);

  for (i = 0; i < c->numchannels; i++) {
    char *channel = commac_channel_at(c, (size_t)i);

    comsys_disconnect_channel(evaluation, player, channel);
    do_comdisconnectraw_notify(evaluation, player, channel);
  }
  send_channel(evaluation, "MUXConnections", "* %s has disconnected *",
               game_object_name(evaluation->world->database, player));
}

void do_comconnect(EvaluationContext *evaluation, DbRef player, Descriptor *d) {
  struct Commac *c;
  int i;
  char *lsite;

  c = get_commac(evaluation->runtime->channels, player);

  for (i = 0; i < c->numchannels; i++) {
    char *channel = commac_channel_at(c, (size_t)i);

    do_comconnectchannel(evaluation, player, channel,
                         commac_alias_at(c, (size_t)i));
    do_comconnectraw_notify(evaluation, player, channel);
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

void comsys_disconnect_channel(EvaluationContext *evaluation, DbRef player,
                               char *channel) {
  struct Comuser *user;
  struct Comuser *prevuser = nullptr;
  struct Channel *ch;

  ch = select_channel(evaluation->runtime->channels, channel);
  if (!ch)
    return;
  for (user = ch->on_users; user;) {
    if (user->who == player) {
      if (prevuser)
        prevuser->on_next = user->on_next;
      else
        ch->on_users = user->on_next;
      return;
    }
    prevuser = user;
    user = user->on_next;
  }
}
