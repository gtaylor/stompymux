/* mux_text_bindings.c - Lua bindings for mux.text. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/utf8.h"

/**
 * Validates styled-text markup and returns it unchanged.
 *
 * @par LuaLS definition mux callable mux.text.markup
 * @code{.lua}
 * ---Validates styled-text markup and returns it unchanged.
 * ---@param value string
 * ---@return string markup
 * ---
 * ---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
 * ---@see mux.error.codes.text.invalid
 * function mux_text.markup(value) end
 * @endcode
 */
static int lua_mux_markup(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *markup = luaL_checkstring(state, 1);
  char *output = alloc_lbuf("lua_mux_markup");
  char error[256];

  if (!styled_text_compile(package->services->styled_text_palette, markup,
                           output, LBUF_SIZE, error, sizeof(error))) {
    free_buf(output);
    return lua_error_raise(state, LUA_ERROR_CODE_TEXT_INVALID,
                           "invalid styled-text markup: %s", error);
  }
  lua_pushstring(state, markup);
  free_buf(output);
  return 1;
}

/**
 * Tests whether every byte in a string is printable ASCII.
 *
 * @par LuaLS definition mux callable mux.text.is_printable_ascii
 * @code{.lua}
 * ---Tests whether every byte is printable ASCII (0x20 through 0x7e).
 * ---@param value string
 * ---@return boolean printable
 * function mux_text.is_printable_ascii(value) end
 * @endcode
 */
static int lua_mux_is_printable_ascii(lua_State *state) {
  size_t length;
  const char *value;

  luaL_checktype(state, 1, LUA_TSTRING);
  value = lua_tolstring(state, 1, &length);
  lua_pushboolean(state, utf8_is_printable_ascii(value, length));
  return 1;
}

typedef enum LuaStylePropertyKind : int {
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
  const char *value = nullptr;
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
    enabled = (lua_toboolean(state, -1) != 0);
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

/**
 * Applies styled-text markup described by an options table.
 *
 * @par LuaLS definition mux callable mux.text.style
 * @code{.lua}
 * ---Wraps text in markup described by the supplied style options.
 * ---@param value string
 * ---@param options StyleOptions
 * ---@return string styled
 * ---
 * ---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
 * ---@see mux.error.codes.text.invalid
 * function mux_text.style(value, options) end
 * @endcode
 */
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
    return lua_error_arg(state, 1, LUA_ERROR_CODE_TEXT_INVALID,
                         "value contains an embedded NUL byte");
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
      free_buf(markup);
      return lua_error_raise(state, LUA_ERROR_CODE_TEXT_INVALID,
                             "style fields have invalid types");
    }
  }
  safe_str(value, markup, &cursor);
  for (size_t index = 0; index < open_count; index++)
    safe_str("[/]", markup, &cursor);
  *cursor = '\0';

  validated = alloc_lbuf("lua_mux_style.validated");
  if (!styled_text_compile(package->services->styled_text_palette, markup,
                           validated, LBUF_SIZE, error, sizeof(error))) {
    free_buf(markup);
    free_buf(validated);
    return lua_error_raise(state, LUA_ERROR_CODE_TEXT_INVALID,
                           "invalid style: %s", error);
  }
  lua_pushstring(state, markup);
  free_buf(markup);
  free_buf(validated);
  return 1;
}

/**
 * Removes styled-text markup and ANSI styling from a string.
 *
 * @par LuaLS definition mux callable mux.text.strip_style
 * @code{.lua}
 * ---Removes styled-text markup and ANSI styling.
 * ---@param value string
 * ---@return string plain
 * function mux_text.strip_style(value) end
 * @endcode
 */
static int lua_mux_strip_style(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *value = luaL_checkstring(state, 1);
  char *output = alloc_lbuf("lua_mux_strip_style");

  styled_text_strip(package->services->styled_text_palette, value, output,
                    LBUF_SIZE);
  lua_pushstring(state, output);
  free_buf(output);
  return 1;
}

/**
 * Measures the visible byte width of styled text.
 *
 * @par LuaLS definition mux callable mux.text.width
 * @code{.lua}
 * ---Measures visible byte width, excluding markup and ANSI styling.
 * ---@param value string
 * ---@return integer width
 * function mux_text.width(value) end
 * @endcode
 */
static int lua_mux_text_width(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_pushinteger(state, (lua_Integer)styled_text_width(
                             package->services->styled_text_palette,
                             luaL_checkstring(state, 1)));
  return 1;
}

/**
 * Truncates styled text to a maximum visible width.
 *
 * @par LuaLS definition mux callable mux.text.truncate
 * @code{.lua}
 * ---Safely truncates styled text to a non-negative visible byte width.
 * ---@param value string
 * ---@param width integer
 * ---@return string truncated
 * ---
 * ---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
 * ---@see mux.error.codes.text.invalid
 * function mux_text.truncate(value, width) end
 * @endcode
 */
static int lua_mux_truncate_text(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *value = luaL_checkstring(state, 1);
  lua_Integer width = luaL_checkinteger(state, 2);
  char *output;

  if (width < 0)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_TEXT_INVALID,
                         "width must not be negative");
  output = alloc_lbuf("lua_mux_truncate_text");
  styled_text_truncate(package->services->styled_text_palette, value,
                       (size_t)width, output, LBUF_SIZE);
  lua_pushstring(state, output);
  free_buf(output);
  return 1;
}

/**
 * @par LuaLS definition mux type text.style_options
 * @code{.lua}
 * ---Optional styled-text attributes applied by [`mux.text.style`](lua://mux.text.style).
 * ---@class StyleOptions
 * ---@field foreground? string Palette foreground name.
 * ---@field background? string Palette background name.
 * ---@field bold? boolean Whether to enable bold intensity.
 * ---@field underline? boolean Whether to underline the text.
 * ---@field inverse? boolean Whether to swap foreground and background presentation.
 * @endcode
 *
 * @par LuaLS definition mux namespace mux.text
 * @code{.lua}
 * ---Styled-text validation, construction, inspection, and transformation.
 * ---@class MuxTextPackage
 * local mux_text = {}
 * @endcode
 */
void lua_mux_install_text_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
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
  lua_setfield(state, -2, "width");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_truncate_text, 1);
  lua_setfield(state, -2, "truncate");
  lua_setfield(state, -2, "text");
}
