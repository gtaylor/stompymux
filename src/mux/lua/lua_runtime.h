/* lua_runtime.h - Lua runtime lifecycle and MUX callback declarations. */

#pragma once

#include "mux/database/db.h"

#include <stdbool.h>
#include <stddef.h>
#include <time.h>

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
typedef struct ServerLog ServerLog;

typedef enum LuaEventType {
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
  LUA_EVENT_MATCH_HEARD,
  LUA_EVENT_MATCH_HEARD_OTHER,
  LUA_EVENT_MATCH_HEARD_SELF,
  LUA_EVENT_CLONE,
  LUA_EVENT_SERVER_STARTUP,
  LUA_EVENT_CONNECT,
  LUA_EVENT_DISCONNECT,
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

typedef enum LuaLockType {
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

typedef enum LuaLockOperation {
  LUA_LOCK_OPERATION_MATCH,
  LUA_LOCK_OPERATION_TRAVERSE,
  LUA_LOCK_OPERATION_TAKE,
  LUA_LOCK_OPERATION_LOOK,
  LUA_LOCK_OPERATION_COMMAND_MATCH,
  LUA_LOCK_OPERATION_LISTEN,
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
  LUA_LOCK_OPERATION_ZONE_CONTROL,
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

struct LuaServices {
  /* Every member is borrowed from MuxServer. */
  const ServerConfiguration *configuration;
  GameDatabase *database;
  DescriptorRegistry *descriptors;
  CommandQueue *commands;
  RuntimeClock *clock;
  CommandContext *background_command;
  ServerLog *log;
  int *record_players;
};

struct LuaOwner {
  LuaRuntime *runtime;
};

static inline void lua_services_initialize(
    LuaServices *services, const ServerConfiguration *configuration,
    GameDatabase *database, DescriptorRegistry *descriptors,
    CommandQueue *commands, RuntimeClock *clock,
    CommandContext *background_command, ServerLog *log, int *record_players) {
  *services = (LuaServices){
      .configuration = configuration,
      .database = database,
      .descriptors = descriptors,
      .commands = commands,
      .clock = clock,
      .background_command = background_command,
      .log = log,
      .record_players = record_players,
  };
}

int lua_initialize(LuaOwner *owner, const LuaServices *services, char *error,
                   size_t error_size);
void lua_shutdown(LuaOwner *owner);
int lua_reload(LuaOwner *owner, char *error, size_t error_size);
int lua_check(EvaluationContext *evaluation, LuaRuntime *source, DbRef player,
              char *error, size_t error_size);
int lua_validate_path(LuaRuntime *runtime, const char *path, char *error,
                      size_t error_size);
int lua_command_match(LuaRuntime *runtime, Descriptor *descriptor, DbRef thing,
                      DbRef player, DbRef cause, const char *command);
int lua_list_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                           DbRef first, DbRef player, DbRef cause,
                           const char *command);
int lua_global_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                             DbRef player, DbRef cause, const char *command);
const char *lua_event_name(LuaEventType event);
bool lua_event_defined(LuaRuntime *runtime, DbRef object, LuaEventType event);
bool lua_event_dispatch(LuaRuntime *runtime,
                        const LuaEventInvocation *invocation);
const char *lua_lock_name(LuaLockType lock);
const char *lua_lock_operation_name(LuaLockOperation operation);
bool lua_lock_defined(LuaRuntime *runtime, DbRef object, LuaLockType lock);
void lua_lock_evaluate(LuaRuntime *runtime, const LuaLockInvocation *invocation,
                       LuaLockResult *result);
void lua_schedule_tick(LuaRuntime *runtime, time_t now);
