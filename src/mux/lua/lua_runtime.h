/** @file
 * Lua runtime lifecycle and MUX callback declarations.
 */
#pragma once

#include <stddef.h>
#include <time.h>

#include "mux/commands/command_context.h"
#include "mux/commands/command_runtime.h"
#include "mux/network/descriptor.h"
#include "mux/objects/db.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/world/world_context.h"

struct LuaOwner; // IWYU pragma: keep

typedef struct LuaRuntime LuaRuntime;
typedef struct LuaServices LuaServices;
typedef struct LuaOwner LuaOwner;
typedef struct CommandContext CommandContext;
typedef struct CommandQueue CommandQueue;
typedef struct Descriptor Descriptor;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ConfigurationRegistry ConfigurationRegistry;
typedef struct ServerLog ServerLog;
typedef struct StyledTextPalette StyledTextPalette;

typedef void (*LuaCommandVisitor)(void *context, const char *source,
                                  DbRef object, const char *pattern);

typedef enum LuaEventType : int {
  LUA_EVENT_NONE,
  LUA_EVENT_SUCCESS,
  LUA_EVENT_FAIL,
  LUA_EVENT_DROP,
  LUA_EVENT_GIVE_FAIL,
  LUA_EVENT_GIVE_RECEIVE_FAIL,
  LUA_EVENT_DROP_FAIL,
  LUA_EVENT_USE,
  LUA_EVENT_USE_FAIL,
  LUA_EVENT_DESCRIBE,
  LUA_EVENT_ENTER,
  LUA_EVENT_LEAVE,
  LUA_EVENT_MOVE,
  LUA_EVENT_ENTER_FAIL,
  LUA_EVENT_LEAVE_FAIL,
  LUA_EVENT_TELEPORT,
  LUA_EVENT_TELEPORT_DESTINATION_FAIL,
  LUA_EVENT_TELEPORT_OUT_FAIL,
  LUA_EVENT_CLONE,
  LUA_EVENT_SERVER_FIRST_STARTUP,
  LUA_EVENT_SERVER_STARTUP,
  LUA_EVENT_PLAYER_CONNECT,
  LUA_EVENT_PLAYER_DISCONNECT,
  LUA_EVENT_MECH_DESTROYED,
  LUA_EVENT_MECH_MINE_TRIGGER,
  LUA_EVENT_AERO_LAND,
  LUA_EVENT_OOD_LAND,
  LUA_EVENT_COUNT,
} LuaEventType;

typedef struct LuaEventInvocation {
  LuaEventType type;
  Descriptor *descriptor;
  DbRef object;
  DbRef enactor;
  DbRef cause;
  char **arguments;
  int argument_count;
  bool reconnect;
  const char *reason;
} LuaEventInvocation;

typedef enum LuaAppearanceType : int {
  LUA_APPEARANCE_INTERNAL,
  LUA_APPEARANCE_EXTERNAL,
} LuaAppearanceType;

typedef struct LuaAppearanceInvocation {
  LuaAppearanceType type;
  Descriptor *descriptor;
  DbRef object;
  DbRef enactor;
  DbRef cause;
} LuaAppearanceInvocation;

typedef struct LuaAppearanceResult {
  bool defined;
  char rendered[LBUF_SIZE];
} LuaAppearanceResult;

typedef struct LuaMechStatusInvocation {
  Descriptor *descriptor;
  DbRef object;
  DbRef enactor;
  DbRef cause;
} LuaMechStatusInvocation;

typedef struct LuaMechStatusResult {
  bool defined;
  char rendered[LBUF_SIZE];
} LuaMechStatusResult;

typedef enum LuaLockType : int {
  LUA_LOCK_DEFAULT,
  LUA_LOCK_DROP,
  LUA_LOCK_ENTER,
  LUA_LOCK_GIVE,
  LUA_LOCK_LEAVE,
  LUA_LOCK_LINK,
  LUA_LOCK_RECEIVE,
  LUA_LOCK_SPEECH,
  LUA_LOCK_TELEPORT,
  LUA_LOCK_TELEPORT_OUT,
  LUA_LOCK_USE,
  LUA_LOCK_COUNT,
} LuaLockType;

typedef enum LuaLockOperation : int {
  LUA_LOCK_OPERATION_MATCH,
  LUA_LOCK_OPERATION_TRAVERSE,
  LUA_LOCK_OPERATION_TAKE,
  LUA_LOCK_OPERATION_LOOK,
  LUA_LOCK_OPERATION_COMMAND_MATCH,
  LUA_LOCK_OPERATION_USE,
  LUA_LOCK_OPERATION_DROP,
  LUA_LOCK_OPERATION_GIVE,
  LUA_LOCK_OPERATION_RECEIVE,
  LUA_LOCK_OPERATION_ENTER,
  LUA_LOCK_OPERATION_LEAVE,
  LUA_LOCK_OPERATION_TELEPORT,
  LUA_LOCK_OPERATION_TELEPORT_OUT,
  LUA_LOCK_OPERATION_LINK,
  LUA_LOCK_OPERATION_SET_HOME,
  LUA_LOCK_OPERATION_SPEAK,
  LUA_LOCK_OPERATION_CHANNEL_JOIN,
  LUA_LOCK_OPERATION_CHANNEL_TRANSMIT,
  LUA_LOCK_OPERATION_CHANNEL_RECEIVE,
  LUA_LOCK_OPERATION_BTECH_ENTER,
  LUA_LOCK_OPERATION_BTECH_CONTACT,
  LUA_LOCK_OPERATION_COUNT,
} LuaLockOperation;

typedef struct LuaLockInvocation {
  LuaLockType type;
  LuaLockOperation operation;
  Descriptor *descriptor;
  DbRef object;
  DbRef enactor;
  DbRef cause;
  DbRef subject;
  bool silent;
} LuaLockInvocation;

typedef struct LuaLockResult {
  bool defined;
  bool passes;
  bool has_enactor_message;
  bool has_other_message;
  char enactor_message[LBUF_SIZE];
  char other_message[LBUF_SIZE];
} LuaLockResult;

typedef enum LuaMessageType : int {
  LUA_MESSAGE_NONE,
  LUA_MESSAGE_SUCCESS,
  LUA_MESSAGE_DROP,
  LUA_MESSAGE_DESCRIBE,
  LUA_MESSAGE_USE,
  LUA_MESSAGE_LEAVE,
  LUA_MESSAGE_ENTER,
  LUA_MESSAGE_MOVE,
  LUA_MESSAGE_TELEPORT,
  LUA_MESSAGE_ENTER_SOURCE,
  LUA_MESSAGE_LEAVE_DESTINATION,
  LUA_MESSAGE_TELEPORT_SOURCE,
  LUA_MESSAGE_COUNT,
} LuaMessageType;

typedef enum LuaMessageOperation : int {
  LUA_MESSAGE_OPERATION_NONE,
  LUA_MESSAGE_OPERATION_LOOK,
  LUA_MESSAGE_OPERATION_TAKE,
  LUA_MESSAGE_OPERATION_TRAVERSE,
  LUA_MESSAGE_OPERATION_RECEIVE,
  LUA_MESSAGE_OPERATION_DROP,
  LUA_MESSAGE_OPERATION_GIVE,
  LUA_MESSAGE_OPERATION_DESCRIBE,
  LUA_MESSAGE_OPERATION_INSIDE_DESCRIBE,
  LUA_MESSAGE_OPERATION_USE,
  LUA_MESSAGE_OPERATION_MOVE,
  LUA_MESSAGE_OPERATION_TELEPORT,
  LUA_MESSAGE_OPERATION_COUNT,
} LuaMessageOperation;

typedef struct LuaMessageInvocation {
  LuaMessageType type;
  LuaMessageOperation operation;
  Descriptor *descriptor;
  DbRef object;
  DbRef enactor;
  DbRef cause;
  DbRef source;
  DbRef destination;
  bool silent;
} LuaMessageInvocation;

typedef struct LuaMessageResult {
  bool defined;
  bool has_enactor_message;
  bool has_other_message;
  char enactor_message[LBUF_SIZE];
  char other_message[LBUF_SIZE];
} LuaMessageResult;

struct LuaServices {
  /* Every member is borrowed from MuxServer. */
  const ServerConfiguration *configuration;
  const ConfigurationRegistry *configuration_registry;
  GameDatabase *database;
  DescriptorRegistry *descriptors;
  CommandQueue *commands;
  RuntimeClock *clock;
  CommandContext *background_command;
  ServerLog *log;
  const int *record_players;
  StyledTextPalette *styled_text_palette;
};

struct LuaOwner {
  LuaRuntime *runtime;
};

/** Initializes lua services. @param[out] services Services. @param[in]
 * configuration Server configuration. @param[in] database Game database.
 * @param[in] descriptors Descriptors. @param[in] commands Commands. @param[in]
 * clock Clock. @param[in] background_command Background command. @param[in] log
 * Server log. @param[in] record_players Record players. @param[in] palette
 * Palette. */

static inline void lua_services_initialize(
    LuaServices *services, const ServerConfiguration *configuration,
    const ConfigurationRegistry *configuration_registry, GameDatabase *database,
    DescriptorRegistry *descriptors, CommandQueue *commands,
    RuntimeClock *clock, CommandContext *background_command, ServerLog *log,
    const int *record_players, StyledTextPalette *palette) {
  *services = (LuaServices){
      .configuration = configuration,
      .configuration_registry = configuration_registry,
      .database = database,
      .descriptors = descriptors,
      .commands = commands,
      .clock = clock,
      .background_command = background_command,
      .log = log,
      .record_players = record_players,
      .styled_text_palette = palette,
  };
}

/** Initializes lua. @param[out] owner Owning runtime object. @param[in]
 * services Services. @param[out] error Storage receiving an error description.
 * @param[in] error_size Size of error in bytes. */

bool lua_initialize(LuaOwner *owner, const LuaServices *services, char *error,
                    size_t error_size);
/** Executes lua shutdown. @param[in,out] owner Owning runtime object. */

void lua_shutdown(LuaOwner *owner);
/** Executes lua reload. @param[in,out] owner Owning runtime object. @param[out]
 * error Storage receiving an error description. @param[in] error_size Size of
 * error in bytes. */

bool lua_reload(LuaOwner *owner, char *error, size_t error_size);
/** Executes lua check. @param[in] evaluation Expression evaluation context.
 * @param[in] source Source value. @param[in] player Player object. @param[out]
 * error Storage receiving an error description. @param[in] error_size Size of
 * error in bytes. */

int lua_check(EvaluationContext *evaluation, LuaRuntime *source, DbRef player,
              char *error, size_t error_size);
/** Executes lua validate path. @param[in,out] runtime Runtime services.
 * @param[in] path Filesystem path. @param[out] error Storage receiving an error
 * description. @param[in] error_size Size of error in bytes. */

bool lua_validate_path(LuaRuntime *runtime, const char *path, char *error,
                       size_t error_size);
typedef struct LuaExamineObjectRequest {
  LuaRuntime *runtime;
  EvaluationContext *evaluation;
  DbRef viewer;
  DbRef object;
} LuaExamineObjectRequest;

/** Executes lua examine object. @param[in] request Request. */

void lua_examine_object(const LuaExamineObjectRequest *request);
/** Executes lua command match. @param[in,out] runtime Runtime services.
 * @param[in,out] descriptor Network descriptor. @param[in] thing Thing.
 * @param[in] player Player object. @param[in] cause Object that caused the
 * operation. @param[in] command Command text or descriptor. */

int lua_command_match(LuaRuntime *runtime, Descriptor *descriptor, DbRef thing,
                      DbRef player, DbRef cause, const char *command);
/** Executes lua appearance evaluate. @param[in,out] runtime Runtime services.
 * @param[in] invocation Command invocation. @param[out] result Result. */

void lua_appearance_evaluate(LuaRuntime *runtime,
                             const LuaAppearanceInvocation *invocation,
                             LuaAppearanceResult *result);
/** Executes lua mech status evaluate. @param[in,out] runtime Runtime services.
 * @param[in] invocation Command invocation. @param[out] result Result. */

void lua_mech_status_evaluate(LuaRuntime *runtime,
                              const LuaMechStatusInvocation *invocation,
                              LuaMechStatusResult *result);
/** Executes lua list command match. @param[in,out] runtime Runtime services.
 * @param[in,out] descriptor Network descriptor. @param[in] first First.
 * @param[in] player Player object. @param[in] cause Object that caused the
 * operation. @param[in] command Command text or descriptor. */

int lua_list_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                           DbRef first, DbRef player, DbRef cause,
                           const char *command);
/** Executes lua global command match. @param[in,out] runtime Runtime services.
 * @param[in,out] descriptor Network descriptor. @param[in] player Player
 * object. @param[in] cause Object that caused the operation. @param[in] command
 * Command text or descriptor. */

bool lua_global_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                              DbRef player, DbRef cause, const char *command);
/** Executes lua visit global commands. @param[in,out] runtime Runtime services.
 * @param[in] player Player object. @param[in] visitor Visitor. @param[in,out]
 * context Operation context. */

size_t lua_visit_global_commands(LuaRuntime *runtime, DbRef player,
                                 LuaCommandVisitor visitor, void *context);
/** Executes lua visit object commands. @param[in,out] runtime Runtime services.
 * @param[in] object Game object. @param[in] player Player object. @param[in]
 * visitor Visitor. @param[in,out] context Operation context. */

size_t lua_visit_object_commands(LuaRuntime *runtime, DbRef object,
                                 DbRef player, LuaCommandVisitor visitor,
                                 void *context);
/** Executes lua event name. @param[in] event Event. */

const char *lua_event_name(LuaEventType event);
/** Executes lua event defined. @param[in,out] runtime Runtime services.
 * @param[in] object Game object. @param[in] event Event. */

bool lua_event_defined(LuaRuntime *runtime, DbRef object, LuaEventType event);
/** Executes lua event dispatch. @param[in,out] runtime Runtime services.
 * @param[in] invocation Command invocation. */

bool lua_event_dispatch(LuaRuntime *runtime,
                        const LuaEventInvocation *invocation);
/** Dispatches an event to all matching global logic modules in lexical order.
 * @param[in,out] runtime Runtime services. @param[in] invocation Event
 * invocation. */

void lua_global_event_dispatch(LuaRuntime *runtime,
                               const LuaEventInvocation *invocation);
/** Executes lua lock name. @param[in] lock Lock. */

const char *lua_lock_name(LuaLockType lock);
/** Executes lua lock operation name. @param[in] operation Operation. */

const char *lua_lock_operation_name(LuaLockOperation operation);
/** Executes lua lock defined. @param[in,out] runtime Runtime services.
 * @param[in] object Game object. @param[in] lock Lock. */

bool lua_lock_defined(LuaRuntime *runtime, DbRef object, LuaLockType lock);
/** Executes lua lock evaluate. @param[in,out] runtime Runtime services.
 * @param[in] invocation Command invocation. @param[out] result Result. */

void lua_lock_evaluate(LuaRuntime *runtime, const LuaLockInvocation *invocation,
                       LuaLockResult *result);
/** Executes lua message name. @param[in] message Message. */

const char *lua_message_name(LuaMessageType message);
/** Executes lua message operation name. @param[in] operation Operation. */

const char *lua_message_operation_name(LuaMessageOperation operation);
/** Executes lua message defined. @param[in,out] runtime Runtime services.
 * @param[in] object Game object. @param[in] message Message. */

bool lua_message_defined(LuaRuntime *runtime, DbRef object,
                         LuaMessageType message);
/** Executes lua message evaluate. @param[in,out] runtime Runtime services.
 * @param[in] invocation Command invocation. @param[out] result Result. */

void lua_message_evaluate(LuaRuntime *runtime,
                          const LuaMessageInvocation *invocation,
                          LuaMessageResult *result);
/** Executes lua schedule tick. @param[in,out] runtime Runtime services.
 * @param[in] now Now. */

void lua_schedule_tick(LuaRuntime *runtime, time_t now);
