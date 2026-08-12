/* comsys.c - Player channel creation, membership, and message delivery. */

#include <stdlib.h>
#include <string.h>

#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
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
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/fifo.h"
#include "mux/support/hash_table.h"
#include "mux/support/utf8.h"
#include "mux/world/player.h"

static void comlist_description(GameDatabase *database, struct Channel *channel,
                                char *buffer, size_t buffer_size);

static void chan_show_switches(EvaluationContext *evaluation, DbRef player) {
  raw_notify(evaluation, player, "@chan command switches:");
  raw_notify(evaluation, player, "  /boot     Remove a member from a channel.");
  raw_notify(evaluation, player, "  /create   Create a channel.");
  raw_notify(evaluation, player, "  /destroy  Destroy a channel.");
  raw_notify(evaluation, player, "  /emit     Send an administrative message.");
  raw_notify(evaluation, player, "  /list     List channels.");
  raw_notify(evaluation, player, "  /object   Attach a channel object.");
  raw_notify(evaluation, player,
             "  /oflags   Set channel permissions for objects.");
  raw_notify(evaluation, player,
             "  /pflags   Set channel permissions for players.");
  raw_notify(evaluation, player, "  /flags    Set or clear channel flags.");
  raw_notify(evaluation, player, "  /status   Show a channel's status.");
  raw_notify(evaluation, player, "  /who      List a channel's members.");
}

static void chan_invalid_switches(EvaluationContext *evaluation, DbRef player) {
  raw_notify(evaluation, player, "Illegal combination of @chan switches.");
}

void do_chan(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  int key = invocation->key;
  int operation = key & CHAN_OPERATION_MASK;
  CommandInvocation routed = *invocation;

  if (key == 0) {
    chan_show_switches(evaluation, invocation->player);
    return;
  }

  switch (operation) {
  case CHAN_BOOT:
    if (key != CHAN_BOOT)
      break;
    routed.key = 0;
    do_chboot(&routed);
    return;
  case CHAN_CREATE:
    if (key != CHAN_CREATE)
      break;
    routed.key = 0;
    do_createchannel(&routed);
    return;
  case CHAN_DESTROY:
    if (key != CHAN_DESTROY)
      break;
    routed.key = 0;
    do_destroychannel(&routed);
    return;
  case CHAN_EMIT:
    if (key & ~(CHAN_EMIT | CHAN_NOHEADER))
      break;
    routed.key = key & CHAN_NOHEADER ? CEMIT_NOHEADER : 0;
    do_cemit(&routed);
    return;
  case CHAN_LIST:
    if (key & ~(CHAN_LIST | CHAN_FULL))
      break;
    routed.key = key & CHAN_FULL ? CLIST_FULL : 0;
    do_chanlist(&routed);
    return;
  case CHAN_OBJECT:
    if (key != CHAN_OBJECT)
      break;
    routed.key = 0;
    do_channel_object(&routed);
    return;
  case CHAN_OFLAGS:
    if (key != CHAN_OFLAGS)
      break;
    routed.key = 4;
    do_channel_membership_flags(&routed);
    return;
  case CHAN_PFLAGS:
    if (key != CHAN_PFLAGS)
      break;
    routed.key = 3;
    do_channel_membership_flags(&routed);
    return;
  case CHAN_FLAGS:
    if (key != CHAN_FLAGS)
      break;
    routed.key = 0;
    do_channel_flags(&routed);
    return;
  case CHAN_STATUS:
    if (key & ~(CHAN_STATUS | CHAN_FULL))
      break;
    routed.key = key & CHAN_FULL ? CSTATUS_FULL : 0;
    do_chanstatus(&routed);
    return;
  case CHAN_WHO:
    if (key != CHAN_WHO)
      break;
    routed.key = 0;
    do_channelwho(&routed);
    return;
  default:
    break;
  }
  chan_invalid_switches(evaluation, invocation->player);
}

void do_createchannel(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *channel = invocation->first;
  struct Channel *newchannel;

  if (select_channel(evaluation->runtime->channels, channel)) {
    notify_printf(evaluation, player, "Channel %s already exists.", channel);
    return;
  }
  if (!*channel) {
    raw_notify(evaluation, player, "You must specify a channel to create.");
    return;
  }
  if (strlen(channel) >= CHAN_NAME_LEN ||
      !utf8_is_printable_ascii(channel, strlen(channel)) ||
      strchr(channel, ' ')) {
    raw_notify(evaluation, player,
               "Channel names must be printable ASCII without spaces.");
    return;
  }
  if (!is_wizard(evaluation->world->database, player)) {
    raw_notify(evaluation, player, "You do not have permission to do that.");
    return;
  }
  newchannel = (struct Channel *)malloc(sizeof(struct Channel));

  strncpy(newchannel->name, channel, CHAN_NAME_LEN - 1);
  newchannel->name[CHAN_NAME_LEN - 1] = '\0';
  newchannel->last_messages = nullptr;
  newchannel->type = 127;
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

void channel_destroy(struct Channel *channel) {
  if (channel == nullptr)
    return;
  for (int index = 0; index < channel->num_users; index++)
    free(channel_user_at(channel, (size_t)index));
  free((void *)channel->users);
  while (channel->last_messages != nullptr &&
         fifo_length(&channel->last_messages) > 0) {
    Chmsg *message = fifo_pop(&channel->last_messages);
    free(message->msg);
    free(message);
  }
  free(channel->last_messages);
  free(channel);
}

void do_destroychannel(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *channel = invocation->first;
  struct Channel *ch;

  ch = (struct Channel *)hash_table_find(
      channel, &evaluation->runtime->channels->channels);

  if (!ch) {
    raw_notify(evaluation, player, "@chan/destroy: Unknown channel.");
    return;
  }
  if (!is_wizard(evaluation->world->database, player)) {
    raw_notify(evaluation, player, "You do not have permission to do that. ");
    return;
  }
  evaluation->runtime->channels->count--;
  hash_table_delete(channel, &evaluation->runtime->channels->channels);

  channel_destroy(ch);
  notify_printf(evaluation, player, "Channel %s destroyed.", channel);
}

void comsys_list_channels(EvaluationContext *evaluation, DbRef player) {
  struct Channel *ch;

  raw_notify(evaluation, player,
             "** Channel             --Flags--  Obj  Users   Messages");

  for (ch = (struct Channel *)hash_table_first_entry(
           &evaluation->runtime->channels->channels);
       ch; ch = (struct Channel *)hash_table_next_entry(
               &evaluation->runtime->channels->channels)) {
    notify_printf(
        evaluation, player, "%c%c %-20.20s %c%c%c/%c%c%c %5d %6d %10d",
        (ch->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
        (ch->type & (CHANNEL_LOUD)) ? 'L' : '-', ch->name,
        (ch->type & (CHANNEL_PL_MULT * CHANNEL_JOIN)) ? 'J' : '-',
        (ch->type & (CHANNEL_PL_MULT * CHANNEL_TRANSMIT)) ? 'X' : '-',
        (ch->type & (CHANNEL_PL_MULT * CHANNEL_RECIEVE)) ? 'R' : '-',
        (ch->type & (CHANNEL_OBJ_MULT * CHANNEL_JOIN)) ? 'j' : '-',
        (ch->type & (CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT)) ? 'x' : '-',
        (ch->type & (CHANNEL_OBJ_MULT * CHANNEL_RECIEVE)) ? 'r' : '-',
        (ch->chan_obj != NOTHING) ? ch->chan_obj : -1, ch->num_users,
        ch->num_messages);
  }
  raw_notify(evaluation, player, "-- End of list of Channels --");
}

void do_comlist(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  struct Channel *ch;
  struct Comuser *user;
  struct Commac *c;
  Descriptor *descriptor;
  char description[LBUF_SIZE];
  int description_width;
  int terminal_width;
  int i;

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
    char *channel_name = commac_channel_at(c, (size_t)i);

    ch = select_channel(evaluation->runtime->channels, channel_name);
    user = select_user(ch, player);
    if (user) {
      comlist_description(evaluation->world->database, ch, description,
                          (size_t)description_width + 1);
      notify_printf(evaluation, player, "%-9.9s %-19.19s %-6.6s %.*s",
                    commac_alias_at(c, (size_t)i), channel_name,
                    (user->on ? "on" : "off"), description_width, description);
    } else {
      notify_printf(evaluation, player, "Bad Comsys Alias: %s for Channel: %s",
                    commac_alias_at(c, (size_t)i), channel_name);
    }
  }
  raw_notify(evaluation, player, "-- End of comlist --");
}

static void comlist_description(GameDatabase *database, struct Channel *ch,
                                char *buffer, size_t buffer_size) {
  long flags;
  char *description;

  if (buffer_size == 0)
    return;
  if (ch->chan_obj == NOTHING) {
    strlcpy(buffer, "No description.", buffer_size);
    return;
  }

  description = attribute_get(database, ch->chan_obj, A_DESC, &flags);
  if (!*description) {
    strlcpy(buffer, "No description.", buffer_size);
  } else {
    const size_t DESCRIPTION_LENGTH = strlen(description);
    size_t output = 0;

    while (output < DESCRIPTION_LENGTH && output < buffer_size - 1) {
      const char CHARACTER = *(const char *)checked_storage_at_const(
          description, DESCRIPTION_LENGTH + 1, sizeof(char), output);

      *(char *)checked_storage_at(buffer, buffer_size, sizeof(char), output) =
          CHARACTER == '\r' || CHARACTER == '\n' ? ' ' : CHARACTER;
      output++;
    }
    *(char *)checked_storage_at(buffer, buffer_size, sizeof(char), output) =
        '\0';
  }
  free_lbuf(description);
}
