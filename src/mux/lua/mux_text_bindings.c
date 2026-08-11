/* mux_package.c - Built-in Lua mux package bindings. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/mux_package.h"
#include "mux/lua/mux_package_internal.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/utf8.h"

static int lua_mux_markup(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *markup = luaL_checkstring(state, 1);
  char *output = alloc_lbuf("lua_mux_markup");
  char error[256];

  if (!styled_text_compile(package->services->styled_text_palette, markup,
                           output, LBUF_SIZE, error, sizeof(error))) {
    free_lbuf(output);
    return luaL_error(state, "invalid styled-text markup: %s", error);
  }
  lua_pushstring(state, markup);
  free_lbuf(output);
  return 1;
}

static int lua_mux_is_printable_ascii(lua_State *state) {
  size_t length;
  const char *value;

  luaL_checktype(state, 1, LUA_TSTRING);
  value = lua_tolstring(state, 1, &length);
  lua_pushboolean(state, utf8_is_printable_ascii(value, length));
  return 1;
}

typedef enum LuaStylePropertyKind {
  LUA_STYLE_PROPERTY_STRING,
  LUA_STYLE_PROPERTY_BOOLEAN,
} LuaStylePropertyKind;

typedef struct LuaStylePropertyRequest {
  lua_State *state;
  int table;
  const char *field;
  const char *tag;
  LuaStylePropertyKind kind;
  char *markup;
  char **cursor;
  size_t *open_count;
} LuaStylePropertyRequest;

typedef struct LuaStyleProperty {
  const char *field;
  const char *tag;
  LuaStylePropertyKind kind;
} LuaStyleProperty;

static bool lua_mux_style_open(const LuaStylePropertyRequest *request) {
  lua_State *state = request->state;
  const char *value;
  bool enabled;

  lua_getfield(state, request->table, request->field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return true;
  }
  if (request->kind == LUA_STYLE_PROPERTY_BOOLEAN) {
    if (!lua_isboolean(state, -1)) {
      lua_pop(state, 1);
      return false;
    }
    enabled = lua_toboolean(state, -1);
    lua_pop(state, 1);
    if (!enabled)
      return true;
  } else {
    if (!lua_isstring(state, -1)) {
      lua_pop(state, 1);
      return false;
    }
    value = lua_tostring(state, -1);
    if (strchr(value, '[') || strchr(value, ']')) {
      lua_pop(state, 1);
      return false;
    }
  }
  safe_str("[", request->markup, request->cursor);
  safe_str(request->tag, request->markup, request->cursor);
  if (request->kind == LUA_STYLE_PROPERTY_STRING) {
    safe_str(value, request->markup, request->cursor);
    lua_pop(state, 1);
  }
  safe_str("]", request->markup, request->cursor);
  (*request->open_count)++;
  return true;
}

static int lua_mux_style(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  size_t text_length;
  const char *value = luaL_checklstring(state, 1, &text_length);
  char *markup;
  char *cursor;
  char *validated;
  char error[256];
  size_t open_count = 0;

  if (strlen(value) != text_length)
    return luaL_argerror(state, 1, "value contains an embedded NUL byte");
  luaL_checktype(state, 2, LUA_TTABLE);
  markup = alloc_lbuf("lua_mux_style.markup");
  cursor = markup;
  *cursor = '\0';
  static const LuaStyleProperty PROPERTIES[] = {
      {"foreground", "fg=", LUA_STYLE_PROPERTY_STRING},
      {"background", "bg=", LUA_STYLE_PROPERTY_STRING},
      {"bold", "bold", LUA_STYLE_PROPERTY_BOOLEAN},
      {"underline", "underline", LUA_STYLE_PROPERTY_BOOLEAN},
      {"inverse", "inverse", LUA_STYLE_PROPERTY_BOOLEAN}};
  for (size_t index = 0; index < sizeof(PROPERTIES) / sizeof(PROPERTIES[0]);
       index++) {
    const LuaStyleProperty *property = checked_storage_at_const(
        PROPERTIES, sizeof(PROPERTIES) / sizeof(PROPERTIES[0]),
        sizeof(*PROPERTIES), index);
    if (!lua_mux_style_open(
            &(LuaStylePropertyRequest){.state = state,
                                       .table = 2,
                                       .field = property->field,
                                       .tag = property->tag,
                                       .kind = property->kind,
                                       .markup = markup,
                                       .cursor = &cursor,
                                       .open_count = &open_count})) {
      free_lbuf(markup);
      return luaL_error(state, "style fields have invalid types");
    }
  }
  safe_str(value, markup, &cursor);
  for (size_t index = 0; index < open_count; index++)
    safe_str("[/]", markup, &cursor);
  *cursor = '\0';

  validated = alloc_lbuf("lua_mux_style.validated");
  if (!styled_text_compile(package->services->styled_text_palette, markup,
                           validated, LBUF_SIZE, error, sizeof(error))) {
    free_lbuf(markup);
    free_lbuf(validated);
    return luaL_error(state, "invalid style: %s", error);
  }
  lua_pushstring(state, markup);
  free_lbuf(markup);
  free_lbuf(validated);
  return 1;
}

static int lua_mux_strip_style(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *value = luaL_checkstring(state, 1);
  char *output = alloc_lbuf("lua_mux_strip_style");

  styled_text_strip(package->services->styled_text_palette, value, output,
                    LBUF_SIZE);
  lua_pushstring(state, output);
  free_lbuf(output);
  return 1;
}

static int lua_mux_text_width(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_pushinteger(state, (lua_Integer)styled_text_width(
                             package->services->styled_text_palette,
                             luaL_checkstring(state, 1)));
  return 1;
}

static int lua_mux_truncate_text(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *value = luaL_checkstring(state, 1);
  lua_Integer width = luaL_checkinteger(state, 2);
  char *output;

  if (width < 0)
    return luaL_argerror(state, 2, "width must not be negative");
  output = alloc_lbuf("lua_mux_truncate_text");
  styled_text_truncate(package->services->styled_text_palette, value,
                       (size_t)width, output, LBUF_SIZE);
  lua_pushstring(state, output);
  free_lbuf(output);
  return 1;
}

void lua_mux_install_text_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_markup, 1);
  lua_setfield(state, -2, "markup");
  lua_pushcfunction(state, lua_mux_is_printable_ascii);
  lua_setfield(state, -2, "is_printable_ascii");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_style, 1);
  lua_setfield(state, -2, "style");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_strip_style, 1);
  lua_setfield(state, -2, "strip_style");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_text_width, 1);
  lua_setfield(state, -2, "text_width");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_truncate_text, 1);
  lua_setfield(state, -2, "truncate_text");
}
