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
 * @par Lua name `mux.world.list_objects`
 * @par Lua signature `mux.world.list_objects( options? )`
 * @par Lua parameters - `options` (`ListObjectsOptions|nil`) Optional filters.
 * `types` is an array of typed constants from `mux.world.types`, and `in_zone`
 * is a dbref or Object whose direct zone members are included.
 * @par Lua returns - `objects` (`table`): Matching Object handles in ascending
 * dbref order.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` for malformed options or object type
 * constants not owned by this runtime.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid zone.
 * - `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` for a zone being destroyed.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
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
 * @par Lua name `mux.world.create_object`
 * @par Lua signature `mux.world.create_object( options )`
 * @par Lua parameters - `options` (`CreateObjectOptions`) Exact creation
 * fields. `type` and `name` are required. Rooms accept only optional `zone`;
 * things require `location` and accept optional `home` and `zone`; exits
 * require source `location` and accept optional `destination` and `zone`.
 * `type` must be `ROOM`, `THING`, or `EXIT` from `mux.world.types` in the
 * current runtime.
 * @par Lua returns - `object` (`Object`): The newly created object.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` for invalid or inapplicable fields, names, or
 * object type constants.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for invalid references or object kinds.
 * - `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` for referenced objects being destroyed
 * or creation failure.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
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
 * @par Lua name `mux.world.teleport`
 * @par Lua signature `mux.world.teleport( options )`
 * @par Lua parameters - `options` (`TeleportOptions`) Teleport fields. Its
 * `object` and `destination` references are required.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` for missing or unknown fields;
 * `LUA_ERROR_CODE_OBJECT_INVALID` for invalid references, object kinds, or a
 * self-destination; `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` for an object or
 * destination being destroyed, or when the native teleport is denied.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_teleport(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  static const char *const FIELDS[] = {"object", "destination"};
  bool object_present;
  bool destination_present;

  lua_mux_require_runtime(package, state, "world.teleport");
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
 * @par Lua name `mux.world.destroy`
 * @par Lua signature `mux.world.destroy( object, options? )`
 * @par Lua parameters - `object` (`number|Object`) Live object to destroy.
 * - `options` (`DestroyOptions|nil`) Optional fields; `override=true` bypasses
 * the target's SAFE flag, but never core-object or Wizard-player protection.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` for invalid options;
 * `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid object;
 * `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` for a protected or already-GOING object.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_destroy(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  static const char *const FIELDS[] = {"override"};
  bool override_safe = false;

  lua_mux_require_runtime(package, state, "world.destroy");
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
 * @par Lua name `mux.world.pemit`
 * @par Lua signature `mux.world.pemit( object, message )`
 * @par Lua parameters - `object` (`number|Object`) The recipient.
 * - `message` (`string`) Valid UTF-8 text without embedded NUL bytes.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_CONNECTION_INVALID` for embedded NUL or invalid UTF-8;
 * `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid recipient.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
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

void lua_mux_install_world_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
  lua_mux_install_object_bindings(state, package);
  lua_mux_install_state_bindings(state, package);
  lua_mux_install_attribute_bindings(state, package);
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
  lua_pushcclosure(state, lua_mux_teleport, 1);
  lua_setfield(state, -2, "teleport");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_destroy, 1);
  lua_setfield(state, -2, "destroy");
  lua_setfield(state, -2, "world");
}
