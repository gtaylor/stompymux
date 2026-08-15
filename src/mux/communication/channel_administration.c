/* comsys.c - Player channel creation, membership, and message delivery. */

#include <stdio.h>
#include <string.h>
#include <strings.h>

#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_helpers.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/commac.h"
#include "mux/communication/comsys.h"
#include "mux/communication/comsys_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/network_output.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/owned_text.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/player.h"

void do_channel_membership_flags(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int flag = invocation->key;
  char *arg1 = invocation->first;
  char *arg2 = invocation->second;
  char *s;
  struct Channel *ch;
  int add_remove = 1;

  ch = select_channel(evaluation->runtime->channels, arg1);
  if (!ch) {
    switch (flag) {
    case 3:
      raw_notify(evaluation, player, "@chan/pflags: Unknown channel.");
      break;
    case 4:
      raw_notify(evaluation, player, "@chan/oflags: Unknown channel.");
      break;
    default:
      raw_notify(evaluation, player, "@chan: Unknown channel.");
      break;
    }
    return;
  }
  if (!is_wizard(evaluation->world->database, player)) {
    raw_notify(evaluation, player, "Permission denied.");
    return;
  }
  s = arg2;
  if (*s == '!') {
    add_remove = 0;
    s = checked_mutable_string_suffix(s, 1);
  }
  switch (flag) {
  case 3:
    if (strcasecmp(s, "join") == 0) {
      add_remove ? (ch->type |= (CHANNEL_PL_MULT * CHANNEL_JOIN))
                 : (ch->type &= ~(CHANNEL_PL_MULT * CHANNEL_JOIN));
      raw_notify(evaluation, player,
                 (add_remove) ? "@chan/pflags: Set."
                              : "@chan/pflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "receive") == 0) {
      add_remove ? (ch->type |= (CHANNEL_PL_MULT * CHANNEL_RECIEVE))
                 : (ch->type &= ~(CHANNEL_PL_MULT * CHANNEL_RECIEVE));
      raw_notify(evaluation, player,
                 (add_remove) ? "@chan/pflags: Set."
                              : "@chan/pflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "transmit") == 0) {
      add_remove ? (ch->type |= (CHANNEL_PL_MULT * CHANNEL_TRANSMIT))
                 : (ch->type &= ~(CHANNEL_PL_MULT * CHANNEL_TRANSMIT));
      raw_notify(evaluation, player,
                 (add_remove) ? "@chan/pflags: Set."
                              : "@chan/pflags: Cleared.");
      return;
    }
    raw_notify(evaluation, player, "@chan/pflags: Unknown flag.");
    break;
  case 4:
    if (strcasecmp(s, "join") == 0) {
      add_remove ? (ch->type |= (CHANNEL_OBJ_MULT * CHANNEL_JOIN))
                 : (ch->type &= ~(CHANNEL_OBJ_MULT * CHANNEL_JOIN));
      raw_notify(evaluation, player,
                 (add_remove) ? "@chan/oflags: Set."
                              : "@chan/oflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "receive") == 0) {
      add_remove ? (ch->type |= (CHANNEL_OBJ_MULT * CHANNEL_RECIEVE))
                 : (ch->type &= ~(CHANNEL_OBJ_MULT * CHANNEL_RECIEVE));
      raw_notify(evaluation, player,
                 (add_remove) ? "@chan/oflags: Set."
                              : "@chan/oflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "transmit") == 0) {
      add_remove ? (ch->type |= (CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT))
                 : (ch->type &= ~(CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT));
      raw_notify(evaluation, player,
                 (add_remove) ? "@chan/oflags: Set."
                              : "@chan/oflags: Cleared.");
      return;
    }
    raw_notify(evaluation, player, "@chan/oflags: Unknown flag.");
    break;
  default:
    break;
  }
}

int comsys_test_access(const ChannelAccessRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef player = request->player;
  struct Channel *chan = request->channel;
  long flag_value = request->access;
  LuaLockInvocation lock;
  LuaLockResult result;

  if (is_wizard(evaluation->world->database, player))
    return (1);

  /*
   * Channel objects allow custom locks for channels.  The normal
   * lock is used to see if they can join that channel. The enter lock is
   * checked to see if they can receive messages on
   * it. The use lock is checked to see if they can transmit on
   * it. Note: These checks do not supercede the normal channel
   * flags. If a channel is set JOIN for players, ALL players can
   * join the channel, whether or not they pass the lock.  Same for
   * all channel object locks.
   */

  if ((flag_value & CHANNEL_JOIN) &&
      !((chan->chan_obj == NOTHING) || (chan->chan_obj == 0))) {
    if (lock_test(evaluation, player, player, player, chan->chan_obj,
                  LUA_LOCK_DEFAULT, LUA_LOCK_OPERATION_CHANNEL_JOIN, true,
                  &lock, &result))
      return (1);
  }
  if ((flag_value & CHANNEL_TRANSMIT) &&
      !((chan->chan_obj == NOTHING) || (chan->chan_obj == 0))) {
    if (lock_test(evaluation, player, player, player, chan->chan_obj,
                  LUA_LOCK_USE, LUA_LOCK_OPERATION_CHANNEL_TRANSMIT, true,
                  &lock, &result))
      return (1);
  }
  if ((flag_value & CHANNEL_RECIEVE) &&
      !((chan->chan_obj == NOTHING) || (chan->chan_obj == 0))) {
    if (lock_test(evaluation, player, player, player, chan->chan_obj,
                  LUA_LOCK_ENTER, LUA_LOCK_OPERATION_CHANNEL_RECEIVE, true,
                  &lock, &result))
      return (1);
  }
  if (typeof_obj(evaluation->world->database, player) == OBJECT_TYPE_PLAYER)
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
  const char *ch;
  char *alias;

  alias = alloc_lbuf("do_comsystem");
  const size_t LENGTH = strlen(cmd);
  size_t offset = 0;

  while (offset < LENGTH) {
    const char CHARACTER = *(const char *)checked_storage_at_const(
        cmd, LENGTH + 1, sizeof(char), offset);

    if (CHARACTER == ' ')
      break;
    *(char *)checked_storage_at(alias, LBUF_SIZE, sizeof(char), offset) =
        CHARACTER;
    offset++;
  }
  *(char *)checked_storage_at(alias, LBUF_SIZE, sizeof(char), offset) = '\0';
  t = checked_storage_at(cmd, LENGTH + 1, sizeof(char),
                         offset < LENGTH ? offset + 1 : offset);

  struct Commac *commac = get_commac(evaluation->runtime->channels, who);
  ch = commac_channel_for_alias(commac, alias);
  if (ch && *ch) {
    comsys_process_alias_command(evaluation, who, ch, t);
    free_buf(alias);
    return 0;
  }
  free_buf(alias);
  return 1;
}

void do_cemit(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *chan = invocation->first;
  char *text = invocation->second;
  struct Channel *ch;

  ch = select_channel(evaluation->runtime->channels, chan);
  if (!ch) {
    raw_notify(evaluation, player, "@chan/emit: Unknown channel.");
    return;
  }
  if (!is_wizard(evaluation->world->database, player)) {
    raw_notify(evaluation, player, "Permission denied.");
    return;
  }
  if (key == CEMIT_NOHEADER)
    comsys_send_channel_message(evaluation, ch, text);
  else
    comsys_channel_printf(evaluation, ch, "[%s] %s", chan, text);
}

void do_channel_flags(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *channel = invocation->first;
  char *flag = invocation->second;
  struct Channel *ch;
  int flag_value;
  bool enable = true;

  ch = select_channel(evaluation->runtime->channels, channel);
  if (!ch) {
    raw_notify(evaluation, player, "@chan/flags: Unknown channel.");
    return;
  }
  if (!is_wizard(evaluation->world->database, player)) {
    raw_notify(evaluation, player, "@chan/flags: Permission denied.");
    return;
  }
  if (*flag == '!') {
    enable = false;
    flag = checked_mutable_string_suffix(flag, 1);
  }
  if (strcasecmp(flag, "public") == 0) {
    flag_value = CHANNEL_PUBLIC;
  } else if (strcasecmp(flag, "loud") == 0) {
    flag_value = CHANNEL_LOUD;
  } else if (strcasecmp(flag, "transparent") == 0) {
    flag_value = CHANNEL_TRANSPARENT;
  } else {
    raw_notify(evaluation, player, "@chan/flags: Unknown flag.");
    return;
  }

  if (enable) {
    ch->type |= flag_value;
    raw_notify(evaluation, player, "@chan/flags: Set.");
  } else {
    ch->type &= ~flag_value;
    raw_notify(evaluation, player, "@chan/flags: Cleared.");
  }
}

void do_chboot(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *channel = invocation->first;
  char *victim = invocation->second;
  struct Channel *ch;
  DbRef thing;

  /*
   * * I sure hope it's not going to be that *
   * *  * *  * *  * * long.
   */

  ch = select_channel(evaluation->runtime->channels, channel);
  if (!ch) {
    raw_notify(evaluation, player, "@chan/boot: Unknown channel.");
    return;
  }
  if (!select_user(ch, player)) {
    raw_notify(evaluation, player, "@chan/boot: You are not on that channel.");
    return;
  }
  if (!is_wizard(evaluation->world->database, player)) {
    raw_notify(evaluation, player, "Permission denied.");
    return;
  }
  thing = match_thing(&evaluation->command->match, player, victim);

  if (thing == NOTHING) {
    return;
  }
  if (!select_user(ch, thing)) {
    notify_printf(evaluation, player, "@chan/boot: %s is not on the channel.",
                  game_object_name(evaluation->world->database, thing));
    return;
  }
  /*
   * We should be in the clear now. :)
   */
  OwnedText player_name =
      unparse_object_numonly(evaluation->world->database, player);
  OwnedText thing_name =
      unparse_object_numonly(evaluation->world->database, thing);
  comsys_channel_printf(evaluation, ch, "[%s] %s boots %s off the channel.",
                        ch->name, player_name.text, thing_name.text);
  owned_text_release(&player_name);
  owned_text_release(&thing_name);
  comsys_delete_channel_alias(evaluation, thing, channel);
}

void do_channel_object(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *channel = invocation->first;
  char *object = invocation->second;
  struct Channel *ch;
  DbRef thing;
  OwnedText buff;

  init_match(&evaluation->command->match, player, object, OBJECT_TYPE_NOTYPE);
  match_everything(&evaluation->command->match, 0);
  thing = match_result(&evaluation->command->match);

  ch = select_channel(evaluation->runtime->channels, channel);
  if (!ch) {
    raw_notify(evaluation, player, "@chan/object: Unknown channel.");
    return;
  }
  if (thing == NOTHING) {
    ch->chan_obj = NOTHING;
    raw_notify(evaluation, player, "@chan/object: Set.");
    return;
  }
  if (!is_wizard(evaluation->world->database, player)) {
    raw_notify(evaluation, player, "@chan/object: Permission denied.");
    return;
  }
  ch->chan_obj = (int)thing;
  buff = unparse_object(evaluation->world->database, evaluation, player, thing);
  notify_printf(evaluation, player,
                "Channel %s is now using %s as channel object.", ch->name,
                buff.text);
  owned_text_release(&buff);
}

void do_chanlist(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  struct Channel *ch;
  long flags;
  char temp[MBUF_SIZE];
  char buf[MBUF_SIZE];
  OwnedText atrstr;

  flags = 0;

  if (key & CLIST_FULL) {
    comsys_list_channels(evaluation, player);
    return;
  }
  raw_notify(evaluation, player, "** Channel       Description");

  for (ch = (struct Channel *)hash_table_first_entry(
           &evaluation->runtime->channels->channels);
       ch; ch = (struct Channel *)hash_table_next_entry(
               &evaluation->runtime->channels->channels)) {
    if (is_wizard(evaluation->world->database, player) ||
        (ch->type & CHANNEL_PUBLIC) ||
        (comsys_test_access(&(ChannelAccessRequest){.evaluation = evaluation,
                                                    .player = player,
                                                    .access = CHANNEL_JOIN,
                                                    .channel = ch}))) {

      atrstr = attribute_get(evaluation->world->database, ch->chan_obj, A_DESC,
                             &flags);
      if ((ch->chan_obj == NOTHING) || !*atrstr.text)
        (void)snprintf(buf, MBUF_SIZE, "%s", "No description.");
      else
        (void)snprintf(buf, MBUF_SIZE, "%-54.54s", atrstr.text);

      owned_text_release(&atrstr);
      (void)snprintf(temp, MBUF_SIZE, "%c%c %-13.13s %-60.60s",
                     (ch->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
                     (ch->type & (CHANNEL_LOUD)) ? 'L' : '-', ch->name, buf);

      raw_notify(evaluation, player, temp);
    }
  }
  raw_notify(evaluation, player, "-- End of list of Channels --");
}

void do_chanstatus(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *chan = invocation->first;
  struct Channel *ch;
  long flags;
  OwnedText atrstr;

  if (key & CSTATUS_FULL) {
    struct Channel *selected_channel;
    raw_notify(evaluation, player,
               "** Channel             --Flags--  Obj  Users   Messages");

    selected_channel = select_channel(evaluation->runtime->channels, chan);
    if (!selected_channel) {
      raw_notify(evaluation, player, "@chan/status: Unknown channel.");
      return;
    }
    notify_printf(
        evaluation, player, "%c%c %-20.20s %c%c%c/%c%c%c %5d %6d %10d",
        (selected_channel->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
        (selected_channel->type & (CHANNEL_LOUD)) ? 'L' : '-',
        selected_channel->name,
        (selected_channel->type & (CHANNEL_PL_MULT * CHANNEL_JOIN)) ? 'J' : '-',
        (selected_channel->type & (CHANNEL_PL_MULT * CHANNEL_TRANSMIT)) ? 'X'
                                                                        : '-',
        (selected_channel->type & (CHANNEL_PL_MULT * CHANNEL_RECIEVE)) ? 'R'
                                                                       : '-',
        (selected_channel->type & (CHANNEL_OBJ_MULT * CHANNEL_JOIN)) ? 'j'
                                                                     : '-',
        (selected_channel->type & (CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT)) ? 'x'
                                                                         : '-',
        (selected_channel->type & (CHANNEL_OBJ_MULT * CHANNEL_RECIEVE)) ? 'r'
                                                                        : '-',
        (selected_channel->chan_obj != NOTHING) ? selected_channel->chan_obj
                                                : -1,
        selected_channel->num_users, selected_channel->num_messages);
    raw_notify(evaluation, player, "-- End of list of Channels --");
    return;
  }
  char temp[MBUF_SIZE];
  char buf[MBUF_SIZE];

  raw_notify(evaluation, player, "** Channel       Description");
  ch = select_channel(evaluation->runtime->channels, chan);
  if (!ch) {
    raw_notify(evaluation, player, "@chan/status: Unknown channel.");
    return;
  }
  atrstr =
      attribute_get(evaluation->world->database, ch->chan_obj, A_DESC, &flags);
  if ((ch->chan_obj == NOTHING) || !*atrstr.text)
    (void)snprintf(buf, MBUF_SIZE, "%s", "No description.");
  else
    (void)snprintf(buf, MBUF_SIZE, "%-54.54s", atrstr.text);

  owned_text_release(&atrstr);
  (void)snprintf(temp, MBUF_SIZE, "%c%c %-13.13s %-60.60s",
                 (ch->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
                 (ch->type & (CHANNEL_LOUD)) ? 'L' : '-', ch->name, buf);

  raw_notify(evaluation, player, temp);
  raw_notify(evaluation, player, "-- End of list of Channels --");
}
