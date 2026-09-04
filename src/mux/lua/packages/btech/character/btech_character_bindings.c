/* btech_character_bindings.c - Native Lua bindings for btech.character. */

#include <lauxlib.h>
#include <limits.h>
#include <lua.h>
#include <math.h>
#include <strings.h>

#include "btech/character/btechstats_api.h"
#include "btech/character/btechstats_global.h"
#include "btech/character/btechstats_internal.h"
#include "btechstats.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

static DbRef require_character(lua_State *state, LuaBtechPackage *package,
                               int argument) {
  if (luaL_testudata(state, argument, LUA_MUX_OBJECT_METATABLE) == nullptr)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                        "character must be an Object");
  const DbRef CHARACTER = lua_btech_require_object(package, state, argument);
  if (!is_player(package->services->database, CHARACTER))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                        "character must be a player");
  return CHARACTER;
}

static int check_integer(lua_State *state, int argument, const char *label) {
  if (lua_type(state, argument) != LUA_TNUMBER)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be an integer", label);
  const lua_Number NUMBER = lua_tonumber(state, argument);
  if (!isfinite(NUMBER) || floor(NUMBER) != NUMBER || NUMBER < INT_MIN ||
      NUMBER > INT_MAX)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a ranged integer", label);
  return (int)NUMBER;
}

static int require_value(lua_State *state, BtechContext *context,
                         int argument) {
  int code;
  if (lua_type(state, argument) == LUA_TSTRING)
    code = char_getvaluecode(context, lua_tostring(state, argument));
  else
    code = check_integer(state, argument, "value");
  if (code < 0 || code >= NUM_CHARVALUES)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "unknown character value");
  return code;
}

static bool kind_matches(const CharacterValue *definition, const char *kind) {
  const char *type = character_value_type_name(definition->type);
  return (bool)(strcasecmp(type, kind) == 0 ||
                (strcasecmp(kind, "skills") == 0 &&
                 strcasecmp(type, "skill") == 0) ||
                (strcasecmp(kind, "advantages") == 0 &&
                 strcasecmp(type, "advantage") == 0) ||
                (strcasecmp(kind, "attributes") == 0 &&
                 strcasecmp(type, "attribute") == 0));
}

static void push_definition(lua_State *state, int code,
                            const CharacterValue *definition) {
  lua_newtable(state);
  lua_pushinteger(state, code);
  lua_setfield(state, -2, "code");
  lua_pushstring(state, definition->name);
  lua_setfield(state, -2, "name");
  lua_pushstring(state, character_value_type_name(definition->type));
  lua_setfield(state, -2, "kind");
  lua_pushinteger(state, definition->default_xp_threshold);
  lua_setfield(state, -2, "default_experience_threshold");
}

static int lua_btech_character_catalog(lua_State *state,
                                       LuaBtechPackage *package) {
  if (lua_type(state, 1) != LUA_TSTRING)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "kind must be a string");
  const char *kind = lua_tostring(state, 1);
  const bool FILTER = lua_isnoneornil(state, 2) == 0;
  const DbRef CHARACTER =
      FILTER ? require_character(state, package, 2) : NOTHING;
  int output = 1;
  lua_newtable(state);
  for (int code = 0; code < NUM_CHARVALUES; code++) {
    const CharacterValue *definition = character_value_definition(code);
    if (!kind_matches(definition, kind))
      continue;
    if (FILTER && definition->type != CHAR_ATTRIBUTE &&
        character_value_by_code(
            &(CharacterValueRequest){.context = lua_btech_context(package),
                                     .player = CHARACTER,
                                     .code = code}) == 0 &&
        char_getxpbycode(
            &(CharacterValueRequest){.context = lua_btech_context(package),
                                     .player = CHARACTER,
                                     .code = code}) == 0)
      continue;
    push_definition(state, code, definition);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

static int lua_btech_character_value(lua_State *state,
                                     LuaBtechPackage *package) {
  const DbRef CHARACTER = require_character(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  const int CODE = require_value(state, context, 2);
  const CharacterValue *definition = character_value_definition(CODE);
  lua_newtable(state);
  push_definition(state, CODE, definition);
  lua_setfield(state, -2, "definition");
  lua_pushinteger(state,
                  character_value_by_code(&(CharacterValueRequest){
                      .context = context, .player = CHARACTER, .code = CODE}));
  lua_setfield(state, -2, "amount");
  if (definition->type == CHAR_SKILL) {
    lua_pushinteger(state,
                    char_getskilltargetbycode(context, CHARACTER, CODE, 0));
    lua_setfield(state, -2, "target");
    lua_pushinteger(
        state, char_getxpbycode(&(CharacterValueRequest){
                   .context = context, .player = CHARACTER, .code = CODE}));
    lua_setfield(state, -2, "experience");
    lua_pushinteger(state,
                    character_xp_to_next_level(context, CHARACTER, CODE));
    lua_setfield(state, -2, "experience_to_next_level");
  }
  return 1;
}

static int lua_btech_character_experience_threshold(lua_State *state,
                                                    LuaBtechPackage *package) {
  if (lua_type(state, 1) != LUA_TSTRING)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "skill must be a string");
  const int THRESHOLD =
      btthreshold_func(lua_btech_context(package), lua_tostring(state, 1));
  if (THRESHOLD < 0)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID, "unknown skill");
  lua_pushinteger(state, THRESHOLD);
  return 1;
}

static int lua_btech_character_set_value(lua_State *state,
                                         LuaBtechPackage *package) {
  const DbRef CHARACTER = require_character(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  const int CODE = require_value(state, context, 2);
  character_value_set_by_code(&(CharacterValueChange){
      .target = {.context = context, .player = CHARACTER, .code = CODE},
      .value = check_integer(state, 3, "amount")});
  return 0;
}

static int lua_btech_character_set_skill_target(lua_State *state,
                                                LuaBtechPackage *package) {
  const DbRef CHARACTER = require_character(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  const int CODE = require_value(state, context, 2);
  if (character_value_definition(CODE)->type != CHAR_SKILL)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "value is not a skill");
  const int TARGET = check_integer(state, 3, "target");
  if (!character_skill_target_set(context, CHARACTER, CODE, TARGET))
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "target is unreachable for this skill");
  return 0;
}

static int lua_btech_character_set_skill_experience(lua_State *state,
                                                    LuaBtechPackage *package) {
  const DbRef CHARACTER = require_character(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  const int CODE = require_value(state, context, 2);
  if (character_value_definition(CODE)->type != CHAR_SKILL)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "value is not a skill");
  const int EXPERIENCE = check_integer(state, 3, "experience");
  if (EXPERIENCE < 0)
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "experience must be nonnegative");
  PSTATS stats;
  character_stats_retrieve(context, CHARACTER, VALUES_SKILLS, &stats);
  character_stats_xp_set(&(CharacterStatsExperienceChange){
      .stats = &stats, .code = CODE, .value = EXPERIENCE});
  character_stats_store(context, CHARACTER, &stats, VALUES_SKILLS);
  return 0;
}

static int lua_btech_character_add_skill_experience(lua_State *state,
                                                    LuaBtechPackage *package) {
  const DbRef CHARACTER = require_character(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  const int CODE = require_value(state, context, 2);
  if (character_value_definition(CODE)->type != CHAR_SKILL)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "value is not a skill");
  if (!char_gainxpbycode(&(CharacterExperienceChange){
          .target = {.context = context, .player = CHARACTER, .code = CODE},
          .amount = check_integer(state, 3, "amount"),
          .override_interval = true}))
    return lua_btech_operation_error(state, "experience_adjustment_failed",
                                     "experience adjustment failed");
  return 0;
}

static const BtechLuaNativeEntry BTECH_CHARACTER_ENTRIES[] = {
    {"catalog", "character.catalog", lua_btech_character_catalog},
    {"value", "character.value", lua_btech_character_value},
    {"experience_threshold", "character.experience_threshold",
     lua_btech_character_experience_threshold},
    {"set_value", "character.set_value", lua_btech_character_set_value},
    {"set_skill_target", "character.set_skill_target",
     lua_btech_character_set_skill_target},
    {"set_skill_experience", "character.set_skill_experience",
     lua_btech_character_set_skill_experience},
    {"add_skill_experience", "character.add_skill_experience",
     lua_btech_character_add_skill_experience},
};

void lua_btech_install_character_bindings(lua_State *state,
                                          LuaBtechPackage *package) {
  lua_btech_install_native_bindings(
      state, package, "character", BTECH_CHARACTER_ENTRIES,
      sizeof(BTECH_CHARACTER_ENTRIES) / sizeof(BTECH_CHARACTER_ENTRIES[0]));
}
