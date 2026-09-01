/** @file
 * Channel-system types, commands, and function declarations.
 */
#pragma once

#include <stdarg.h>
#include <stdint.h>
#include <time.h>

#include "mux/commands/command_context.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/commac.h"
#include "mux/network/descriptor.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
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

constexpr int CEMIT_NOHEADER = 1; /* Channel emit without header. */
constexpr int CLIST_FULL = 1;     /* Full listing of channels. */
constexpr int CSTATUS_FULL = 1;   /* Full listing of channel status. */

typedef struct Chanentry CHANENT;
struct Chanentry {
  char *channame;
  struct Channel *chan;
};

constexpr int CHAN_NAME_LEN = 50;
struct Comuser {
  DbRef who;
  int on;
  struct Comuser *on_next;
};

struct Channel {
  char name[CHAN_NAME_LEN];
  uint64_t generation;
  int type;
  int num_users;
  int max_users;
  int chan_obj;
  struct Comuser **users;
  struct Comuser *on_users; /* Linked list of who is on */
  Fifo *last_messages;
  int num_messages;
};

typedef enum ChannelCreateResult : int {
  CHANNEL_CREATE_OK,
  CHANNEL_CREATE_NAME_REQUIRED,
  CHANNEL_CREATE_NAME_INVALID,
  CHANNEL_CREATE_ALREADY_EXISTS,
} ChannelCreateResult;

typedef enum ChannelAddPlayerResult : int {
  CHANNEL_ADD_PLAYER_OK,
  CHANNEL_ADD_PLAYER_ALIAS_REQUIRED,
  CHANNEL_ADD_PLAYER_ALIAS_INVALID,
  CHANNEL_ADD_PLAYER_ALIAS_IN_USE,
  CHANNEL_ADD_PLAYER_CAPACITY_FAILURE,
} ChannelAddPlayerResult;

typedef struct {
  time_t time;
  char *msg;
} Chmsg;

/** Executes init chantab. @param[in,out] channels Channels. */

void init_chantab(ChannelRegistry *channels);
/** Destroys channel. @param[in,out] channel Channel. */

void channel_destroy(struct Channel *channel);
/** Creates and registers a channel after validating its native name rules.
 * @param[in,out] channels Channel registry.
 * @param[in] name Requested channel name.
 * @param[out] created Newly created channel on success.
 * @return The creation result. */

ChannelCreateResult comsys_channel_create(ChannelRegistry *channels,
                                          const char *name,
                                          struct Channel **created);
/** Removes and destroys a registered channel.
 * @param[in,out] channels Channel registry.
 * @param[in] channel Channel to remove.
 * @return Whether the exact channel was registered and removed. */

bool comsys_channel_destroy(ChannelRegistry *channels, struct Channel *channel);
/** Executes send channel. @param[in,out] evaluation Expression evaluation
 * context. @param[in] chan Chan. @param[in] format Format. */

void send_channel(EvaluationContext *evaluation, const char *chan,
                  const char *format, ...)
    __attribute__((format(printf, 3, 4)));
typedef struct ChannelMessageTarget {
  EvaluationContext *evaluation;
  const char *channel;
} ChannelMessageTarget;

/** Executes send channel v. @param[in] target Target object or value.
 * @param[in] format Format. @param[in,out] arguments Argument list. */

void send_channel_v(const ChannelMessageTarget *target, const char *format,
                    va_list arguments) __attribute__((format(printf, 2, 0)));
/** Executes select channel. @param[in,out] channels Channels. @param[in]
 * channel Channel. */

struct Channel *select_channel(ChannelRegistry *channels, const char *channel);
/** Executes select user. @param[in,out] ch Ch. @param[in] player Player object.
 */

struct Comuser *select_user(struct Channel *ch, DbRef player);
/** Returns channel user at. @param[in] channel Channel. @param[in] index
 * Zero-based index. */

struct Comuser *channel_user_at(const struct Channel *channel, size_t index);
/** Executes channel user slot. @param[in,out] channel Channel. @param[in] index
 * Zero-based index. */

struct Comuser **channel_user_slot(struct Channel *channel, size_t index);
/** Handles the addcom command. @param[in,out] invocation Command invocation. */

void do_addcom(CommandInvocation *invocation);
/** Adds alias to comsys. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. @param[in] arg1 Arg1. @param[in]
 * arg2 Arg2. */

void comsys_add_alias(EvaluationContext *evaluation, DbRef player,
                      const char *arg1, const char *arg2);
/** Administratively adds a player's alias and joins them to a channel without
 * applying its join lock.
 * @param[in,out] evaluation Expression evaluation context.
 * @param[in] player Player object to add.
 * @param[in,out] channel Channel to join.
 * @param[in] alias Player-local channel alias.
 * @param[in] quiet Whether to suppress the channel-wide join announcement.
 * @return The add-player result. */

ChannelAddPlayerResult comsys_channel_add_player(EvaluationContext *evaluation,
                                                 DbRef player,
                                                 struct Channel *channel,
                                                 const char *alias, bool quiet);
/** Handles the delcom command. @param[in,out] invocation Command invocation. */

void do_delcom(CommandInvocation *invocation);
/** Handles the chan command. @param[in,out] invocation Command invocation. */

void do_chan(CommandInvocation *invocation);
/** Handles the createchannel command. @param[in] invocation Command invocation.
 */

void do_createchannel(CommandInvocation *invocation);
/** Handles the destroychannel command. @param[in,out] invocation Command
 * invocation. */

void do_destroychannel(CommandInvocation *invocation);
/** Handles the comlist command. @param[in,out] invocation Command invocation.
 */

void do_comlist(CommandInvocation *invocation);
/** Handles the clearcom command. @param[in,out] invocation Command invocation.
 */

void do_clearcom(CommandInvocation *invocation);
/** Executes comsys clear player. @param[in,out] evaluation Expression
 * evaluation context. @param[in] player Player object. */

void comsys_clear_player(EvaluationContext *evaluation, DbRef player);
/** Handles the allcom command. @param[in,out] invocation Command invocation. */

void do_allcom(CommandInvocation *invocation);
/** Handles the channelwho command. @param[in,out] invocation Command
 * invocation. */

void do_channelwho(CommandInvocation *invocation);
/** Handles the comdisconnect command. @param[in,out] evaluation Expression
 * evaluation context. @param[in] player Player object. */

void do_comdisconnect(EvaluationContext *evaluation, DbRef player);
/** Handles the comconnect command. @param[in,out] evaluation Expression
 * evaluation context. @param[in] player Player object. @param[in,out] d D. */

void do_comconnect(EvaluationContext *evaluation, DbRef player, Descriptor *d);
/** Handles the channel membership flags command. @param[in,out] invocation
 * Command invocation. */

void do_channel_membership_flags(CommandInvocation *invocation);
/** Handles the channel object command. @param[in,out] invocation Command
 * invocation. */

void do_channel_object(CommandInvocation *invocation);
/** Handles the channel flags command. @param[in,out] invocation Command
 * invocation. */

void do_channel_flags(CommandInvocation *invocation);
/** Handles the comsystem command. @param[in,out] evaluation Expression
 * evaluation context. @param[in] who Who. @param[in,out] cmd Cmd. */

bool do_comsystem(EvaluationContext *evaluation, DbRef who, char *cmd);
/** Handles the cemit command. @param[in,out] invocation Command invocation. */

void do_cemit(CommandInvocation *invocation);
/** Handles the chboot command. @param[in,out] invocation Command invocation. */

void do_chboot(CommandInvocation *invocation);
/** Handles the chanstatus command. @param[in,out] invocation Command
 * invocation. */

void do_chanstatus(CommandInvocation *invocation);
/** Handles the chanlist command. @param[in,out] invocation Command invocation.
 */

void do_chanlist(CommandInvocation *invocation);
/** Handles the joinchannel command. @param[in,out] evaluation Expression
 * evaluation context. @param[in] player Player object. @param[in,out] ch Ch. */

void do_joinchannel(EvaluationContext *evaluation, DbRef player,
                    struct Channel *ch, bool quiet);
/** Executes fun cemit. @param[out] buff Caller-owned output storage.
 * @param[in,out] bufc Bufc. @param[in] player Player object. @param[in] cause
 * Object that caused the operation. @param[in,out] fargs Fargs. @param[in]
 * nfargs Nfargs. @param[in,out] cargs Cargs. @param[in] ncargs Ncargs.
 * @param[in,out] context Operation context. */

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

/** Reports whether is undead. @param[in] database Game database. @param[in] x
 * X. */

static inline bool is_undead(GameDatabase *database, DbRef x) {
  return ((!is_god(database, x) || !is_going(database, x)) &&
          (!is_player(database, x) || is_connected(database, x))) != 0;
}

/* Going objects are ignored only when they are God. Disconnected players are
 * not active channel recipients. */
