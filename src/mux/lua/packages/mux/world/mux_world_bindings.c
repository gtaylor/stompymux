/* mux_world_bindings.c - Lua bindings for mux.world. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/commands/command_context.h"
#include "mux/lua/command_access.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/utf8.h"
#include "mux/world/move.h"
#include "mux/world/object.h"
#include "mux/world/object_lifecycle.h"

static const char *lua_mux_world_require_name(lua_State *state, int table,
                                              size_t *length) {
  const char *name;

  lua_getfield(state, table, "name");
  if (lua_type(state, -1) != LUA_TSTRING)
    lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                  "options.name must be a string");
  name = lua_tolstring(state, -1, length);
  if (strlen(name) != *length)
    lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                  "options.name contains an embedded NUL byte");
  if (!utf8_validate(name, *length))
    lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                  "options.name is not valid UTF-8");
  lua_pop(state, 1);
  return name;
}

static char *lua_mux_world_compile_name(LuaMuxPackage *package,
                                        lua_State *state, int table,
                                        const char *name) {
  char clear[LBUF_SIZE];
  char error[256];
  char *compiled = alloc_lbuf("lua_mux_world_compile_name");

  if (!styled_text_compile(package->services->styled_text_palette, name,
                           compiled, LBUF_SIZE, error, sizeof(error))) {
    free_buf(compiled);
    (void)lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                        "options.name has invalid styled-text markup: %s",
                        error);
    return nullptr;
  }
  if (!string_copy_bounded(compiled, LBUF_SIZE, name)) {
    free_buf(compiled);
    (void)lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                        "options.name is too long");
    return nullptr;
  }
  styled_text_strip(package->services->styled_text_palette, compiled, clear,
                    sizeof(clear));
  if (!*clear) {
    free_buf(compiled);
    (void)lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                        "options.name must not be empty");
    return nullptr;
  }
  return compiled;
}

static void lua_mux_world_require_container(LuaMuxPackage *package,
                                            lua_State *state, int argument,
                                            const char *label, DbRef object) {
  if (!has_contents(package->services->database, object))
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "%s must be an object that can contain objects", label);
  if (is_going(package->services->database, object))
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                  "%s is being destroyed", label);
}

static void lua_mux_world_require_zone(LuaMuxPackage *package, lua_State *state,
                                       int argument, const char *label,
                                       DbRef zone) {
  GameDatabase *database = package->services->database;

  if (is_going(database, zone))
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                  "%s is being destroyed", label);
  if (!is_thing(database, zone) && !is_room(database, zone))
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "%s must be a thing or room", label);
}

typedef struct LuaMuxWorldContainmentCheck {
  GameDatabase *database;
  DbRef object;
  DbRef destination;
} LuaMuxWorldContainmentCheck;

static bool
lua_mux_world_destination_is_within(const LuaMuxWorldContainmentCheck *check) {
  GameDatabase *database = check->database;
  DbRef current = check->destination;

  for (DbRef depth = 0; depth < database->top; depth++) {
    if (current == check->object)
      return true;
    if (!is_good_obj(database, current) || !has_location(database, current))
      return false;
    current = game_object_location(database, current);
  }
  return true;
}

/**
 * Lists database objects, optionally filtered by type and assigned zone.
 *
 * @par LuaLS definition mux callable mux.world.list_objects
 * @code{.lua}
 * ---Lists database objects matching optional type and direct-zone filters.
 * ---@param options? ListObjectsOptions Optional filters; unknown fields are rejected.
 * ---@return Object[] objects Matching objects in ascending dbref order.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function mux_world.list_objects(options) end
 * @endcode
 */
static int lua_mux_list_objects(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;
  static const char *const FIELDS[] = {"types", "in_zone"};
  LuaMuxObjectTypeFilter filter = {
      .enabled = false,
      .rooms = false,
      .things = false,
      .exits = false,
      .players = false,
  };
  DbRef zone = NOTHING;
  bool filter_zone = false;
  int result_index = 1;

  lua_mux_require_runtime(package, state, "world.list_objects");
  if (!lua_isnoneornil(state, 1)) {
    lua_mux_check_options(state, 1, FIELDS, sizeof(FIELDS) / sizeof(*FIELDS));
    lua_mux_object_type_filter_parse(package, state, 1, &filter);
    zone = lua_mux_option_object(package, state, 1, "in_zone", false,
                                 &filter_zone);
    if (filter_zone && is_going(database, zone))
      return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                           "options.in_zone is being destroyed");
  }

  lua_newtable(state);
  for (DbRef object = 0; object < database->top; object++) {
    if (!is_good_obj(database, object) ||
        typeof_obj(database, object) == OBJECT_TYPE_GARBAGE)
      continue;
    if (!lua_mux_object_type_filter_matches(&filter,
                                            typeof_obj(database, object)))
      continue;
    if (filter_zone && game_object_zone(database, object) != zone)
      continue;
    lua_mux_push_object(state, package, object);
    lua_rawseti(state, -2, result_index++);
  }
  return 1;
}

typedef struct LuaMuxCreateObjectOptions {
  int type;
  DbRef location;
  DbRef home;
  DbRef destination;
  DbRef zone;
  bool destination_present;
  bool zone_present;
} LuaMuxCreateObjectOptions;

static int lua_mux_world_create_object_type(LuaMuxPackage *package,
                                            lua_State *state) {
  if (!lua_istable(state, 1))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "options must be a table");

  lua_getfield(state, 1, "type");
  if (lua_isnil(state, -1))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "options.type is required");
  int type =
      lua_mux_require_object_type_at(package, state, -1, 1, "options.type");
  lua_pop(state, 1);
  if (type != OBJECT_TYPE_ROOM && type != OBJECT_TYPE_THING &&
      type != OBJECT_TYPE_EXIT)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "options.type must be ROOM, THING, or EXIT");
  return type;
}

static void lua_mux_world_check_create_object_fields(lua_State *state,
                                                     int type) {
  static const char *const ROOM_FIELDS[] = {"type", "name", "zone"};
  static const char *const THING_FIELDS[] = {"type", "name", "location", "home",
                                             "zone"};
  static const char *const EXIT_FIELDS[] = {"type", "name", "location",
                                            "destination", "zone"};

  switch (type) {
  case OBJECT_TYPE_ROOM:
    lua_mux_check_options(state, 1, ROOM_FIELDS,
                          sizeof(ROOM_FIELDS) / sizeof(*ROOM_FIELDS));
    break;
  case OBJECT_TYPE_THING:
    lua_mux_check_options(state, 1, THING_FIELDS,
                          sizeof(THING_FIELDS) / sizeof(*THING_FIELDS));
    break;
  case OBJECT_TYPE_EXIT:
    lua_mux_check_options(state, 1, EXIT_FIELDS,
                          sizeof(EXIT_FIELDS) / sizeof(*EXIT_FIELDS));
    break;
  default:
    (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                          "unsupported object type reached creation parser");
  }
}

static void lua_mux_world_parse_create_object_references(
    LuaMuxPackage *package, lua_State *state,
    LuaMuxCreateObjectOptions *options) {
  switch (options->type) {
  case OBJECT_TYPE_ROOM:
    break;
  case OBJECT_TYPE_THING: {
    bool location_present;
    bool home_present;

    options->location = lua_mux_option_object(package, state, 1, "location",
                                              true, &location_present);
    options->home =
        lua_mux_option_object(package, state, 1, "home", false, &home_present);
    lua_mux_world_require_container(package, state, 1, "options.location",
                                    options->location);
    if (home_present)
      lua_mux_world_require_container(package, state, 1, "options.home",
                                      options->home);
    else
      options->home = options->location;
    (void)location_present;
    break;
  }
  case OBJECT_TYPE_EXIT: {
    bool location_present;

    options->location = lua_mux_option_object(package, state, 1, "location",
                                              true, &location_present);
    options->destination = lua_mux_option_object(
        package, state, 1, "destination", false, &options->destination_present);
    if (!has_exits(package->services->database, options->location))
      (void)lua_error_arg(
          state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
          "options.location must be an object that can have exits");
    if (is_going(package->services->database, options->location))
      (void)lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                          "options.location is being destroyed");
    if (options->destination_present)
      lua_mux_world_require_container(package, state, 1, "options.destination",
                                      options->destination);
    (void)location_present;
    break;
  }
  default:
    (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                          "unsupported object type reached reference parser");
  }

  options->zone = lua_mux_option_object(package, state, 1, "zone", false,
                                        &options->zone_present);
  if (options->zone_present)
    lua_mux_world_require_zone(package, state, 1, "options.zone",
                               options->zone);
}

static const char *lua_mux_world_object_type_name(int type) {
  switch (type) {
  case OBJECT_TYPE_ROOM:
    return "room";
  case OBJECT_TYPE_THING:
    return "thing";
  case OBJECT_TYPE_EXIT:
    return "exit";
  default:
    return "object";
  }
}

static void
lua_mux_world_finish_create_object(LuaMuxPackage *package,
                                   const LuaMuxCreateObjectOptions *options,
                                   DbRef object) {
  GameDatabase *database = package->services->database;
  EvaluationContext *evaluation =
      &package->services->background_command->evaluation;

  if (options->zone_present)
    game_object_set_zone(database, object, options->zone);
  switch (options->type) {
  case OBJECT_TYPE_THING:
    game_object_set_link(database, object, options->home);
    move_via_generic(&(ObjectMovementRequest){.evaluation = evaluation,
                                              .object = object,
                                              .destination = options->location,
                                              .cause = GOD});
    break;
  case OBJECT_TYPE_EXIT:
    game_object_set_exits(database, object, options->location);
    game_object_set_next(database, object,
                         game_object_exits(database, options->location));
    game_object_set_exits(database, options->location, object);
    if (options->destination_present)
      game_object_set_location(database, object, options->destination);
    break;
  default:
    break;
  }
}

/**
 * Creates a room, thing, or exit selected by a typed object-kind constant.
 *
 * @par LuaLS definition mux callable mux.world.create_object
 * @code{.lua}
 * ---Creates a room, thing, or exit selected by a typed object-kind constant.
 * ---Rooms are detached; things require a container and receive a home; exits
 * ---require a source and may be linked to a destination. Unknown fields and
 * ---fields that do not apply to the selected type are rejected.
 * ---@param options CreateObjectOptions Exact creation fields selected by `options.type`.
 * ---@return Object object Newly created object.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable), or [`mux.error.codes.internal`](lua://mux.error.codes.internal) if a validated object kind reaches an unsupported native creation branch.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * ---@see mux.error.codes.internal
 * function mux_world.create_object(options) end
 * @endcode
 */
static int lua_mux_create_object(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  LuaMuxCreateObjectOptions options = {
      .type = 0,
      .location = NOTHING,
      .home = NOTHING,
      .destination = NOTHING,
      .zone = NOTHING,
      .destination_present = false,
      .zone_present = false,
  };
  size_t name_length;

  lua_mux_require_runtime(package, state, "world.create_object");
  options.type = lua_mux_world_create_object_type(package, state);
  lua_mux_world_check_create_object_fields(state, options.type);
  const char *name = lua_mux_world_require_name(state, 1, &name_length);
  lua_mux_world_parse_create_object_references(package, state, &options);
  char *compiled = lua_mux_world_compile_name(package, state, 1, name);
  DbRef object = create_obj(&package->services->background_command->evaluation,
                            GOD, options.type, compiled);
  free_buf(compiled);
  if (object == NOTHING)
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                           "%s creation failed",
                           lua_mux_world_object_type_name(options.type));

  lua_mux_world_finish_create_object(package, &options, object);
  lua_mux_push_object(state, package, object);
  return 1;
}

/**
 * Teleports a location-bearing object to a destination container.
 *
 * @par LuaLS definition mux callable mux.world.teleport_object
 * @code{.lua}
 * ---Teleports a thing or player through the native movement path.
 * ---@param options TeleportOptions Teleport fields; unknown fields are rejected.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function mux_world.teleport_object(options) end
 * @endcode
 */
static int lua_mux_teleport_object(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  static const char *const FIELDS[] = {"object", "destination"};
  bool object_present;
  bool destination_present;

  lua_mux_require_runtime(package, state, "world.teleport_object");
  lua_mux_check_options(state, 1, FIELDS, sizeof(FIELDS) / sizeof(*FIELDS));
  DbRef object =
      lua_mux_option_object(package, state, 1, "object", true, &object_present);
  DbRef destination = lua_mux_option_object(package, state, 1, "destination",
                                            true, &destination_present);
  (void)object_present;
  (void)destination_present;

  if (!has_location(package->services->database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "options.object must be a thing or player");
  if (is_going(package->services->database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "options.object is being destroyed");
  lua_mux_world_require_container(package, state, 1, "options.destination",
                                  destination);
  if (lua_mux_world_destination_is_within(&(LuaMuxWorldContainmentCheck){
          .database = package->services->database,
          .object = object,
          .destination = destination,
      }))
    return lua_error_arg(
        state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
        "options.destination must not be inside options.object");

  if (!move_via_teleport(&(ObjectMovementRequest){
          .evaluation = &package->services->background_command->evaluation,
          .object = object,
          .destination = destination,
          .cause = GOD,
      }))
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                           "teleport was denied");
  return 0;
}

/**
 * Silently schedules an object for destruction.
 *
 * @par LuaLS definition mux callable mux.world.destroy_object
 * @code{.lua}
 * ---Silently schedules a live object for destruction by the normal maintenance purge.
 * ---@param object DbRef|Object Object to destroy.
 * ---@param options? DestroyOptions Destruction controls; unknown fields are rejected.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable), or [`mux.error.codes.internal`](lua://mux.error.codes.internal) for an unexpected native destruction result.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * ---@see mux.error.codes.internal
 * function mux_world.destroy_object(object, options) end
 * @endcode
 */
static int lua_mux_destroy_object(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  static const char *const FIELDS[] = {"override"};
  bool override_safe = false;

  lua_mux_require_runtime(package, state, "world.destroy_object");
  DbRef object = lua_mux_require_object(package, state, 1);
  if (!lua_isnoneornil(state, 2)) {
    lua_mux_check_options(state, 2, FIELDS, sizeof(FIELDS) / sizeof(*FIELDS));
    lua_getfield(state, 2, "override");
    if (!lua_isnil(state, -1)) {
      if (!lua_isboolean(state, -1))
        return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                             "options.override must be a boolean");
      override_safe = lua_toboolean(state, -1) != 0;
    }
    lua_pop(state, 1);
  }

  ObjectDestroyStatus status =
      object_destroy_schedule(&(ObjectDestroyScheduleRequest){
          .evaluation = &package->services->background_command->evaluation,
          .actor = GOD,
          .object = object,
          .override_safe = override_safe,
      });
  switch (status) {
  case OBJECT_DESTROY_SCHEDULED:
    return 0;
  case OBJECT_DESTROY_ALREADY_GOING:
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is already being destroyed");
  case OBJECT_DESTROY_SAFE:
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is SAFE; pass override=true to destroy it");
  case OBJECT_DESTROY_PROTECTED:
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is protected from destruction");
  case OBJECT_DESTROY_PLAYER_PERMISSION:
  case OBJECT_DESTROY_WIZARD_PLAYER:
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "Wizard players are protected from destruction");
  }
  return lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                         "unexpected object destruction result");
}

/**
 * Privately emits a message to an object.
 *
 * @par LuaLS definition mux callable mux.world.pemit
 * @code{.lua}
 * ---Sends valid UTF-8 text to an object.
 * ---@param object DbRef|Object
 * ---@param message string
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.connection.invalid
 * ---@see mux.error.codes.object.invalid
 * function mux_world.pemit(object, message) end
 * @endcode
 */
static int lua_mux_pemit(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  size_t length;
  const char *message = luaL_checklstring(state, 2, &length);

  if (lua_mux_package_is_checking(package))
    return lua_error_raise(state, LUA_ERROR_CODE_CHECKING_UNAVAILABLE,
                           "mux.world.pemit is unavailable during @lua/check");
  if (strlen(message) != length)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_CONNECTION_INVALID,
                         "message contains an embedded NUL byte");
  if (!utf8_validate(message, length))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_CONNECTION_INVALID,
                         "message is not valid UTF-8");
  object = lua_mux_require_object(package, state, 1);
  notify_checked(&package->services->background_command->evaluation, object,
                 object, message, MSG_ME_ALL | MSG_F_DOWN);
  return 0;
}

/**
 * @par LuaLS definition mux type world.create_options
 * @code{.lua}
 * ---Fields accepted when creating a detached room through
 * ---[`mux.world.create_object`](lua://mux.world.create_object).
 * ---@class (exact) CreateRoomOptions
 * ---@field type RoomObjectType The current runtime's [`mux.world.types.ROOM`](lua://mux.world.types.ROOM) constant.
 * ---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
 * ---@field zone? DbRef|Object Live thing or room to assign; omission preserves the native creator's inherited zone.
 *
 * ---Fields accepted when creating and placing a thing through
 * ---[`mux.world.create_object`](lua://mux.world.create_object).
 * ---@class (exact) CreateThingOptions
 * ---@field type ThingObjectType The current runtime's [`mux.world.types.THING`](lua://mux.world.types.THING) constant.
 * ---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
 * ---@field location DbRef|Object Required object that can contain the new thing.
 * ---@field home? DbRef|Object Home object; defaults to `location` when omitted.
 * ---@field zone? DbRef|Object Live thing or room to assign; omission preserves the native creator's inherited zone.
 *
 * ---Fields accepted when creating and attaching an exit through
 * ---[`mux.world.create_object`](lua://mux.world.create_object).
 * ---@class (exact) CreateExitOptions
 * ---@field type ExitObjectType The current runtime's [`mux.world.types.EXIT`](lua://mux.world.types.EXIT) constant.
 * ---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
 * ---@field location DbRef|Object Required source object capable of holding exits.
 * ---@field destination? DbRef|Object Optional destination capable of containing objects; omission leaves the exit unlinked.
 * ---@field zone? DbRef|Object Live thing or room to assign; omission preserves the native creator's inherited zone.
 * @endcode
 *
 * @par LuaLS definition mux alias world.create_options.union
 * @code{.lua}
 * ---Exact fields accepted by [`mux.world.create_object`](lua://mux.world.create_object).
 * ---The selected typed object-kind constant determines which other fields apply.
 * ---@alias CreateObjectOptions CreateRoomOptions|CreateThingOptions|CreateExitOptions
 * @endcode
 *
 * @par LuaLS definition mux type world.options
 * @code{.lua}
 * ---Fields accepted when teleporting a thing or player.
 * ---@class (exact) TeleportOptions
 * ---@field object DbRef|Object Required thing or player to move.
 * ---@field destination DbRef|Object Required object capable of containing objects.
 *
 * ---Options controlling object destruction.
 * ---@class (exact) DestroyOptions
 * ---@field override? boolean Whether to bypass the target's SAFE flag; core objects and Wizard players remain protected.
 *
 * ---Fields selecting a native lock invocation to test.
 * ---@class (exact) LockPassesOptions
 * ---@field object DbRef|Object Required object whose lock is tested.
 * ---@field enactor DbRef|Object Required object attempting the action.
 * ---@field lock Lock Required typed lock constant from [`mux.world.locks`](lua://mux.world.locks).
 * ---@field cause? DbRef|Object Object that caused the action; defaults to `enactor`.
 * ---@field subject? DbRef|Object Object acted upon in the lock context; defaults to `enactor`.
 *
 * ---Filters for [`Object:contents`](lua://Object.contents).
 * ---@class (exact) ContentsOptions
 * ---@field types? ObjectType[] Native object kinds to include; an empty array matches nothing.
 * ---@field visible_to? DbRef|Object Viewer whose native visibility rules are applied.
 *
 * ---Filters for [`mux.world.list_objects`](lua://mux.world.list_objects).
 * ---@class (exact) ListObjectsOptions
 * ---@field types? ObjectType[] Native object kinds to include; an empty array matches nothing.
 * ---@field in_zone? DbRef|Object Include only objects directly assigned to this zone.
 * @endcode
 *
 * @par LuaLS definition mux namespace mux.world
 * @code{.lua}
 * ---World database object access.
 * ---@class MuxWorldPackage
 * ---@field access AccessNamespace Immutable namespace of command-access constants.
 * ---@field flags FlagNamespace Immutable namespace of registered flag constants.
 * ---@field locks LockNamespace Immutable namespace of native lock constants.
 * ---@field powers PowerNamespace Immutable namespace of registered power constants.
 * ---@field types ObjectTypeNamespace Immutable namespace of native object-kind constants.
 * local mux_world = {}
 * @endcode
 */
void lua_mux_install_world_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
  lua_mux_install_object_bindings(state, package);
  lua_mux_install_state_bindings(state, package);
  lua_mux_install_flag_power_bindings(state, package);
  lua_mux_install_lock_bindings(state, package);
  lua_command_access_install_namespace(state);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_pemit, 1);
  lua_setfield(state, -2, "pemit");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_list_objects, 1);
  lua_setfield(state, -2, "list_objects");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_create_object, 1);
  lua_setfield(state, -2, "create_object");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_teleport_object, 1);
  lua_setfield(state, -2, "teleport_object");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_destroy_object, 1);
  lua_setfield(state, -2, "destroy_object");
  lua_setfield(state, -2, "world");
}
