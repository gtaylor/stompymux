/* osc8.c - OSC 8 URI validation and configuration serialization. */

#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>

#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/internal.h"
#include "mux/support/styled_text/render.h"
#include "mux/support/utf8.h"

static const char *osc8_suffix(const char *text, size_t length, size_t offset) {
  return checked_storage_at_const(text, length + 1, sizeof(char), offset);
}

static unsigned char osc8_byte(const char *text, size_t length, size_t index) {
  return (unsigned char)*osc8_suffix(text, length, index);
}

static void osc8_write(char *text, size_t capacity, size_t index, char value) {
  *(char *)checked_storage_at(text, capacity, sizeof(char), index) = value;
}

static bool uri_unreserved(unsigned char byte) {
  return (byte >= 'A' && byte <= 'Z') || (byte >= 'a' && byte <= 'z') ||
         (byte >= '0' && byte <= '9') || byte == '-' || byte == '.' ||
         byte == '_' || byte == '~';
}

static bool uri_reserved(unsigned char byte) {
  return strchr(":/?#[]@!$&'()*+,;=", byte) != nullptr;
}

bool styled_link_target_unquote(const char *start, const char *end,
                                char *target, size_t target_size,
                                const char **remainder, char *error,
                                size_t error_size) {
  size_t used = 0;
  const size_t LENGTH = strlen(start) - strlen(end);
  size_t offset = 0;

  if (LENGTH == 0 || osc8_byte(start, LENGTH, 0) != '"') {
    styled_set_error(error, error_size, "link target must be double quoted");
    return false;
  }
  offset++;
  while (offset < LENGTH && osc8_byte(start, LENGTH, offset) != '"') {
    unsigned char byte = osc8_byte(start, LENGTH, offset++);

    if (byte == '\\') {
      if (offset == LENGTH || (osc8_byte(start, LENGTH, offset) != '\\' &&
                               osc8_byte(start, LENGTH, offset) != '"')) {
        styled_set_error(error, error_size, "invalid escape in link target");
        return false;
      }
      byte = osc8_byte(start, LENGTH, offset++);
    }
    if (byte < 0x20 || byte == 0x7f) {
      styled_set_error(error, error_size,
                       "link target contains a control byte");
      return false;
    }
    if (used + 1 >= target_size) {
      styled_set_error(error, error_size, "link target is too long");
      return false;
    }
    osc8_write(target, target_size, used++, (char)byte);
  }
  if (offset == LENGTH || osc8_byte(start, LENGTH, offset) != '"') {
    styled_set_error(error, error_size, "unterminated quoted link target");
    return false;
  }
  offset++;
  while (offset < LENGTH && (isspace)(osc8_byte(start, LENGTH, offset)))
    offset++;
  *remainder = osc8_suffix(start, LENGTH, offset);
  osc8_write(target, target_size, used, '\0');
  if (used == 0) {
    styled_set_error(error, error_size, "link target must not be empty");
    return false;
  }
  if (!utf8_validate_printable(target, used)) {
    styled_set_error(error, error_size,
                     "link target must be printable, valid UTF-8");
    return false;
  }
  return true;
}

bool styled_external_uri_valid(const char *uri, char *error,
                               size_t error_size) {
  size_t length = strlen(uri);
  const char *body;

  if (!strncasecmp(uri, "http:", 5))
    body = checked_string_suffix(uri, 5);
  else if (!strncasecmp(uri, "https:", 6))
    body = checked_string_suffix(uri, 6);
  else if (!strncasecmp(uri, "ftp:", 4))
    body = checked_string_suffix(uri, 4);
  else {
    styled_set_error(error, error_size,
                     "link URI scheme must be http, https, or ftp");
    return false;
  }
  if (*body == '\0') {
    styled_set_error(error, error_size, "link URI must include a destination");
    return false;
  }
  if (length > OSC8_URI_LIMIT) {
    styled_set_error(error, error_size, "link URI is too long");
    return false;
  }
  for (size_t index = 0; index < length; index++) {
    unsigned char byte = osc8_byte(uri, length, index);

    if (byte == '%' && index + 2 < length &&
        (isxdigit)(osc8_byte(uri, length, index + 1)) &&
        (isxdigit)(osc8_byte(uri, length, index + 2))) {
      index += 2;
      continue;
    }
    if (byte >= 0x80 || (!uri_unreserved(byte) && !uri_reserved(byte))) {
      styled_set_error(error, error_size,
                       "link URI contains a byte that must be percent encoded");
      return false;
    }
  }
  return true;
}

bool styled_command_uri_encode(StyledLinkKind kind, const char *command,
                               char *uri, size_t uri_size, char *error,
                               size_t error_size) {
  const char *scheme = kind == STYLED_LINK_SEND ? "send:" : "prompt:";
  size_t used = strlen(scheme);

  if (used + 1 > uri_size)
    goto too_long;
  memcpy(uri, scheme, used + 1);
  const size_t COMMAND_LENGTH = strlen(command);
  for (size_t index = 0; index < COMMAND_LENGTH; index++) {
    const unsigned char BYTE = osc8_byte(command, COMMAND_LENGTH, index);
    if (uri_unreserved(BYTE)) {
      if (used + 1 >= uri_size)
        goto too_long;
      osc8_write(uri, uri_size, used++, (char)BYTE);
    } else {
      if (used + 3 >= uri_size)
        goto too_long;
      (void)snprintf(checked_storage_at(uri, uri_size, sizeof(char), used),
                     uri_size - used, "%%%02X", BYTE);
      used += 3;
    }
  }
  osc8_write(uri, uri_size, used, '\0');
  return true;

too_long:
  styled_set_error(error, error_size, "encoded link URI is too long");
  return false;
}

bool styled_link_enabled(StyledLinkKind kind,
                         const StyledTextRenderOptions *options) {
  if (options == nullptr)
    return false;
  switch (kind) {
  case STYLED_LINK_EXTERNAL:
    return options->osc_hyperlinks;
  case STYLED_LINK_SEND:
    return options->osc_hyperlinks_send;
  case STYLED_LINK_PROMPT:
    return options->osc_hyperlinks_prompt;
  }
  return false;
}

bool styled_config_capability_advertised(
    const StyledTextRenderOptions *options) {
  return options &&
         (options->osc_hyperlinks_style_basic ||
          options->osc_hyperlinks_style_states ||
          options->osc_hyperlinks_tooltip || options->osc_hyperlinks_menu ||
          options->osc_hyperlinks_visibility ||
          options->osc_hyperlinks_spoiler || options->osc_hyperlinks_disabled ||
          options->osc_hyperlinks_selection || options->osc_hyperlinks_compact);
}

bool styled_emit_link_open(const char *uri, char *output, size_t output_size,
                           size_t *used) {
  constexpr char PREFIX[] = "\033]8;;";
  constexpr char SUFFIX[] = "\033\\";
  size_t length = sizeof(PREFIX) - 1 + strlen(uri) + sizeof(SUFFIX) - 1;

  if (*used + length + OSC8_CLOSE_SIZE >= output_size)
    return false;
  return styled_append_string(output, output_size, used, PREFIX) &&
         styled_append_string(output, output_size, used, uri) &&
         styled_append_string(output, output_size, used, SUFFIX);
}

bool styled_emit_link_close(char *output, size_t output_size, size_t *used) {
  return styled_append_string(output, output_size, used, "\033]8;;\033\\");
}

static bool append_json_separator(char *json, size_t json_size, size_t *used,
                                  bool *first) {
  if (*first) {
    *first = false;
    return true;
  }
  return styled_append_string(json, json_size, used, ",");
}

static bool append_json_color(char *json, size_t json_size, size_t *used,
                              bool *first, const char *name,
                              const StyledColor *color) {
  char property[96];
  int length = snprintf(property, sizeof(property), "\"%s\":\"#%02x%02x%02x\"",
                        name, color->red, color->green, color->blue);
  return length > 0 && append_json_separator(json, json_size, used, first) &&
         styled_append_bytes(json, json_size, used, property, (size_t)length);
}

static bool append_json_boolean(char *json, size_t json_size, size_t *used,
                                bool *first, const char *name,
                                StyledBoolean value) {
  char property[64];
  int length = snprintf(property, sizeof(property), "\"%s\":%s", name,
                        value == STYLED_BOOLEAN_TRUE ? "true" : "false");
  return length > 0 && append_json_separator(json, json_size, used, first) &&
         styled_append_bytes(json, json_size, used, property, (size_t)length);
}

static const char *decoration_json_value(StyledDecoration decoration) {
  switch (decoration) {
  case STYLED_DECORATION_FALSE:
    return "false";
  case STYLED_DECORATION_TRUE:
    return "true";
  case STYLED_DECORATION_WAVY:
    return "\"wavy\"";
  case STYLED_DECORATION_DOTTED:
    return "\"dotted\"";
  case STYLED_DECORATION_DASHED:
    return "\"dashed\"";
  case STYLED_DECORATION_UNSET:
    break;
  }
  return nullptr;
}

static bool append_json_decoration(char *json, size_t json_size, size_t *used,
                                   bool *first, const char *name,
                                   StyledDecoration decoration) {
  char property[64];
  const char *value = decoration_json_value(decoration);
  int length;

  if (!value)
    return false;
  length = snprintf(property, sizeof(property), "\"%s\":%s", name, value);
  return length > 0 && append_json_separator(json, json_size, used, first) &&
         styled_append_bytes(json, json_size, used, property, (size_t)length);
}

static bool append_json_properties(char *json, size_t json_size, size_t *used,
                                   const StyledLinkProperties *properties,
                                   bool compact) {
  bool first = true;

  if (!styled_append_string(json, json_size, used, "{"))
    return false;
  if (properties->has_foreground &&
      !append_json_color(json, json_size, used, &first, compact ? "c" : "color",
                         &properties->foreground))
    return false;
  if (properties->has_background &&
      !append_json_color(json, json_size, used, &first, "bg",
                         &properties->background))
    return false;
  if (properties->bold != STYLED_BOOLEAN_UNSET &&
      !append_json_boolean(json, json_size, used, &first,
                           compact ? "b" : "bold", properties->bold))
    return false;
  if (properties->italic != STYLED_BOOLEAN_UNSET &&
      !append_json_boolean(json, json_size, used, &first,
                           compact ? "i" : "italic", properties->italic))
    return false;
  if (properties->underline != STYLED_DECORATION_UNSET &&
      !append_json_decoration(json, json_size, used, &first,
                              compact ? "u" : "underline",
                              properties->underline))
    return false;
  if (properties->overline != STYLED_DECORATION_UNSET &&
      !append_json_decoration(json, json_size, used, &first,
                              compact ? "o" : "overline", properties->overline))
    return false;
  if (properties->strikethrough != STYLED_DECORATION_UNSET &&
      !append_json_decoration(json, json_size, used, &first,
                              compact ? "st" : "strikethrough",
                              properties->strikethrough))
    return false;
  if (properties->has_decoration_color &&
      !append_json_color(json, json_size, used, &first,
                         compact ? "tdc" : "text-decoration-color",
                         &properties->decoration_color))
    return false;
  return styled_append_string(json, json_size, used, "}");
}

static bool append_json_string(char *json, size_t json_size, size_t *used,
                               const char *value) {
  if (!styled_append_string(json, json_size, used, "\""))
    return false;
  const size_t LENGTH = strlen(value);
  for (size_t index = 0; index < LENGTH; index++) {
    const unsigned char BYTE = osc8_byte(value, LENGTH, index);
    if (BYTE == '"' || BYTE == '\\') {
      char escaped[2] = {'\\', (char)BYTE};
      if (!styled_append_bytes(json, json_size, used, escaped, sizeof(escaped)))
        return false;
    } else if (BYTE < 0x20) {
      char escaped[7];
      int escaped_length = snprintf(escaped, sizeof(escaped), "\\u%04x", BYTE);
      if (escaped_length <= 0 ||
          !styled_append_bytes(json, json_size, used, escaped,
                               (size_t)escaped_length))
        return false;
    } else if (!styled_append_bytes(json, json_size, used,
                                    osc8_suffix(value, LENGTH, index), 1)) {
      return false;
    }
  }
  return styled_append_string(json, json_size, used, "\"");
}

static bool append_json_style(char *json, size_t json_size, size_t *used,
                              bool *root_first, const StyledLinkStyle *style,
                              bool include_base, bool include_states,
                              bool compact) {
  static const char *const COMPACT_STATES[STYLED_LINK_STATE_COUNT] = {
      "a", "h", "fv", "f", "vi", "sl", "d", "l", "al",
  };
  bool first = true;

  if (!append_json_separator(json, json_size, used, root_first) ||
      !styled_append_string(json, json_size, used,
                            compact ? "\"s\":{" : "\"style\":{"))
    return false;
  if (include_base && styled_link_properties_present(&style->base)) {
    char properties[512];
    size_t property_used = 0;
    properties[0] = '\0';
    if (!append_json_properties(properties, sizeof(properties), &property_used,
                                &style->base, compact))
      return false;
    if (!append_json_separator(json, json_size, used, &first) ||
        !styled_append_bytes(json, json_size, used,
                             checked_string_suffix(properties, 1),
                             strlen(properties) - 2))
      return false;
  }
  if (include_states) {
    for (size_t index = 0; index < STYLED_LINK_STATE_COUNT; index++) {
      const StyledLinkProperties *properties =
          styled_link_style_state_const(style, index);
      if (!styled_link_properties_present(properties))
        continue;
      if (!append_json_separator(json, json_size, used, &first))
        return false;
      char name[48];
      const char *compact_name = *(const char *const *)checked_storage_at_const(
          (const void *)COMPACT_STATES, STYLED_LINK_STATE_COUNT,
          sizeof(*COMPACT_STATES), index);
      const char *state_name = *(const char *const *)checked_storage_at_const(
          (const void *)STYLED_LINK_STATE_NAMES, STYLED_LINK_STATE_COUNT,
          sizeof(*STYLED_LINK_STATE_NAMES), index);
      int length = snprintf(name, sizeof(name),
                            "\"%s\":", compact ? compact_name : state_name);
      if (length <= 0 ||
          !styled_append_bytes(json, json_size, used, name, (size_t)length) ||
          !append_json_properties(json, json_size, used, properties, compact))
        return false;
    }
  }
  return !first && styled_append_string(json, json_size, used, "}");
}

bool styled_link_menu_has_enabled_action(
    const StyledLinkConfig *config, const StyledTextRenderOptions *options) {
  for (size_t index = 0; index < config->menu_count; index++) {
    const StyledLinkMenuItem *item =
        styled_link_menu_item_at_const(config, index);

    if (item->has_action && styled_link_enabled(item->action_kind, options))
      return true;
  }
  return false;
}

static bool append_json_menu(char *json, size_t json_size, size_t *used,
                             bool *root_first, const StyledLinkConfig *config,
                             const StyledTextRenderOptions *options,
                             bool compact) {
  bool first = true;
  bool have_action = false;
  bool pending_separator = false;

  if (!append_json_separator(json, json_size, used, root_first) ||
      !styled_append_string(json, json_size, used,
                            compact ? "\"m\":[" : "\"menu\":["))
    return false;
  for (size_t index = 0; index < config->menu_count; index++) {
    const StyledLinkMenuItem *item =
        styled_link_menu_item_at_const(config, index);
    char action[OSC8_URI_LIMIT + 16];

    if (item->separator) {
      if (have_action)
        pending_separator = true;
      continue;
    }
    if (!styled_link_enabled(item->action_kind, options))
      continue;
    if (pending_separator) {
      if (!append_json_separator(json, json_size, used, &first) ||
          !styled_append_string(json, json_size, used, "\"-\""))
        return false;
      pending_separator = false;
    }
    if (!append_json_separator(json, json_size, used, &first) ||
        !styled_append_string(json, json_size, used, "{") ||
        !append_json_string(json, json_size, used, item->label) ||
        !styled_append_string(json, json_size, used, ":"))
      return false;
    if (item->action_kind == STYLED_LINK_EXTERNAL) {
      if (strlen(item->action) >= sizeof(action))
        return false;
      memcpy(action, item->action, strlen(item->action) + 1);
    } else {
      const char *scheme =
          item->action_kind == STYLED_LINK_SEND ? "send:" : "prompt:";
      int length =
          snprintf(action, sizeof(action), "%s%s", scheme, item->action);
      if (length <= 0 || (size_t)length >= sizeof(action))
        return false;
    }
    if (!append_json_string(json, json_size, used, action) ||
        !styled_append_string(json, json_size, used, "}"))
      return false;
    have_action = true;
  }
  return have_action && styled_append_string(json, json_size, used, "]");
}

static bool append_json_title(char *json, size_t json_size, size_t *used,
                              bool *root_first, const StyledLinkConfig *config,
                              bool include_style, bool compact) {
  if (!append_json_separator(json, json_size, used, root_first) ||
      !styled_append_string(json, json_size, used,
                            compact ? "\"ti\":" : "\"title\":"))
    return false;
  if (!include_style)
    return append_json_string(json, json_size, used, config->title);
  if (!styled_append_string(json, json_size, used, "{\"text\":") ||
      !append_json_string(json, json_size, used, config->title) ||
      !styled_append_string(json, json_size, used,
                            compact ? ",\"s\":" : ",\"style\":"))
    return false;
  if (!append_json_properties(json, json_size, used, &config->title_style,
                              compact))
    return false;
  return styled_append_string(json, json_size, used, "}");
}

static bool append_json_uint32(char *json, size_t json_size, size_t *used,
                               bool *first, const char *name, uint32_t value) {
  char property[64];
  int length = snprintf(property, sizeof(property), "\"%s\":%u", name, value);

  return length > 0 && append_json_separator(json, json_size, used, first) &&
         styled_append_bytes(json, json_size, used, property, (size_t)length);
}

static bool append_json_visibility(char *json, size_t json_size, size_t *used,
                                   bool *root_first,
                                   const StyledLinkVisibility *visibility,
                                   bool compact) {
  bool first = true;
  bool expire_first = true;
  const char *action;

  switch (visibility->action) {
  case STYLED_VISIBILITY_ACTION_CONCEAL:
    action = "\"conceal\"";
    break;
  case STYLED_VISIBILITY_ACTION_REVEAL:
    action = "\"reveal\"";
    break;
  case STYLED_VISIBILITY_ACTION_REVEAL_CONCEAL:
    action = "[\"reveal\",\"conceal\"]";
    break;
  case STYLED_VISIBILITY_ACTION_UNSET:
    action = nullptr;
    break;
  }
  if (!append_json_separator(json, json_size, used, root_first) ||
      !styled_append_string(json, json_size, used,
                            compact ? "\"v\":{" : "\"visibility\":{"))
    return false;
  if (action && (!append_json_separator(json, json_size, used, &first) ||
                 !styled_append_string(json, json_size, used, "\"action\":") ||
                 !styled_append_string(json, json_size, used, action)))
    return false;
  if (visibility->has_delay &&
      !append_json_uint32(json, json_size, used, &first, "delay",
                          visibility->delay))
    return false;
  bool have_expire = visibility->expire_input != STYLED_BOOLEAN_UNSET ||
                     visibility->expire_prompt != STYLED_BOOLEAN_UNSET ||
                     visibility->expire_output != STYLED_BOOLEAN_UNSET ||
                     visibility->has_output_delay;
  if (have_expire) {
    if (!append_json_separator(json, json_size, used, &first) ||
        !styled_append_string(json, json_size, used, "\"expire\":{"))
      return false;
    if (visibility->expire_input != STYLED_BOOLEAN_UNSET &&
        !append_json_boolean(json, json_size, used, &expire_first, "input",
                             visibility->expire_input))
      return false;
    if (visibility->expire_prompt != STYLED_BOOLEAN_UNSET &&
        !append_json_boolean(json, json_size, used, &expire_first, "prompt",
                             visibility->expire_prompt))
      return false;
    if (visibility->expire_output != STYLED_BOOLEAN_UNSET &&
        !append_json_boolean(json, json_size, used, &expire_first, "output",
                             visibility->expire_output))
      return false;
    if (visibility->has_output_delay &&
        !append_json_uint32(json, json_size, used, &expire_first, "outputDelay",
                            visibility->output_delay))
      return false;
    if (!styled_append_string(json, json_size, used, "}"))
      return false;
  }
  if (visibility->wholeline != STYLED_BOOLEAN_UNSET &&
      !append_json_boolean(json, json_size, used, &first, "wholeline",
                           visibility->wholeline))
    return false;
  return styled_append_string(json, json_size, used, "}");
}

static bool append_json_selection(char *json, size_t json_size, size_t *used,
                                  bool *root_first,
                                  const StyledLinkSelection *selection,
                                  bool compact) {
  bool first = true;

  if (!append_json_separator(json, json_size, used, root_first) ||
      !styled_append_string(json, json_size, used,
                            compact ? "\"sel\":{" : "\"selection\":{"))
    return false;
  if (selection->group &&
      (!append_json_separator(json, json_size, used, &first) ||
       !styled_append_string(json, json_size, used, "\"group\":") ||
       !append_json_string(json, json_size, used, selection->group)))
    return false;
  if (selection->value &&
      (!append_json_separator(json, json_size, used, &first) ||
       !styled_append_string(json, json_size, used, "\"value\":") ||
       !append_json_string(json, json_size, used, selection->value)))
    return false;
  if (selection->toggle != STYLED_BOOLEAN_UNSET &&
      !append_json_boolean(json, json_size, used, &first, "toggle",
                           selection->toggle))
    return false;
  if (selection->selected != STYLED_BOOLEAN_UNSET &&
      !append_json_boolean(json, json_size, used, &first, "selected",
                           selection->selected))
    return false;
  if (selection->exclusive != STYLED_BOOLEAN_UNSET &&
      !append_json_boolean(json, json_size, used, &first, "exclusive",
                           selection->exclusive))
    return false;
  if (selection->disabled != STYLED_BOOLEAN_UNSET &&
      !append_json_boolean(json, json_size, used, &first, "disabled",
                           selection->disabled))
    return false;
  return styled_append_string(json, json_size, used, "}");
}

static bool build_config_json(const StyledLinkConfig *config, bool include_base,
                              bool include_states, bool include_tooltip,
                              bool include_menu, bool include_title,
                              bool include_title_style, bool include_visibility,
                              bool include_selection, bool include_spoiler,
                              bool include_disabled,
                              const StyledTextRenderOptions *options,
                              char *json, size_t json_size) {
  size_t used = 0;
  bool first = true;
  bool compact = options && options->osc_hyperlinks_compact;

  json[0] = '\0';
  if (!styled_append_string(json, json_size, &used, "{"))
    return false;
  if ((include_base || include_states) &&
      !append_json_style(json, json_size, &used, &first, &config->style,
                         include_base, include_states, compact))
    return false;
  if (include_tooltip &&
      (!append_json_separator(json, json_size, &used, &first) ||
       !styled_append_string(json, json_size, &used,
                             compact ? "\"t\":" : "\"tooltip\":") ||
       !append_json_string(json, json_size, &used, config->tooltip)))
    return false;
  if (include_menu && !append_json_menu(json, json_size, &used, &first, config,
                                        options, compact))
    return false;
  if (include_title && config->title &&
      !append_json_title(json, json_size, &used, &first, config,
                         include_title_style, compact))
    return false;
  if (include_visibility &&
      !append_json_visibility(json, json_size, &used, &first,
                              &config->visibility, compact))
    return false;
  if (include_selection &&
      !append_json_selection(json, json_size, &used, &first, &config->selection,
                             compact))
    return false;
  if (include_spoiler &&
      !append_json_boolean(json, json_size, &used, &first,
                           compact ? "sp" : "spoiler", config->spoiler))
    return false;
  if (include_disabled &&
      !append_json_boolean(json, json_size, &used, &first,
                           compact ? "d" : "disabled", config->disabled))
    return false;
  return styled_append_string(json, json_size, &used, "}");
}

static bool append_percent_encoded(const char *value, char *output,
                                   size_t output_size, size_t *used) {
  const size_t LENGTH = strlen(value);
  for (size_t index = 0; index < LENGTH; index++) {
    const unsigned char BYTE = osc8_byte(value, LENGTH, index);
    if (uri_unreserved(BYTE)) {
      if (!styled_append_bytes(output, output_size, used,
                               osc8_suffix(value, LENGTH, index), 1))
        return false;
    } else {
      char encoded[4];
      (void)snprintf(encoded, sizeof(encoded), "%%%02X", BYTE);
      if (!styled_append_bytes(output, output_size, used, encoded, 3))
        return false;
    }
  }
  return true;
}

bool styled_build_configured_uri(
    const char *uri, const StyledLinkConfig *config, bool include_base,
    bool include_states, bool include_tooltip, bool include_menu,
    bool include_title, bool include_title_style, bool include_visibility,
    bool include_selection, bool include_spoiler, bool include_disabled,
    const char *preset_name, bool append_config, bool reserve_config,
    bool reserve_preset, const StyledTextRenderOptions *options, char *output,
    size_t output_size) {
  constexpr char ENCODED_CONFIG[] = "%63%6F%6E%66%69%67";
  constexpr char ENCODED_PRESET[] = "%70%72%65%73%65%74";
  char json[4096];
  const size_t URI_LENGTH = strlen(uri);
  const char *fragment = strchr(uri, '#');
  const size_t MAIN_LENGTH =
      fragment ? URI_LENGTH - strlen(fragment) : URI_LENGTH;
  bool in_query = false;
  bool at_parameter_name = false;
  size_t used = 0;

  output[0] = '\0';
  if (append_config &&
      !build_config_json(config, include_base, include_states, include_tooltip,
                         include_menu, include_title, include_title_style,
                         include_visibility, include_selection, include_spoiler,
                         include_disabled, options, json, sizeof(json)))
    return false;
  for (size_t offset = 0; offset < MAIN_LENGTH;) {
    const char *cursor = osc8_suffix(uri, URI_LENGTH, offset);
    if (reserve_config && at_parameter_name && MAIN_LENGTH - offset >= 6 &&
        !memcmp(cursor, "config", 6) &&
        (offset + 6 == MAIN_LENGTH ||
         osc8_byte(uri, URI_LENGTH, offset + 6) == '=' ||
         osc8_byte(uri, URI_LENGTH, offset + 6) == '&')) {
      if (!styled_append_string(output, output_size, &used, ENCODED_CONFIG))
        return false;
      offset += 6;
      at_parameter_name = false;
      continue;
    }
    if (reserve_preset && at_parameter_name && MAIN_LENGTH - offset >= 6 &&
        !memcmp(cursor, "preset", 6) &&
        (offset + 6 == MAIN_LENGTH ||
         osc8_byte(uri, URI_LENGTH, offset + 6) == '=' ||
         osc8_byte(uri, URI_LENGTH, offset + 6) == '&')) {
      if (!styled_append_string(output, output_size, &used, ENCODED_PRESET))
        return false;
      offset += 6;
      at_parameter_name = false;
      continue;
    }
    char byte = (char)osc8_byte(uri, URI_LENGTH, offset++);
    if (!styled_append_bytes(output, output_size, &used, &byte, 1))
      return false;
    if (byte == '?') {
      in_query = true;
      at_parameter_name = true;
    } else if (in_query && byte == '&') {
      at_parameter_name = true;
    } else if (at_parameter_name && byte != '&') {
      at_parameter_name = false;
    }
  }
  if (preset_name &&
      (!styled_append_string(output, output_size, &used,
                             in_query ? "&preset=" : "?preset=") ||
       !append_percent_encoded(preset_name, output, output_size, &used)))
    return false;
  if (append_config &&
      (!styled_append_string(output, output_size, &used,
                             (in_query || preset_name) ? "&config="
                                                       : "?config=") ||
       !append_percent_encoded(json, output, output_size, &used)))
    return false;
  if (fragment && !styled_append_string(output, output_size, &used, fragment))
    return false;
  return used <= OSC8_URI_LIMIT;
}
