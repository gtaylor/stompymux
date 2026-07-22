
/* comsys.h - Channel-system types, commands, and function declarations. */

#pragma once

#include <time.h>

#include "mux/commands/command_context.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/commac.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/network/descriptor.h"
#include "mux/support/fifo.h"

typedef struct CommandInvocation CommandInvocation;

typedef enum {
  CHAN_BOOT = 1 << 0,
  CHAN_OBJECT = 1 << 1,
  CHAN_CREATE = 1 << 2,
  CHAN_DESTROY = 1 << 3,
  CHAN_EMIT = 1 << 4,
  CHAN_LIST = 1 << 5,
  CHAN_OFLAGS = 1 << 6,
  CHAN_PFLAGS = 1 << 7,
  CHAN_FLAGS = 1 << 8,
  CHAN_STATUS = 1 << 9,
  CHAN_WHO = 1 << 10,
  CHAN_FULL = 1 << 11,
  CHAN_NOHEADER = 1 << 12,
} ChannelCommandKey;

constexpr int CHAN_OPERATION_MASK =
    CHAN_BOOT | CHAN_OBJECT | CHAN_CREATE | CHAN_DESTROY | CHAN_EMIT |
    CHAN_LIST | CHAN_OFLAGS | CHAN_PFLAGS | CHAN_FLAGS | CHAN_STATUS | CHAN_WHO;

typedef struct chanentry CHANENT;
struct chanentry {
  char *channame;
  struct channel *chan;
};

constexpr int CHAN_NAME_LEN = 50;
struct comuser {
  DbRef who;
  int on;
  struct comuser *on_next;
};

struct channel {
  char name[CHAN_NAME_LEN];
  int type;
  int num_users;
  int max_users;
  int chan_obj;
  struct comuser **users;
  struct comuser *on_users; /* Linked list of who is on */
  Fifo *last_messages;
  int num_messages;
};

typedef struct {
  time_t time;
  char *msg;
} chmsg;

int is_in_character_location(GameDatabase *database,
                             const ServerConfiguration *configuration,
                             DbRef player);
void init_chantab(ChannelRegistry *channels);
void send_channel(EvaluationContext *evaluation, const char *chan,
                  const char *format, ...)
    __attribute__((format(printf, 3, 4)));
struct channel *select_channel(ChannelRegistry *channels, const char *channel);
struct comuser *select_user(struct channel *ch, DbRef player);
void do_addcom(CommandInvocation *invocation);
void comsys_add_alias(EvaluationContext *evaluation, DbRef player, char *alias,
                      char *channel);
void do_delcom(CommandInvocation *invocation);
void do_chan(CommandInvocation *invocation);
void do_createchannel(CommandInvocation *invocation);
void do_destroychannel(CommandInvocation *invocation);
void do_comlist(CommandInvocation *invocation);
void do_clearcom(CommandInvocation *invocation);
void comsys_clear_player(EvaluationContext *evaluation, DbRef player);
void do_allcom(CommandInvocation *invocation);
void do_channelwho(CommandInvocation *invocation);
void do_comdisconnect(EvaluationContext *evaluation, DbRef player);
void do_comconnect(EvaluationContext *evaluation, DbRef player, Descriptor *d);
void do_channel_membership_flags(CommandInvocation *invocation);
void do_channel_object(CommandInvocation *invocation);
void do_channel_flags(CommandInvocation *invocation);
int do_comsystem(EvaluationContext *evaluation, DbRef who, char *cmd);
void do_cemit(CommandInvocation *invocation);
void do_chboot(CommandInvocation *invocation);
void do_chanstatus(CommandInvocation *invocation);
void do_chanlist(CommandInvocation *invocation);
void do_joinchannel(EvaluationContext *evaluation, DbRef player,
                    struct channel *ch);
void fun_cemit(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context);

constexpr int CHANNEL_JOIN = 0x001;
constexpr int CHANNEL_TRANSMIT = 0x002;
constexpr int CHANNEL_RECIEVE = 0x004;

constexpr int CHANNEL_PL_MULT = 0x001;
constexpr int CHANNEL_OBJ_MULT = 0x010;
constexpr int CHANNEL_LOUD = 0x100;
constexpr int CHANNEL_PUBLIC = 0x200;
constexpr int CHANNEL_TRANSPARENT = 0x400;

static inline bool is_undead(GameDatabase *database, DbRef x) {
  return (!is_god(database, x) || !is_going(database, x)) &&
         (!is_player(database, x) || is_connected(database, x));
}

/* Going objects are ignored only when they are God. Disconnected players are
 * not active channel recipients. */
