/* internal.h - Private shared types and interfaces for styled text. */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/styled_text/render.h"

typedef enum StyledColorKind : int {
  STYLED_COLOR_DEFAULT,
  STYLED_COLOR_RGB,
} StyledColorKind;

typedef struct StyledColor {
  StyledColorKind kind;
  int red;
  int green;
  int blue;
} StyledColor;

typedef struct StyledState {
  StyledColor foreground;
  StyledColor background;
  bool bold;
  bool italic;
  bool blink;
  bool underline;
  bool overline;
  bool strikethrough;
  bool inverse;
  bool has_link;
  bool link_emitted;
} StyledState;

typedef enum StyledDecoration : int {
  STYLED_DECORATION_UNSET,
  STYLED_DECORATION_FALSE,
  STYLED_DECORATION_TRUE,
  STYLED_DECORATION_WAVY,
  STYLED_DECORATION_DOTTED,
  STYLED_DECORATION_DASHED,
} StyledDecoration;

typedef enum StyledBoolean : int {
  STYLED_BOOLEAN_UNSET,
  STYLED_BOOLEAN_FALSE,
  STYLED_BOOLEAN_TRUE,
} StyledBoolean;

typedef struct StyledLinkProperties {
  StyledColor foreground;
  StyledColor background;
  StyledColor decoration_color;
  bool has_foreground;
  bool has_background;
  bool has_decoration_color;
  StyledBoolean bold;
  StyledBoolean italic;
  StyledDecoration underline;
  StyledDecoration overline;
  StyledDecoration strikethrough;
} StyledLinkProperties;

enum { STYLED_LINK_STATE_COUNT = 9 };

typedef struct StyledLinkStyle {
  StyledLinkProperties base;
  StyledLinkProperties states[STYLED_LINK_STATE_COUNT];
} StyledLinkStyle;

typedef enum StyledLinkKind : int {
  STYLED_LINK_EXTERNAL,
  STYLED_LINK_SEND,
  STYLED_LINK_PROMPT,
} StyledLinkKind;

typedef struct StyledLinkMenuItem {
  char *label;
  char *action;
  StyledLinkKind action_kind;
  bool has_action;
  bool separator;
} StyledLinkMenuItem;

typedef enum StyledVisibilityAction : int {
  STYLED_VISIBILITY_ACTION_UNSET,
  STYLED_VISIBILITY_ACTION_CONCEAL,
  STYLED_VISIBILITY_ACTION_REVEAL,
  STYLED_VISIBILITY_ACTION_REVEAL_CONCEAL,
} StyledVisibilityAction;

typedef struct StyledLinkVisibility {
  StyledVisibilityAction action;
  uint32_t delay;
  uint32_t output_delay;
  bool has_delay;
  bool has_output_delay;
  StyledBoolean expire_input;
  StyledBoolean expire_prompt;
  StyledBoolean expire_output;
  StyledBoolean wholeline;
} StyledLinkVisibility;

typedef struct StyledLinkSelection {
  char *group;
  char *value;
  StyledBoolean toggle;
  StyledBoolean selected;
  StyledBoolean exclusive;
  StyledBoolean disabled;
} StyledLinkSelection;

typedef struct StyledLinkConfig {
  StyledLinkStyle style;
  StyledLinkProperties title_style;
  char *tooltip;
  char *title;
  StyledLinkMenuItem *menu;
  size_t menu_count;
  StyledLinkVisibility visibility;
  StyledLinkSelection selection;
  StyledBoolean spoiler;
  StyledBoolean disabled;
  char *preset;
} StyledLinkConfig;

static inline StyledLinkMenuItem *
styled_link_menu_item_at(StyledLinkConfig *config, size_t index) {
  return checked_storage_at(config->menu, config->menu_count,
                            sizeof(*config->menu), index);
}

static inline const StyledLinkMenuItem *
styled_link_menu_item_at_const(const StyledLinkConfig *config, size_t index) {
  return checked_storage_at_const(config->menu, config->menu_count,
                                  sizeof(*config->menu), index);
}

static inline StyledLinkProperties *
styled_link_style_state(StyledLinkStyle *style, size_t index) {
  return checked_storage_at(style->states, STYLED_LINK_STATE_COUNT,
                            sizeof(*style->states), index);
}

static inline const StyledLinkProperties *
styled_link_style_state_const(const StyledLinkStyle *style, size_t index) {
  return checked_storage_at_const(style->states, STYLED_LINK_STATE_COUNT,
                                  sizeof(*style->states), index);
}

typedef struct StyledTextPreset {
  char *name;
  StyledLinkConfig config;
} StyledTextPreset;

typedef struct CustomNamedColor {
  char *name;
  int red;
  int green;
  int blue;
} CustomNamedColor;

struct StyledTextPalette {
  CustomNamedColor *colors;
  size_t count;
  size_t capacity;
  StyledTextPreset *presets;
  size_t preset_count;
  size_t preset_capacity;
};

static inline CustomNamedColor *styled_palette_color(StyledTextPalette *palette,
                                                     size_t index) {
  return checked_storage_at(palette->colors, palette->capacity,
                            sizeof(*palette->colors), index);
}

static inline const CustomNamedColor *
styled_palette_color_const(const StyledTextPalette *palette, size_t index) {
  return checked_storage_at_const(palette->colors, palette->capacity,
                                  sizeof(*palette->colors), index);
}

static inline StyledTextPreset *
styled_palette_preset(StyledTextPalette *palette, size_t index) {
  return checked_storage_at(palette->presets, palette->preset_capacity,
                            sizeof(*palette->presets), index);
}

static inline const StyledTextPreset *
styled_palette_preset_const(const StyledTextPalette *palette, size_t index) {
  return checked_storage_at_const(palette->presets, palette->preset_capacity,
                                  sizeof(*palette->presets), index);
}

enum {
  STYLE_STACK_LIMIT = 32,
  SGR_PARAMETER_LIMIT = 32,
  OSC8_URI_LIMIT = 4096,
  OSC8_CLOSE_SIZE = 7,
};

extern const char *const STYLED_LINK_STATE_NAMES[STYLED_LINK_STATE_COUNT];

bool styled_append_bytes(char *output, size_t output_size, size_t *used,
                         const char *value, size_t length);
bool styled_append_string(char *output, size_t output_size, size_t *used,
                          const char *value);
bool styled_append_utf8_codepoint(char *output, size_t output_size,
                                  size_t *used, const char *value,
                                  size_t *consumed);
void styled_set_error(char *error, size_t error_size, const char *message);
bool styled_emit_state(const StyledState *state, char *output,
                       size_t output_size, size_t *used);
size_t styled_output_size(const StyledState *state, size_t output_size);
bool styled_format_equal(const StyledState *left, const StyledState *right);
bool styled_sgr_parse(const char *cursor, const char **end, int *parameters,
                      size_t *parameter_count);
const char *styled_skip_escape(const char *cursor);

bool styled_color_parse(const StyledTextPalette *palette, const char *value,
                        StyledColor *color);

bool styled_style_directive_apply(const StyledTextPalette *palette,
                                  const char *directive, StyledState *state,
                                  char *error, size_t error_size);
bool styled_link_text_replace(char **destination, const char *value,
                              char *error, size_t error_size);
bool styled_link_directives_parse(
    const StyledTextPalette *palette, const char *directives, const char *end,
    StyledLinkConfig *config, StyledState *fallback, bool allow_ansi_fallback,
    bool validate_complete, char *error, size_t error_size);

void styled_link_config_destroy(StyledLinkConfig *config);
bool styled_link_config_valid(const StyledLinkConfig *config, char *error,
                              size_t error_size);
bool styled_link_preset_config_valid(const StyledLinkConfig *config,
                                     char *error, size_t error_size);
bool styled_link_config_copy(StyledLinkConfig *destination,
                             const StyledLinkConfig *source);
bool styled_link_config_merge(StyledLinkConfig *base,
                              const StyledLinkConfig *overlay);
void styled_link_fallback_apply(const StyledLinkProperties *properties,
                                StyledState *state);
bool styled_link_properties_present(const StyledLinkProperties *properties);
bool styled_link_visibility_present(const StyledLinkVisibility *visibility);
bool styled_link_selection_present(const StyledLinkSelection *selection);
bool styled_link_config_present(const StyledLinkConfig *config);
bool styled_link_has_states(const StyledLinkConfig *config);

bool styled_link_target_unquote(const char *start, const char *end,
                                char *target, size_t target_size,
                                const char **remainder, char *error,
                                size_t error_size);
bool styled_external_uri_valid(const char *uri, char *error, size_t error_size);
bool styled_command_uri_encode(StyledLinkKind kind, const char *command,
                               char *uri, size_t uri_size, char *error,
                               size_t error_size);
bool styled_link_enabled(StyledLinkKind kind,
                         const StyledTextRenderOptions *options);
bool styled_config_capability_advertised(
    const StyledTextRenderOptions *options);
bool styled_emit_link_open(const char *uri, char *output, size_t output_size,
                           size_t *used);
bool styled_emit_link_close(char *output, size_t output_size, size_t *used);
bool styled_link_menu_has_enabled_action(
    const StyledLinkConfig *config, const StyledTextRenderOptions *options);
bool styled_build_configured_uri(
    const char *uri, const StyledLinkConfig *config, bool include_base,
    bool include_states, bool include_tooltip, bool include_menu,
    bool include_title, bool include_title_style, bool include_visibility,
    bool include_selection, bool include_spoiler, bool include_disabled,
    const char *preset_name, bool append_config, bool reserve_config,
    bool reserve_preset, const StyledTextRenderOptions *options, char *output,
    size_t output_size);

bool styled_text_preset_name_valid(const char *name);
const StyledTextPreset *
styled_text_palette_find_preset(const StyledTextPalette *palette,
                                const char *name);

const char *styled_find_tag_close(const char *start);
void styled_text_compile_permissive(const StyledTextPalette *palette,
                                    const char *input, char *output,
                                    size_t output_size,
                                    const StyledTextRenderOptions *options);
