/* mux_comsys_bindings.c - Lua bindings for mux.comsys. */

#include <lauxlib.h>
#include <lua.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/communication/channel_registry.h"
#include "mux/communication/comsys.h"
#include "mux/communication/comsys_internal.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/owned_text.h"
#include "mux/support/utf8.h"

static const char LUA_MUX_CHANNEL_METATABLE[] = "btmux.channel";
static const char LUA_MUX_CHANNEL_FLAGS_METATABLE[] = "btmux.channel_flags";
static const char LUA_MUX_CHANNEL_FLAG_METATABLE[] = "btmux.channel_flag";
static const char LUA_MUX_CHANNEL_FLAG_NAMESPACE_METATABLE[] =
    "btmux.channel_flag_namespace";

typedef struct LuaMuxChannel LuaMuxChannel;
struct LuaMuxChannel {
  LuaMuxPackage *package;
  struct Channel *identity;
  uint64_t generation;
  char name[CHAN_NAME_LEN];
};

typedef struct LuaMuxChannelFlags LuaMuxChannelFlags;
struct LuaMuxChannelFlags {
  LuaMuxPackage *package;
  struct Channel *identity;
  uint64_t generation;
  char name[CHAN_NAME_LEN];
};

typedef struct LuaMuxChannelFlag LuaMuxChannelFlag;
struct LuaMuxChannelFlag {
  LuaMuxPackage *package;
  int value;
  const char *name;
};

typedef struct LuaMuxChannelFlagNamespace LuaMuxChannelFlagNamespace;
struct LuaMuxChannelFlagNamespace {
  LuaMuxPackage *package;
};

typedef struct ChannelFlagDefinition ChannelFlagDefinition;
struct ChannelFlagDefinition {
  int value;
  const char *name;
};

typedef struct LuaMuxChannelMethod LuaMuxChannelMethod;
struct LuaMuxChannelMethod {
  const char *name;
  lua_CFunction function;
};

static const ChannelFlagDefinition CHANNEL_FLAGS[] = {
    {CHANNEL_PUBLIC, "PUBLIC"},
    {CHANNEL_LOUD, "LOUD"},
    {CHANNEL_TRANSPARENT, "TRANSPARENT"},
};

static const ChannelFlagDefinition *lua_mux_channel_flag_at(size_t index) {
  return checked_storage_at_const(
      CHANNEL_FLAGS, sizeof(CHANNEL_FLAGS) / sizeof(*CHANNEL_FLAGS),
      sizeof(*CHANNEL_FLAGS), index);
}

static ChannelRegistry *lua_mux_channel_registry(LuaMuxPackage *package) {
  return package->services->background_command->evaluation.runtime->channels;
}

static struct Channel *
lua_mux_check_channel_identity(LuaMuxPackage *package, lua_State *state,
                               int argument, const char *name,
                               struct Channel *identity, uint64_t generation) {
  struct Channel *current =
      select_channel(lua_mux_channel_registry(package), name);

  if (current == nullptr || current != identity ||
      current->generation != generation)
    lua_error_arg(state, argument, LUA_ERROR_CODE_CHANNEL_INVALID,
                  "channel no longer exists");
  return current;
}

static LuaMuxChannel *lua_mux_check_channel(lua_State *state, int argument) {
  LuaMuxChannel *handle =
      luaL_checkudata(state, argument, LUA_MUX_CHANNEL_METATABLE);

  lua_mux_require_runtime(handle->package, state, "comsys.Channel");
  (void)lua_mux_check_channel_identity(handle->package, state, argument,
                                       handle->name, handle->identity,
                                       handle->generation);
  return handle;
}

static LuaMuxChannelFlags *lua_mux_check_channel_flags(lua_State *state,
                                                       int argument) {
  LuaMuxChannelFlags *handle =
      luaL_checkudata(state, argument, LUA_MUX_CHANNEL_FLAGS_METATABLE);

  lua_mux_require_runtime(handle->package, state, "comsys.Channel:flags");
  (void)lua_mux_check_channel_identity(handle->package, state, argument,
                                       handle->name, handle->identity,
                                       handle->generation);
  return handle;
}

static LuaMuxChannel *lua_mux_push_channel(lua_State *state,
                                           LuaMuxPackage *package,
                                           struct Channel *channel) {
  LuaMuxChannel *handle = lua_newuserdata(state, sizeof(*handle));

  *handle = (LuaMuxChannel){
      .package = package,
      .identity = channel,
      .generation = channel->generation,
  };
  (void)string_copy_bounded(handle->name, sizeof(handle->name), channel->name);
  luaL_getmetatable(state, LUA_MUX_CHANNEL_METATABLE);
  lua_setmetatable(state, -2);
  return handle;
}

static void lua_mux_push_channel_flag(lua_State *state, LuaMuxPackage *package,
                                      const ChannelFlagDefinition *definition) {
  LuaMuxChannelFlag *flag = lua_newuserdata(state, sizeof(*flag));

  *flag = (LuaMuxChannelFlag){
      .package = package,
      .value = definition->value,
      .name = definition->name,
  };
  luaL_getmetatable(state, LUA_MUX_CHANNEL_FLAG_METATABLE);
  lua_setmetatable(state, -2);
}

static LuaMuxChannelFlag *lua_mux_check_channel_flag(lua_State *state,
                                                     int argument,
                                                     LuaMuxPackage *package) {
  LuaMuxChannelFlag *flag =
      luaL_testudata(state, argument, LUA_MUX_CHANNEL_FLAG_METATABLE);

  if (flag == nullptr || flag->package != package)
    lua_error_arg(state, argument, LUA_ERROR_CODE_CHANNEL_FLAG_INVALID,
                  "expected a mux.comsys.flags constant");
  return flag;
}

static bool lua_mux_option_boolean(lua_State *state, int table,
                                   const char *field) {
  bool value = false;

  lua_getfield(state, table, field);
  if (!lua_isnil(state, -1)) {
    if (!lua_isboolean(state, -1))
      lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                    "options.%s must be a boolean", field);
    value = lua_toboolean(state, -1) != 0;
  }
  lua_pop(state, 1);
  return value;
}

/**
 * Retrieves an existing communication channel by name.
 *
 * @par Lua name `mux.comsys.channel`
 * @par Lua signature `mux.comsys.channel( name )`
 * @par Lua parameters - `name` (`string`) Existing channel name.
 * @par Lua returns - `channel` (`Channel`): A live channel handle.
 * @par Lua errors - `LUA_ERROR_CODE_CHANNEL_INVALID` for an unknown channel;
 * `LUA_ERROR_CODE_ARG_INVALID` for embedded NUL bytes;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * @par Lua availability Available only at runtime.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_comsys_channel(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  size_t length;
  const char *name = luaL_checklstring(state, 1, &length);

  lua_mux_require_runtime(package, state, "comsys.channel");
  if (strlen(name) != length)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "channel name contains an embedded NUL byte");
  struct Channel *channel =
      select_channel(lua_mux_channel_registry(package), name);
  if (channel == nullptr)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_CHANNEL_INVALID,
                         "unknown channel '%s'", name);
  lua_mux_push_channel(state, package, channel);
  return 1;
}

/**
 * Creates a communication channel using native channel-name rules.
 *
 * @par Lua name `mux.comsys.create_channel`
 * @par Lua signature `mux.comsys.create_channel( name )`
 * @par Lua parameters - `name` (`string`) Printable ASCII name without spaces.
 * @par Lua returns - `channel` (`Channel`): The created channel.
 * @par Lua errors - `LUA_ERROR_CODE_ARG_INVALID` for an invalid name;
 * `LUA_ERROR_CODE_CHANNEL_INVALID` when the channel already exists;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * @par Lua availability Available only at runtime.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_comsys_create_channel(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  size_t length;
  const char *name = luaL_checklstring(state, 1, &length);
  struct Channel *channel = nullptr;

  lua_mux_require_runtime(package, state, "comsys.create_channel");
  if (strlen(name) != length)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "channel name contains an embedded NUL byte");
  ChannelCreateResult result =
      comsys_channel_create(lua_mux_channel_registry(package), name, &channel);
  if (result == CHANNEL_CREATE_ALREADY_EXISTS)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_CHANNEL_INVALID,
                         "channel '%s' already exists", name);
  if (result != CHANNEL_CREATE_OK)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "channel name must be printable ASCII without spaces "
                         "and shorter than %d bytes",
                         CHAN_NAME_LEN);
  lua_mux_push_channel(state, package, channel);
  return 1;
}

/**
 * Permanently destroys a communication channel.
 *
 * @par Lua name `mux.comsys.destroy_channel`
 * @par Lua signature `mux.comsys.destroy_channel( channel )`
 * @par Lua parameters - `channel` (`Channel`) Live channel handle.
 * @par Lua returns - None.
 * @par Lua errors - `LUA_ERROR_CODE_CHANNEL_INVALID` for a stale handle;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * @par Lua availability Available only at runtime.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_comsys_destroy_channel(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  if (!comsys_channel_destroy(lua_mux_channel_registry(handle->package),
                              handle->identity))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_CHANNEL_INVALID,
                         "channel no longer exists");
  return 0;
}

// NOLINTNEXTLINE(bugprone-easily-swappable-parameters)
static int lua_mux_channel_compare(const void *left, const void *right) {
  const struct Channel *left_channel = *(const struct Channel *const *)left;
  const struct Channel *right_channel = *(const struct Channel *const *)right;
  int folded = strcasecmp(left_channel->name, right_channel->name);

  return folded != 0 ? folded : strcmp(left_channel->name, right_channel->name);
}

/**
 * Lists every communication channel in deterministic name order.
 *
 * @par Lua name `mux.comsys.list_channels`
 * @par Lua signature `mux.comsys.list_channels( )`
 * @par Lua parameters - None.
 * @par Lua returns - `channels` (`table`): Array of Channel handles.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * @par Lua availability Available only at runtime.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_comsys_list_channels(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  ChannelRegistry *registry;
  struct Channel **channels;
  size_t count = 0;

  lua_mux_require_runtime(package, state, "comsys.list_channels");
  registry = lua_mux_channel_registry(package);
  if (registry->count < 0)
    return lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                           "channel registry count is inconsistent");
  channels = (struct Channel **)checked_storage_allocate_array(
      (size_t)registry->count, sizeof(*channels));
  for (struct Channel *channel = hash_table_first_entry(&registry->channels);
       channel != nullptr;
       channel = hash_table_next_entry(&registry->channels)) {
    if (count >= (size_t)registry->count) {
      free((void *)channels);
      return lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                             "channel registry count is inconsistent");
    }
    *(struct Channel **)checked_storage_at(
        (void *)channels, (size_t)registry->count, sizeof(*channels), count++) =
        channel;
  }
  if (count > 1)
    qsort((void *)channels, count, sizeof(*channels), lua_mux_channel_compare);
  lua_newtable(state);
  for (size_t index = 0; index < count; index++) {
    struct Channel *channel =
        *(struct Channel *const *)checked_storage_at_const(
            (const void *)channels, count, sizeof(*channels), index);

    lua_mux_push_channel(state, package, channel);
    lua_rawseti(state, -2, (int)index + 1);
  }
  free((void *)channels);
  return 1;
}

/** Returns the channel name. Lua signature `channel:name( ) -> string`. */
static int lua_mux_channel_name(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  lua_pushstring(state, handle->name);
  return 1;
}

/** Returns the attached Object or nil. Lua signature `channel:object( )`. */
static int lua_mux_channel_object(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);
  struct Channel *channel = handle->identity;

  if (channel->chan_obj == NOTHING) {
    lua_pushnil(state);
    return 1;
  }
  if (!is_good_obj(handle->package->services->database, channel->chan_obj))
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_INVALID,
                           "attached channel object no longer exists");
  lua_mux_push_object(state, handle->package, channel->chan_obj);
  return 1;
}

/** Returns the member count. Lua signature `channel:user_count( )`. */
static int lua_mux_channel_user_count(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  lua_pushinteger(state, handle->identity->num_users);
  return 1;
}

/** Returns the allocated member capacity. Lua signature
 * `channel:max_user_count( )`. */
static int lua_mux_channel_max_user_count(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  lua_pushinteger(state, handle->identity->max_users);
  return 1;
}

/** Returns the lifetime message count. Lua signature `channel:message_count(
 * )`. */
static int lua_mux_channel_message_count(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  lua_pushinteger(state, handle->identity->num_messages);
  return 1;
}

/**
 * Attaches or detaches the object supplying channel locks and description.
 *
 * @par Lua name `Channel:set_object`
 * @par Lua signature `channel:set_object( object )`
 * @par Lua parameters - `object` (`number|Object|nil`) Live object or nil.
 * @par Lua returns - None.
 * @par Lua errors - Channel/object validation errors; unavailable while
 * checking.
 * @par Lua availability Available only at runtime.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_channel_set_object(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  if (lua_isnone(state, 2))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "expected an object or nil");
  if (lua_isnil(state, 2)) {
    handle->identity->chan_obj = NOTHING;
    return 0;
  }
  DbRef object = lua_mux_require_object(handle->package, state, 2);
  if (is_going(handle->package->services->database, object))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is being destroyed");
  handle->identity->chan_obj = (int)object;
  return 0;
}

/** Opens the channel flag collection. Lua signature `channel:flags( )`. */
static int lua_mux_channel_flags(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);
  LuaMuxChannelFlags *flags = lua_newuserdata(state, sizeof(*flags));

  *flags = (LuaMuxChannelFlags){
      .package = handle->package,
      .identity = handle->identity,
      .generation = handle->generation,
  };
  (void)string_copy_bounded(flags->name, sizeof(flags->name), handle->name);
  luaL_getmetatable(state, LUA_MUX_CHANNEL_FLAGS_METATABLE);
  lua_setmetatable(state, -2);
  return 1;
}

/**
 * Emits an administrative channel message.
 *
 * @par Lua name `Channel:emit`
 * @par Lua signature `channel:emit( message, options? )`
 * @par Lua parameters - `message` (`string`) Message to deliver.
 * - `options` (`EmitOptions|nil`) Supports `no_header`.
 * @par Lua returns - None.
 * @par Lua errors - Argument/channel errors; unavailable while checking.
 * @par Lua availability Available only at runtime.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_channel_emit(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);
  static const char *const FIELDS[] = {"no_header"};
  size_t length;
  const char *message = luaL_checklstring(state, 2, &length);
  bool no_header = false;

  if (strlen(message) != length)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "message contains an embedded NUL byte");
  if (!utf8_validate(message, length))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "message is not valid UTF-8");
  if (!lua_isnoneornil(state, 3)) {
    lua_mux_check_options(state, 3, FIELDS, sizeof(FIELDS) / sizeof(*FIELDS));
    no_header = lua_mux_option_boolean(state, 3, "no_header");
  }
  EvaluationContext *evaluation =
      &handle->package->services->background_command->evaluation;
  if (no_header)
    comsys_send_channel_message(evaluation, handle->identity, message);
  else
    comsys_channel_printf(evaluation, handle->identity, "[%s] %s", handle->name,
                          message);
  return 0;
}

/**
 * Returns structured channel membership records.
 *
 * @par Lua name `Channel:who`
 * @par Lua signature `channel:who( options? )`
 * @par Lua parameters - `options` (`WhoOptions|nil`) Supports `all`.
 * @par Lua returns - `members` (`table`): `{object, listening}` records.
 * @par Lua errors - Argument/channel errors; unavailable while checking.
 * @par Lua availability Available only at runtime.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_channel_who(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);
  static const char *const FIELDS[] = {"all"};
  bool all = false;
  int output = 1;

  if (!lua_isnoneornil(state, 2)) {
    lua_mux_check_options(state, 2, FIELDS, sizeof(FIELDS) / sizeof(*FIELDS));
    all = lua_mux_option_boolean(state, 2, "all");
  }
  lua_newtable(state);
  for (int index = 0; index < handle->identity->num_users; index++) {
    struct Comuser *member = channel_user_at(handle->identity, (size_t)index);
    if (!all && !is_undead(handle->package->services->database, member->who))
      continue;
    if (!is_good_obj(handle->package->services->database, member->who))
      continue;
    lua_newtable(state);
    lua_mux_push_object(state, handle->package, member->who);
    lua_setfield(state, -2, "object");
    lua_pushboolean(state, member->on != 0);
    lua_setfield(state, -2, "listening");
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

/**
 * Boots an object from a channel using native alias-removal side effects.
 *
 * @par Lua name `Channel:boot_player`
 * @par Lua signature `channel:boot_player( object )`
 * @par Lua parameters - `object` (`number|Object`) Current member.
 * @par Lua returns - None.
 * @par Lua errors - Object/channel validation errors; unavailable while
 * checking.
 * @par Lua availability Available only at runtime.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_channel_boot_player(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);
  DbRef target = lua_mux_require_object(handle->package, state, 2);

  if (select_user(handle->identity, target) == nullptr)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_CHANNEL_INVALID,
                         "object is not a member of this channel");
  EvaluationContext *evaluation =
      &handle->package->services->background_command->evaluation;
  OwnedText actor_name =
      unparse_object_numonly(handle->package->services->database, GOD);
  OwnedText target_name =
      unparse_object_numonly(handle->package->services->database, target);
  comsys_channel_printf(evaluation, handle->identity,
                        "[%s] %s boots %s off the channel.", handle->name,
                        actor_name.text, target_name.text);
  owned_text_release(&actor_name);
  owned_text_release(&target_name);
  comsys_delete_channel_alias(evaluation, target, handle->name);
  return 0;
}

static int lua_mux_channel_tostring(lua_State *state) {
  LuaMuxChannel *handle = luaL_checkudata(state, 1, LUA_MUX_CHANNEL_METATABLE);

  lua_pushfstring(state, "channel(%s)", handle->name);
  return 1;
}

static int lua_mux_channel_equal(lua_State *state) {
  LuaMuxChannel *left = luaL_checkudata(state, 1, LUA_MUX_CHANNEL_METATABLE);
  LuaMuxChannel *right = luaL_checkudata(state, 2, LUA_MUX_CHANNEL_METATABLE);

  lua_pushboolean(state, left->package == right->package &&
                             left->identity == right->identity &&
                             left->generation == right->generation);
  return 1;
}

static int lua_mux_channel_immutable(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "channel values are immutable");
}

static int lua_mux_channel_flag_immutable(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_CHANNEL_FLAG_INVALID,
                         "channel flag values are immutable");
}

static int lua_mux_channel_flags_list(lua_State *state) {
  LuaMuxChannelFlags *handle = lua_mux_check_channel_flags(state, 1);
  int output = 1;

  lua_newtable(state);
  for (size_t index = 0; index < sizeof(CHANNEL_FLAGS) / sizeof(*CHANNEL_FLAGS);
       index++) {
    const ChannelFlagDefinition *definition = lua_mux_channel_flag_at(index);

    if ((handle->identity->type & definition->value) == 0)
      continue;
    lua_mux_push_channel_flag(state, handle->package, definition);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

static int lua_mux_channel_flags_has(lua_State *state) {
  LuaMuxChannelFlags *handle = lua_mux_check_channel_flags(state, 1);
  LuaMuxChannelFlag *flag =
      lua_mux_check_channel_flag(state, 2, handle->package);

  lua_pushboolean(state, (handle->identity->type & flag->value) != 0);
  return 1;
}

static int lua_mux_channel_flags_change(lua_State *state, bool enabled) {
  LuaMuxChannelFlags *handle = lua_mux_check_channel_flags(state, 1);
  LuaMuxChannelFlag *flag =
      lua_mux_check_channel_flag(state, 2, handle->package);
  bool current = (handle->identity->type & flag->value) != 0;

  if (current == enabled) {
    lua_pushboolean(state, false);
    return 1;
  }
  if (enabled)
    handle->identity->type |= flag->value;
  else
    handle->identity->type &= ~flag->value;
  lua_pushboolean(state, true);
  return 1;
}

static int lua_mux_channel_flags_add(lua_State *state) {
  return lua_mux_channel_flags_change(state, true);
}

static int lua_mux_channel_flags_remove(lua_State *state) {
  return lua_mux_channel_flags_change(state, false);
}

static int lua_mux_channel_flags_tostring(lua_State *state) {
  LuaMuxChannelFlags *handle = lua_mux_check_channel_flags(state, 1);

  lua_pushfstring(state, "channel_flags(%s)", handle->name);
  return 1;
}

static int lua_mux_channel_flag_tostring(lua_State *state) {
  LuaMuxChannelFlag *flag =
      luaL_checkudata(state, 1, LUA_MUX_CHANNEL_FLAG_METATABLE);

  lua_pushstring(state, flag->name);
  return 1;
}

static int lua_mux_channel_flag_equal(lua_State *state) {
  LuaMuxChannelFlag *left =
      luaL_checkudata(state, 1, LUA_MUX_CHANNEL_FLAG_METATABLE);
  LuaMuxChannelFlag *right =
      luaL_checkudata(state, 2, LUA_MUX_CHANNEL_FLAG_METATABLE);

  lua_pushboolean(state, left->package == right->package &&
                             left->value == right->value);
  return 1;
}

static int lua_mux_channel_flag_namespace_index(lua_State *state) {
  LuaMuxChannelFlagNamespace *name_space =
      luaL_checkudata(state, 1, LUA_MUX_CHANNEL_FLAG_NAMESPACE_METATABLE);
  const char *name = lua_tostring(state, 2);

  if (name == nullptr)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_CHANNEL_FLAG_INVALID,
                         "channel flag name must be a string");
  for (size_t index = 0; index < sizeof(CHANNEL_FLAGS) / sizeof(*CHANNEL_FLAGS);
       index++) {
    const ChannelFlagDefinition *definition = lua_mux_channel_flag_at(index);

    if (strcmp(name, definition->name) == 0) {
      lua_mux_push_channel_flag(state, name_space->package, definition);
      return 1;
    }
  }
  return lua_error_arg(state, 2, LUA_ERROR_CODE_CHANNEL_FLAG_INVALID,
                       "unknown channel flag constant '%s'", name);
}

static void lua_mux_install_channel_metatable(lua_State *state,
                                              LuaMuxPackage *package) {
  luaL_newmetatable(state, LUA_MUX_CHANNEL_METATABLE);
  lua_pushvalue(state, -1);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_channel_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_channel_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushcfunction(state, lua_mux_channel_immutable);
  lua_setfield(state, -2, "__newindex");
  const LuaMuxChannelMethod METHODS[] = {
      {"name", lua_mux_channel_name},
      {"object", lua_mux_channel_object},
      {"user_count", lua_mux_channel_user_count},
      {"max_user_count", lua_mux_channel_max_user_count},
      {"message_count", lua_mux_channel_message_count},
      {"set_object", lua_mux_channel_set_object},
      {"flags", lua_mux_channel_flags},
      {"emit", lua_mux_channel_emit},
      {"who", lua_mux_channel_who},
      {"boot_player", lua_mux_channel_boot_player},
  };
  for (size_t index = 0; index < sizeof(METHODS) / sizeof(*METHODS); index++) {
    const LuaMuxChannelMethod *method = checked_storage_at_const(
        METHODS, sizeof(METHODS) / sizeof(*METHODS), sizeof(*METHODS), index);

    lua_pushlightuserdata(state, package);
    lua_pushcclosure(state, method->function, 1);
    lua_setfield(state, -2, method->name);
  }
  lua_pop(state, 1);
}

static void lua_mux_install_channel_flag_metatables(lua_State *state) {
  luaL_newmetatable(state, LUA_MUX_CHANNEL_FLAGS_METATABLE);
  lua_pushvalue(state, -1);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_channel_flags_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_channel_flags_list);
  lua_setfield(state, -2, "list");
  lua_pushcfunction(state, lua_mux_channel_flags_has);
  lua_setfield(state, -2, "has");
  lua_pushcfunction(state, lua_mux_channel_flags_add);
  lua_setfield(state, -2, "add");
  lua_pushcfunction(state, lua_mux_channel_flags_remove);
  lua_setfield(state, -2, "remove");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_MUX_CHANNEL_FLAG_METATABLE);
  lua_pushcfunction(state, lua_mux_channel_flag_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_channel_flag_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushcfunction(state, lua_mux_channel_flag_immutable);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_MUX_CHANNEL_FLAG_NAMESPACE_METATABLE);
  lua_pushcfunction(state, lua_mux_channel_flag_namespace_index);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_channel_flag_immutable);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);
}

/** Installs the trusted `mux.comsys` channel API. */
void lua_mux_install_comsys_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_mux_install_channel_metatable(state, package);
  lua_mux_install_channel_flag_metatables(state);
  lua_newtable(state);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_comsys_channel, 1);
  lua_setfield(state, -2, "channel");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_comsys_create_channel, 1);
  lua_setfield(state, -2, "create_channel");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_comsys_destroy_channel, 1);
  lua_setfield(state, -2, "destroy_channel");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_comsys_list_channels, 1);
  lua_setfield(state, -2, "list_channels");
  LuaMuxChannelFlagNamespace *name_space =
      lua_newuserdata(state, sizeof(*name_space));
  *name_space = (LuaMuxChannelFlagNamespace){.package = package};
  luaL_getmetatable(state, LUA_MUX_CHANNEL_FLAG_NAMESPACE_METATABLE);
  lua_setmetatable(state, -2);
  lua_setfield(state, -2, "flags");
  lua_setfield(state, -2, "comsys");
}
