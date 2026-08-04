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
#include "mux/support/styled_text/markup.h"
#include "mux/world/player.h"

void comsys_clear_player(EvaluationContext *evaluation, DbRef player) {
  int i;
  struct commac *c;

  c = get_commac(evaluation->runtime->channels, player);

  for (i = (c->numchannels) - 1; i > -1; --i) {
    comsys_delete_channel_alias(evaluation, player, c->channels[i]);
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

  c = get_commac(evaluation->runtime->channels, player);

  if ((strcasecmp(arg1, "who") != 0) && (strcasecmp(arg1, "on") != 0) &&
      (strcasecmp(arg1, "off") != 0)) {
    raw_notify(evaluation, player,
               "Only options available are: on, off and who.");
    return;
  }
  for (i = 0; i < c->numchannels; i++) {
    comsys_process_alias_command(evaluation, player, c->channels[i], arg1);
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
    user = ch->users[i];
    if ((flag || is_undead(evaluation->world->database, user->who)) &&
        (!is_hidden(evaluation->world->database, user->who) ||
         ((ch->type & CHANNEL_TRANSPARENT) &&
          !is_dark(evaluation->world->database, user->who)) ||
         is_wizard(evaluation->world->database, player))) {
      cp = unparse_object(evaluation->world->database, evaluation, player,
                          user->who);
      styled_text_strip(evaluation->world->styled_text_palette, cp, ansibuffer,
                        LBUF_SIZE);
      notify_printf(evaluation, player, "%-29.29s %-6.6s %-6.6s", ansibuffer,
                    ((user->on) ? "on " : "off"),
                    (typeof_obj(evaluation->world->database, user->who) ==
                     OBJECT_TYPE_PLAYER)
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
    comsys_channel_printf(
        evaluation, ch, "[%s] %s has disconnected.", ch->name,
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
    comsys_channel_printf(
        evaluation, ch, "[%s] %s has connected.", ch->name,
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
    comsys_disconnect_channel(evaluation, player, c->channels[i]);
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

void comsys_disconnect_channel(EvaluationContext *evaluation, DbRef player,
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
