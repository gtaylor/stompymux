/* link_directives.c - Styled-text and OSC 8 directive parsing. */

#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/support/styled_text/internal.h"
#include "mux/support/styled_text/palette.h"
#include "mux/support/utf8.h"

const char *const styled_link_state_names[STYLED_LINK_STATE_COUNT] = {
    "active",   "hover",    "focus-visible", "focus",    "visited",
    "selected", "disabled", "link",          "any-link",
};

static bool parse_styled_boolean(const char *value, StyledBoolean *result) {
  if (value == nullptr || !strcasecmp(value, "true")) {
    *result = STYLED_BOOLEAN_TRUE;
    return true;
  }
  if (!strcasecmp(value, "false")) {
    *result = STYLED_BOOLEAN_FALSE;
    return true;
  }
  return false;
}

static bool parse_styled_decoration(const char *value,
                                    StyledDecoration *result) {
  if (value == nullptr || !strcasecmp(value, "true")) {
    *result = STYLED_DECORATION_TRUE;
    return true;
  }
  if (!strcasecmp(value, "false"))
    *result = STYLED_DECORATION_FALSE;
  else if (!strcasecmp(value, "wavy"))
    *result = STYLED_DECORATION_WAVY;
  else if (!strcasecmp(value, "dotted"))
    *result = STYLED_DECORATION_DOTTED;
  else if (!strcasecmp(value, "dashed"))
    *result = STYLED_DECORATION_DASHED;
  else
    return false;
  return true;
}

static bool apply_link_properties(const StyledTextPalette *palette,
                                  const char *property, const char *value,
                                  StyledLinkProperties *properties, char *error,
                                  size_t error_size) {
  StyledColor color;

  if (!strcasecmp(property, "color") || !strcasecmp(property, "fg")) {
    if (!value || !*value || !styled_color_parse(palette, value, &color)) {
      styled_set_error(error, error_size, "unknown foreground color");
      return false;
    }
    properties->foreground = color;
    properties->has_foreground = true;
  } else if (!strcasecmp(property, "bg")) {
    if (!value || !*value || !styled_color_parse(palette, value, &color)) {
      styled_set_error(error, error_size, "unknown background color");
      return false;
    }
    properties->background = color;
    properties->has_background = true;
  } else if (!strcasecmp(property, "text-decoration-color")) {
    if (!value || !*value || !styled_color_parse(palette, value, &color)) {
      styled_set_error(error, error_size, "unknown text decoration color");
      return false;
    }
    properties->decoration_color = color;
    properties->has_decoration_color = true;
  } else if (!strcasecmp(property, "bold")) {
    if (!parse_styled_boolean(value, &properties->bold))
      goto invalid_value;
  } else if (!strcasecmp(property, "italic")) {
    if (!parse_styled_boolean(value, &properties->italic))
      goto invalid_value;
  } else if (!strcasecmp(property, "underline")) {
    if (!parse_styled_decoration(value, &properties->underline))
      goto invalid_value;
  } else if (!strcasecmp(property, "overline")) {
    if (!parse_styled_decoration(value, &properties->overline))
      goto invalid_value;
  } else if (!strcasecmp(property, "strikethrough")) {
    if (!parse_styled_decoration(value, &properties->strikethrough))
      goto invalid_value;
  } else {
    styled_set_error(error, error_size, "unknown OSC 8 style property");
    return false;
  }
  return true;

invalid_value:
  styled_set_error(error, error_size, "invalid OSC 8 style property value");
  return false;
}

static bool apply_link_property(const StyledTextPalette *palette,
                                const char *property, const char *value,
                                StyledLinkStyle *style, char *error,
                                size_t error_size) {
  char state[64];
  const char *dot = strchr(property, '.');
  StyledLinkProperties *properties = &style->base;

  if (!dot)
    return apply_link_properties(palette, property, value, properties, error,
                                 error_size);
  if (dot == property || dot[1] == '\0' || strchr(dot + 1, '.') ||
      (size_t)(dot - property) >= sizeof(state)) {
    styled_set_error(error, error_size, "unknown OSC 8 style state");
    return false;
  }
  memcpy(state, property, (size_t)(dot - property));
  state[dot - property] = '\0';
  properties = nullptr;
  for (size_t index = 0; index < STYLED_LINK_STATE_COUNT; index++) {
    if (!strcasecmp(state, styled_link_state_names[index])) {
      properties = &style->states[index];
      break;
    }
  }
  if (!properties) {
    styled_set_error(error, error_size, "unknown OSC 8 style state");
    return false;
  }
  return apply_link_properties(palette, dot + 1, value, properties, error,
                               error_size);
}

static bool next_link_directive(const char **cursor, const char *end,
                                char *name, size_t name_size, char *value,
                                size_t value_size, const char **parsed_value,
                                bool *quoted, char *error, size_t error_size) {
  const char *start;
  size_t name_length;
  size_t used = 0;

  while (*cursor < end && isspace((unsigned char)**cursor))
    (*cursor)++;
  if (*cursor == end)
    return false;
  start = *cursor;
  while (*cursor < end && !isspace((unsigned char)**cursor) && **cursor != '=')
    (*cursor)++;
  name_length = (size_t)(*cursor - start);
  if (name_length == 0 || name_length >= name_size) {
    styled_set_error(error, error_size, "invalid OSC 8 link property");
    return false;
  }
  memcpy(name, start, name_length);
  name[name_length] = '\0';
  *parsed_value = nullptr;
  *quoted = false;
  if (*cursor == end || isspace((unsigned char)**cursor))
    return true;

  (*cursor)++;
  if (*cursor == end) {
    styled_set_error(error, error_size, "OSC 8 link property value is empty");
    return false;
  }
  if (**cursor == '"') {
    *quoted = true;
    (*cursor)++;
    while (*cursor < end && **cursor != '"') {
      unsigned char byte = (unsigned char)*(*cursor)++;

      if (byte == '\\') {
        if (*cursor == end || (**cursor != '\\' && **cursor != '"')) {
          styled_set_error(error, error_size,
                           "invalid escape in OSC 8 link property");
          return false;
        }
        byte = (unsigned char)*(*cursor)++;
      }
      if (byte < 0x20 || byte == 0x7f || used + 1 >= value_size) {
        styled_set_error(error, error_size,
                         byte < 0x20 || byte == 0x7f
                             ? "OSC 8 link property contains a control byte"
                             : "OSC 8 link property value is too long");
        return false;
      }
      value[used++] = (char)byte;
    }
    if (*cursor == end || **cursor != '"') {
      styled_set_error(error, error_size,
                       "unterminated OSC 8 link property value");
      return false;
    }
    (*cursor)++;
    if (*cursor < end && !isspace((unsigned char)**cursor)) {
      styled_set_error(error, error_size,
                       "unexpected text after quoted OSC 8 value");
      return false;
    }
  } else {
    while (*cursor < end && !isspace((unsigned char)**cursor)) {
      if (used + 1 >= value_size) {
        styled_set_error(error, error_size,
                         "OSC 8 link property value is too long");
        return false;
      }
      value[used++] = *(*cursor)++;
    }
  }
  value[used] = '\0';
  if (!utf8_validate_printable(value, used)) {
    styled_set_error(error, error_size,
                     "OSC 8 link property must be printable, valid UTF-8");
    return false;
  }
  *parsed_value = value;
  return true;
}

static bool parse_menu_property(const char *property, size_t *index,
                                const char **field) {
  const char *cursor;
  size_t value = 0;

  if (strncasecmp(property, "menu.", 5))
    return false;
  cursor = property + 5;
  if (!isdigit((unsigned char)*cursor))
    return false;
  while (isdigit((unsigned char)*cursor)) {
    if (value > OSC8_URI_LIMIT)
      return false;
    value = value * 10 + (size_t)(*cursor++ - '0');
  }
  if (value == 0 || value > OSC8_URI_LIMIT || *cursor != '.' || !cursor[1])
    return false;
  *index = value - 1;
  *field = cursor + 1;
  return true;
}

static StyledLinkMenuItem *styled_link_menu_item(StyledLinkConfig *config,
                                                 size_t index, char *error,
                                                 size_t error_size) {
  if (index >= config->menu_count) {
    size_t new_count = index + 1;
    StyledLinkMenuItem *menu =
        realloc(config->menu, new_count * sizeof(*config->menu));

    if (!menu) {
      styled_set_error(error, error_size,
                       "out of memory parsing OSC 8 context menu");
      return nullptr;
    }
    memset(menu + config->menu_count, 0,
           (new_count - config->menu_count) * sizeof(*menu));
    config->menu = menu;
    config->menu_count = new_count;
  }
  return &config->menu[index];
}

static bool apply_menu_property(StyledLinkConfig *config, size_t index,
                                const char *field, const char *value,
                                bool quoted, char *error, size_t error_size) {
  StyledLinkMenuItem *item =
      styled_link_menu_item(config, index, error, error_size);
  StyledLinkKind action_kind;

  if (!item)
    return false;
  if (!strcasecmp(field, "separator")) {
    if (value || item->separator || item->label || item->has_action) {
      styled_set_error(error, error_size, "invalid OSC 8 menu separator");
      return false;
    }
    item->separator = true;
    return true;
  }
  if (item->separator || !quoted || !value || !*value) {
    styled_set_error(error, error_size, "invalid OSC 8 menu item property");
    return false;
  }
  if (!strcasecmp(field, "label")) {
    if (item->label) {
      styled_set_error(error, error_size, "duplicate OSC 8 menu item label");
      return false;
    }
    return styled_link_text_replace(&item->label, value, error, error_size);
  }
  if (!strcasecmp(field, "link"))
    action_kind = STYLED_LINK_EXTERNAL;
  else if (!strcasecmp(field, "send"))
    action_kind = STYLED_LINK_SEND;
  else if (!strcasecmp(field, "prompt"))
    action_kind = STYLED_LINK_PROMPT;
  else {
    styled_set_error(error, error_size, "unknown OSC 8 menu item property");
    return false;
  }
  if (item->has_action) {
    styled_set_error(error, error_size, "duplicate OSC 8 menu item action");
    return false;
  }
  if (action_kind == STYLED_LINK_EXTERNAL &&
      !styled_external_uri_valid(value, error, error_size))
    return false;
  if (!styled_link_text_replace(&item->action, value, error, error_size))
    return false;
  item->action_kind = action_kind;
  item->has_action = true;
  return true;
}

static bool parse_uint32_milliseconds(const char *value, bool quoted,
                                      uint32_t *result) {
  uint64_t parsed = 0;

  if (quoted || !value || !*value)
    return false;
  for (const unsigned char *cursor = (const unsigned char *)value; *cursor;
       cursor++) {
    if (!isdigit(*cursor))
      return false;
    uint64_t digit = (uint64_t)(*cursor - '0');
    if (parsed > (UINT32_MAX - digit) / 10)
      return false;
    parsed = parsed * 10 + digit;
  }
  *result = (uint32_t)parsed;
  return true;
}

static bool apply_visibility_property(StyledLinkVisibility *visibility,
                                      const char *property, const char *value,
                                      bool quoted, char *error,
                                      size_t error_size) {
  StyledBoolean boolean;

  if (!strcasecmp(property, "action")) {
    if (quoted || !value)
      goto invalid_value;
    if (!strcasecmp(value, "conceal"))
      visibility->action = STYLED_VISIBILITY_ACTION_CONCEAL;
    else if (!strcasecmp(value, "reveal"))
      visibility->action = STYLED_VISIBILITY_ACTION_REVEAL;
    else if (!strcasecmp(value, "reveal,conceal"))
      visibility->action = STYLED_VISIBILITY_ACTION_REVEAL_CONCEAL;
    else
      goto invalid_value;
    return true;
  }
  if (!strcasecmp(property, "delay")) {
    if (!parse_uint32_milliseconds(value, quoted, &visibility->delay))
      goto invalid_value;
    visibility->has_delay = true;
    return true;
  }
  if (!strcasecmp(property, "wholeline")) {
    if (quoted || !parse_styled_boolean(value, &visibility->wholeline))
      goto invalid_value;
    return true;
  }
  if (!strncasecmp(property, "expire.", 7)) {
    const char *field = property + 7;

    if (!strcasecmp(field, "outputDelay")) {
      if (!parse_uint32_milliseconds(value, quoted, &visibility->output_delay))
        goto invalid_value;
      visibility->has_output_delay = true;
      return true;
    }
    if (quoted || !parse_styled_boolean(value, &boolean))
      goto invalid_value;
    if (!strcasecmp(field, "input"))
      visibility->expire_input = boolean;
    else if (!strcasecmp(field, "prompt"))
      visibility->expire_prompt = boolean;
    else if (!strcasecmp(field, "output"))
      visibility->expire_output = boolean;
    else {
      styled_set_error(error, error_size, "unknown OSC 8 visibility expiry");
      return false;
    }
    return true;
  }
  styled_set_error(error, error_size, "unknown OSC 8 visibility property");
  return false;

invalid_value:
  styled_set_error(error, error_size,
                   "invalid OSC 8 visibility property value");
  return false;
}

static bool apply_selection_property(StyledLinkSelection *selection,
                                     const char *property, const char *value,
                                     bool quoted, char *error,
                                     size_t error_size) {
  if (!strcasecmp(property, "group") || !strcasecmp(property, "value")) {
    char **destination =
        !strcasecmp(property, "group") ? &selection->group : &selection->value;

    if (!quoted || !value || !*value) {
      styled_set_error(error, error_size,
                       "OSC 8 selection text must be non-empty and quoted");
      return false;
    }
    return styled_link_text_replace(destination, value, error, error_size);
  }

  StyledBoolean *destination;
  if (!strcasecmp(property, "toggle"))
    destination = &selection->toggle;
  else if (!strcasecmp(property, "selected"))
    destination = &selection->selected;
  else if (!strcasecmp(property, "exclusive"))
    destination = &selection->exclusive;
  else if (!strcasecmp(property, "disabled"))
    destination = &selection->disabled;
  else {
    styled_set_error(error, error_size, "unknown OSC 8 selection property");
    return false;
  }
  if (quoted || !parse_styled_boolean(value, destination)) {
    styled_set_error(error, error_size, "invalid OSC 8 selection boolean");
    return false;
  }
  return true;
}

static bool apply_link_config_property(const StyledTextPalette *palette,
                                       StyledLinkConfig *config,
                                       const char *property, const char *value,
                                       bool quoted, char *error,
                                       size_t error_size) {
  size_t menu_index;
  const char *menu_field;

  if (!strcasecmp(property, "preset")) {
    if (!quoted || !value || !*value) {
      styled_set_error(error, error_size,
                       "OSC 8 preset name must be non-empty and quoted");
      return false;
    }
    if (config->preset) {
      styled_set_error(error, error_size, "duplicate OSC 8 preset reference");
      return false;
    }
    return styled_link_text_replace(&config->preset, value, error, error_size);
  }

  if (!strcasecmp(property, "tooltip") || !strcasecmp(property, "title")) {
    char **destination =
        !strcasecmp(property, "tooltip") ? &config->tooltip : &config->title;
    if (!quoted || !value || !*value) {
      styled_set_error(error, error_size,
                       "OSC 8 text property must be non-empty and quoted");
      return false;
    }
    return styled_link_text_replace(destination, value, error, error_size);
  }
  if (!strncasecmp(property, "title.", 6))
    return apply_link_properties(palette, property + 6, value,
                                 &config->title_style, error, error_size);
  if (parse_menu_property(property, &menu_index, &menu_field))
    return apply_menu_property(config, menu_index, menu_field, value, quoted,
                               error, error_size);
  if (!strncasecmp(property, "menu.", 5)) {
    styled_set_error(error, error_size, "invalid OSC 8 menu item index");
    return false;
  }
  if (!strncasecmp(property, "visibility.", 11))
    return apply_visibility_property(&config->visibility, property + 11, value,
                                     quoted, error, error_size);
  if (!strncasecmp(property, "selection.", 10))
    return apply_selection_property(&config->selection, property + 10, value,
                                    quoted, error, error_size);
  if (!strcasecmp(property, "spoiler") || !strcasecmp(property, "disabled")) {
    StyledBoolean *destination =
        !strcasecmp(property, "spoiler") ? &config->spoiler : &config->disabled;

    if (quoted || !parse_styled_boolean(value, destination)) {
      styled_set_error(error, error_size, "invalid OSC 8 behavior boolean");
      return false;
    }
    return true;
  }
  return apply_link_property(palette, property, value, &config->style, error,
                             error_size);
}

bool styled_style_directive_apply(const StyledTextPalette *palette,
                                  const char *directive, StyledState *state,
                                  char *error, size_t error_size) {
  StyledColor color;
  const char *value = strchr(directive, '=');
  size_t name_length = value ? (size_t)(value - directive) : strlen(directive);
  StyledBoolean boolean;
  StyledDecoration decoration;

  if (value)
    value++;

  if (name_length == 4 && !strncasecmp(directive, "bold", name_length)) {
    if (!parse_styled_boolean(value, &boolean))
      goto invalid_value;
    state->bold = boolean == STYLED_BOOLEAN_TRUE;
  } else if (name_length == 6 &&
             !strncasecmp(directive, "italic", name_length)) {
    if (!parse_styled_boolean(value, &boolean))
      goto invalid_value;
    state->italic = boolean == STYLED_BOOLEAN_TRUE;
  } else if (!strcasecmp(directive, "blink")) {
    state->blink = true;
  } else if (name_length == 9 &&
             !strncasecmp(directive, "underline", name_length)) {
    if (!parse_styled_decoration(value, &decoration))
      goto invalid_value;
    state->underline = decoration != STYLED_DECORATION_FALSE;
  } else if (name_length == 8 &&
             !strncasecmp(directive, "overline", name_length)) {
    if (!parse_styled_decoration(value, &decoration))
      goto invalid_value;
    state->overline = decoration != STYLED_DECORATION_FALSE;
  } else if (name_length == 13 &&
             !strncasecmp(directive, "strikethrough", name_length)) {
    if (!parse_styled_decoration(value, &decoration))
      goto invalid_value;
    state->strikethrough = decoration != STYLED_DECORATION_FALSE;
  } else if (!strcasecmp(directive, "inverse")) {
    state->inverse = true;
  } else if ((name_length == 2 && !strncasecmp(directive, "fg", 2)) ||
             (name_length == 5 && !strncasecmp(directive, "color", 5))) {
    if (!value || !styled_color_parse(palette, value, &color)) {
      styled_set_error(error, error_size, "unknown foreground color");
      return false;
    }
    state->foreground = color;
  } else if (name_length == 2 && !strncasecmp(directive, "bg", 2)) {
    if (!value || !styled_color_parse(palette, value, &color)) {
      styled_set_error(error, error_size, "unknown background color");
      return false;
    }
    state->background = color;
  } else if (name_length == 21 &&
             !strncasecmp(directive, "text-decoration-color", name_length)) {
    if (!value || !styled_color_parse(palette, value, &color)) {
      styled_set_error(error, error_size, "unknown text decoration color");
      return false;
    }
  } else if (!strcmp(directive, "/") || !strcasecmp(directive, "reset")) {
    styled_set_error(error, error_size,
                     "style close and reset tags cannot be combined");
    return false;
  } else {
    styled_set_error(error, error_size, "unknown style tag");
    return false;
  }
  return true;

invalid_value:
  styled_set_error(error, error_size, "invalid style property value");
  return false;
}

bool styled_link_directives_parse(
    const StyledTextPalette *palette, const char *directives, const char *end,
    StyledLinkConfig *config, StyledState *fallback, bool allow_ansi_fallback,
    bool validate_complete, char *error, size_t error_size) {
  while (directives < end) {
    char name[64];
    char value[OSC8_URI_LIMIT + 1];
    const char *parsed_value;
    bool quoted;

    while (directives < end && isspace((unsigned char)*directives))
      directives++;
    if (directives == end)
      break;
    if (!next_link_directive(&directives, end, name, sizeof(name), value,
                             sizeof(value), &parsed_value, &quoted, error,
                             error_size))
      return false;
    if ((!strcasecmp(name, "blink") || !strcasecmp(name, "inverse")) &&
        !parsed_value) {
      if (!allow_ansi_fallback) {
        styled_set_error(
            error, error_size,
            "ANSI-only properties are not valid in an OSC 8 preset");
        return false;
      }
      if (!styled_style_directive_apply(palette, name, fallback, error,
                                        error_size))
        return false;
    } else if (!apply_link_config_property(palette, config, name, parsed_value,
                                           quoted, error, error_size)) {
      return false;
    }
  }
  return validate_complete
             ? styled_link_config_valid(config, error, error_size)
             : styled_link_preset_config_valid(config, error, error_size);
}
