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
#include "mux/lua/packages/mux/comsys/mux_comsys_bindings_internal.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/owned_text.h"
#include "mux/support/utf8.h"

const char LUA_MUX_CHANNEL_METATABLE[] = "btmux.channel";

typedef struct LuaMuxChannelMethod LuaMuxChannelMethod;
struct LuaMuxChannelMethod {
  const char *name;
  lua_CFunction function;
};

static ChannelRegistry *lua_mux_channel_registry(LuaMuxPackage *package) {
  return package->services->background_command->evaluation.runtime->channels;
}

struct Channel *lua_mux_check_channel_identity(LuaMuxPackage *package,
                                               lua_State *state, int argument,
                                               const char *name,
                                               struct Channel *identity,
                                               uint64_t generation) {
  struct Channel *current =
      select_channel(lua_mux_channel_registry(package), name);

  if (current == nullptr || current != identity ||
      current->generation != generation)
    lua_error_arg(state, argument, LUA_ERROR_CODE_CHANNEL_INVALID,
                  "channel no longer exists");
  return current;
}

LuaMuxChannel *lua_mux_check_channel(lua_State *state, int argument) {
  LuaMuxChannel *handle =
      luaL_checkudata(state, argument, LUA_MUX_CHANNEL_METATABLE);

  lua_mux_require_runtime(handle->package, state, "comsys.Channel");
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
 * @par LuaLS definition mux callable mux.comsys.channel
 * @code{.lua}
 * ---Retrieves an existing communication channel by case-insensitive name.
 * ---@param name string Existing channel name without embedded NUL bytes; the returned handle preserves canonical spelling.
 * ---@return Channel channel
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.channel.invalid
 * function mux_comsys.channel(name) end
 * @endcode
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
 * @par LuaLS definition mux callable mux.comsys.create_channel
 * @code{.lua}
 * ---Creates a private communication channel using the native channel-name
 * ---rules. Names must be non-empty printable ASCII, contain no spaces, and be
 * ---shorter than 50 bytes.
 * ---@param name string New channel name.
 * ---@return Channel channel
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid) when the name already exists.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.channel.invalid
 * function mux_comsys.create_channel(name) end
 * @endcode
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
 * @par LuaLS definition mux callable mux.comsys.destroy_channel
 * @code{.lua}
 * ---Permanently removes a live channel and its membership storage. The supplied
 * ---handle and every flag handle derived from it become stale.
 * ---@param channel Channel Live channel handle.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * function mux_comsys.destroy_channel(channel) end
 * @endcode
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
 * @par LuaLS definition mux callable mux.comsys.list_channels
 * @code{.lua}
 * ---Lists every live communication channel in case-insensitive name order, with
 * ---original spelling used as the tie-breaker.
 * ---@return Channel[] channels
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
 * ---or [`mux.error.codes.internal`](lua://mux.error.codes.internal) if the native
 * ---registry count changes while it is copied.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.internal
 * function mux_comsys.list_channels() end
 * @endcode
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

/**
 * @par LuaLS definition mux callable Channel:name
 * @code{.lua}
 * ---Returns the channel's exact name.
 * ---@return string name
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * function Channel:name() end
 * @endcode
 */
static int lua_mux_channel_name(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  lua_pushstring(state, handle->name);
  return 1;
}

/**
 * @par LuaLS definition mux callable Channel:object
 * @code{.lua}
 * ---Returns the object that supplies the channel description and locks.
 * ---@return Object? object The attached object, or nil when none is attached.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * ---@see mux.error.codes.object.invalid
 * function Channel:object() end
 * @endcode
 */
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

/**
 * @par LuaLS definition mux callable Channel:user_count
 * @code{.lua}
 * ---Returns the number of channel membership records.
 * ---@return integer count
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * function Channel:user_count() end
 * @endcode
 */
static int lua_mux_channel_user_count(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  lua_pushinteger(state, handle->identity->num_users);
  return 1;
}

/**
 * @par LuaLS definition mux callable Channel:max_user_count
 * @code{.lua}
 * ---Returns the channel's currently allocated membership capacity.
 * ---@return integer count
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * function Channel:max_user_count() end
 * @endcode
 */
static int lua_mux_channel_max_user_count(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  lua_pushinteger(state, handle->identity->max_users);
  return 1;
}

/**
 * @par LuaLS definition mux callable Channel:message_count
 * @code{.lua}
 * ---Returns the channel's lifetime delivered-message count.
 * ---@return integer count
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * function Channel:message_count() end
 * @endcode
 */
static int lua_mux_channel_message_count(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);

  lua_pushinteger(state, handle->identity->num_messages);
  return 1;
}

/**
 * Attaches or detaches the object supplying channel locks and description.
 *
 * @par LuaLS definition mux callable Channel:set_object
 * @code{.lua}
 * ---Attaches an object that supplies channel locks and description, or detaches
 * ---the current object when passed nil.
 * ---@param object DbRef|Object|nil
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `object` is omitted, [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.channel.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function Channel:set_object(object) end
 * @endcode
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

/**
 * Emits an administrative channel message.
 *
 * @par LuaLS definition mux callable Channel:emit
 * @code{.lua}
 * ---Emits an administrative channel message through native delivery, history,
 * ---receive-lock, and message-count behavior.
 * ---@param message string Valid UTF-8 without embedded NUL bytes.
 * ---@param options? ChannelEmitOptions Unknown option fields are rejected.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * ---@see mux.error.codes.arg.invalid
 * function Channel:emit(message, options) end
 * @endcode
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
 * @par LuaLS definition mux callable Channel:who
 * @code{.lua}
 * ---Returns channel membership records. By default the native active-member
 * ---filter is applied; `options.all` includes inactive records.
 * ---@param options? ChannelWhoOptions Unknown option fields are rejected.
 * ---@return ChannelMember[] members
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * ---@see mux.error.codes.arg.invalid
 * function Channel:who(options) end
 * @endcode
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
 * @par LuaLS definition mux callable Channel:boot_player
 * @code{.lua}
 * ---Announces a God-administered boot and removes a current member's channel
 * ---aliases using the native side-effect path.
 * ---@param object DbRef|Object Current channel member.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * ---@see mux.error.codes.object.invalid
 * function Channel:boot_player(object) end
 * @endcode
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

/**
 * @par LuaLS ignore mux __tostring -- String conversion is represented by the Channel class declaration.
 */
static int lua_mux_channel_tostring(lua_State *state) {
  LuaMuxChannel *handle = luaL_checkudata(state, 1, LUA_MUX_CHANNEL_METATABLE);

  lua_pushfstring(state, "channel(%s)", handle->name);
  return 1;
}

/**
 * @par LuaLS ignore mux __eq -- LuaCATS has no equality-operator declaration; Channel equality semantics are documented on the class.
 */
static int lua_mux_channel_equal(lua_State *state) {
  LuaMuxChannel *left = luaL_checkudata(state, 1, LUA_MUX_CHANNEL_METATABLE);
  LuaMuxChannel *right = luaL_checkudata(state, 2, LUA_MUX_CHANNEL_METATABLE);

  lua_pushboolean(state, left->package == right->package &&
                             left->identity == right->identity &&
                             left->generation == right->generation);
  return 1;
}

/**
 * @par LuaLS ignore mux __newindex -- Immutability is represented by the Channel class declaration.
 */
static int lua_mux_channel_immutable(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "channel values are immutable");
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
      {"emit", lua_mux_channel_emit},
      {"who", lua_mux_channel_who},
      {"add_player", lua_mux_channel_add_player},
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

/**
 * @par LuaLS definition mux type channel
 * @code{.lua}
 * ---Options for [`Channel:emit`](lua://Channel.emit).
 * ---@class (exact) ChannelEmitOptions
 * ---@field no_header? boolean Send the message without the usual `[channel]` prefix.
 *
 * ---Options for [`Channel:who`](lua://Channel.who).
 * ---@class (exact) ChannelWhoOptions
 * ---@field all? boolean Include inactive membership records.
 *
 * ---One communication-channel membership record.
 * ---@class ChannelMember
 * ---@field object Object Live member object.
 * ---@field listening boolean Whether the member is currently listening to the channel.
 *
 * ---A generation-sensitive handle to one live communication channel. Equality
 * ---requires the same package, native channel identity, and generation. Handles
 * ---remain stale after destruction even if a channel with the same name is
 * ---created later. Assigning fields raises
 * ---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
 * ---@see mux.error.codes.arg.invalid
 * ---@class Channel
 * local Channel = {}
 * @endcode
 *
 * @par LuaLS definition mux namespace mux.comsys
 * @code{.lua}
 * ---Trusted access to the live communication-channel registry. Mutations take
 * ---effect immediately and are not rolled back when the surrounding Lua
 * ---callback later fails.
 * ---@class MuxComsysPackage
 * ---@field flags ChannelFlagNamespace Immutable typed channel-flag constants.
 * local mux_comsys = {}
 * @endcode
 */
void lua_mux_install_comsys_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_mux_install_channel_metatable(state, package);
  lua_newtable(state);
  lua_mux_install_channel_flag_bindings(state, package);
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
  lua_setfield(state, -2, "comsys");
}
