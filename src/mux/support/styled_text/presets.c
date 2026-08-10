/* presets.c - Session-scoped OSC 8 preset catalog. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/internal.h"
#include "mux/support/styled_text/palette.h"
#include "mux/support/styled_text/render.h"

bool styled_text_preset_name_valid(const char *name) {
  size_t length;

  if (!name || !*name)
    return false;
  length = strlen(name);
  unsigned char first = (unsigned char)*(const char *)checked_storage_at_const(
      name, length + 1, sizeof(char), 0);
  if (length > 60 ||
      !((first >= 'A' && first <= 'Z') || (first >= 'a' && first <= 'z') ||
        (first >= '0' && first <= '9')))
    return false;
  for (size_t index = 0; index < length; index++) {
    const unsigned char character =
        (unsigned char)*(const char *)checked_storage_at_const(
            name, length + 1, sizeof(char), index);
    bool alphanumeric = (character >= 'A' && character <= 'Z') ||
                        (character >= 'a' && character <= 'z') ||
                        (character >= '0' && character <= '9');
    if (!alphanumeric && character != '.' && character != '_' &&
        character != '~' && character != '-')
      return false;
  }
  return true;
}

const StyledTextPreset *
styled_text_palette_find_preset(const StyledTextPalette *palette,
                                const char *name) {
  if (!palette || !name)
    return nullptr;
  for (size_t index = 0; index < palette->preset_count; index++) {
    const StyledTextPreset *preset =
        styled_palette_preset_const(palette, index);
    if (!strcmp(preset->name, name))
      return preset;
  }
  return nullptr;
}

static bool styled_text_preset_uri(const StyledTextPreset *preset,
                                   const StyledTextRenderOptions *options,
                                   char *uri, size_t uri_size) {
  const StyledLinkConfig *config = &preset->config;
  bool include_base = options && options->osc_hyperlinks_style_basic &&
                      styled_link_properties_present(&config->style.base);
  bool include_states = options && options->osc_hyperlinks_style_states &&
                        styled_link_has_states(config);
  bool include_tooltip =
      options && options->osc_hyperlinks_tooltip && config->tooltip;
  bool include_menu = options && options->osc_hyperlinks_menu &&
                      styled_link_menu_has_enabled_action(config, options);
  bool include_title = options && options->osc_hyperlinks_menu && config->title;
  bool include_title_style =
      include_title && options->osc_hyperlinks_style_basic &&
      styled_link_properties_present(&config->title_style);
  bool include_visibility = options && options->osc_hyperlinks_visibility &&
                            styled_link_visibility_present(&config->visibility);
  bool include_selection = options && options->osc_hyperlinks_selection &&
                           styled_link_selection_present(&config->selection);
  bool include_spoiler = options && options->osc_hyperlinks_spoiler &&
                         config->spoiler != STYLED_BOOLEAN_UNSET;
  bool include_disabled = options && options->osc_hyperlinks_disabled &&
                          config->disabled != STYLED_BOOLEAN_UNSET;
  char base[80];
  int length = snprintf(base, sizeof(base), "preset:%s", preset->name);

  return length > 0 && (size_t)length < sizeof(base) &&
         styled_build_configured_uri(
             base, config, include_base, include_states, include_tooltip,
             include_menu, include_title, include_title_style,
             include_visibility, include_selection, include_spoiler,
             include_disabled, nullptr, true, false, false, options, uri,
             uri_size);
}

bool styled_text_palette_set_preset(StyledTextPalette *palette,
                                    const StyledPresetDefinition *definition) {
  const char *name = definition->name;
  const char *directives = definition->directives;
  char *error = definition->error;
  size_t error_size = definition->error_size;
  static const StyledTextRenderOptions all_options = {
      .osc_hyperlinks = true,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_prompt = true,
      .osc_hyperlinks_style_basic = true,
      .osc_hyperlinks_style_states = true,
      .osc_hyperlinks_tooltip = true,
      .osc_hyperlinks_menu = true,
      .osc_hyperlinks_visibility = true,
      .osc_hyperlinks_spoiler = true,
      .osc_hyperlinks_disabled = true,
      .osc_hyperlinks_selection = true,
  };
  StyledLinkConfig config = {0};
  StyledState unused = {0};
  char uri[OSC8_URI_LIMIT + 1];
  char candidate_name[61];
  size_t position = 0;

  if (error && error_size > 0)
    error[0] = '\0';
  if (!palette || !directives) {
    styled_set_error(error, error_size,
                     "OSC 8 preset catalog is not available");
    return false;
  }
  if (!styled_text_preset_name_valid(name)) {
    styled_set_error(error, error_size,
                     "preset name must use 1-60 URI-safe ASCII characters");
    return false;
  }
  if (!styled_link_directives_parse(
          palette, directives,
          checked_string_suffix(directives, strlen(directives)), &config,
          &unused, false, false, error, error_size))
    goto fail;
  if (config.preset) {
    styled_set_error(error, error_size, "OSC 8 presets cannot inherit presets");
    goto fail;
  }
  if (!styled_link_config_present(&config)) {
    styled_set_error(error, error_size, "OSC 8 preset must not be empty");
    goto fail;
  }
  (void)snprintf(candidate_name, sizeof(candidate_name), "%s", name);
  StyledTextPreset candidate = {.name = candidate_name, .config = config};
  if (!styled_text_preset_uri(&candidate, &all_options, uri, sizeof(uri))) {
    styled_set_error(error, error_size, "OSC 8 preset URI is too long");
    goto fail;
  }
  while (position < palette->preset_count &&
         strcmp(styled_palette_preset(palette, position)->name, name) < 0)
    position++;
  if (position < palette->preset_count &&
      !strcmp(styled_palette_preset(palette, position)->name, name)) {
    StyledTextPreset *preset = styled_palette_preset(palette, position);
    styled_link_config_destroy(&preset->config);
    preset->config = config;
    return true;
  }
  if (palette->preset_count == palette->preset_capacity) {
    size_t capacity =
        palette->preset_capacity ? palette->preset_capacity * 2 : 8;
    StyledTextPreset *presets =
        realloc(palette->presets, capacity * sizeof(*presets));
    if (!presets) {
      styled_set_error(error, error_size, "unable to allocate OSC 8 preset");
      goto fail;
    }
    palette->presets = presets;
    palette->preset_capacity = capacity;
  }
  if (position < palette->preset_count)
    memmove(styled_palette_preset(palette, position + 1),
            styled_palette_preset(palette, position),
            (palette->preset_count - position) * sizeof(*palette->presets));
  *styled_palette_preset(palette, position) =
      (StyledTextPreset){.name = strdup(name), .config = config};
  if (!styled_palette_preset(palette, position)->name) {
    if (position < palette->preset_count)
      memmove(styled_palette_preset(palette, position),
              styled_palette_preset(palette, position + 1),
              (palette->preset_count - position) * sizeof(*palette->presets));
    styled_set_error(error, error_size, "unable to allocate OSC 8 preset");
    goto fail;
  }
  palette->preset_count++;
  return true;

fail:
  styled_link_config_destroy(&config);
  return false;
}

size_t styled_text_palette_preset_count(const StyledTextPalette *palette) {
  return palette ? palette->preset_count : 0;
}

bool styled_text_palette_render_preset(const StyledTextPalette *palette,
                                       size_t index,
                                       const StyledTextRenderOptions *options,
                                       char *output, size_t output_size) {
  char uri[OSC8_URI_LIMIT + 1];
  size_t used = 0;

  if (!output || output_size == 0)
    return false;
  output[0] = '\0';
  if (!palette || index >= palette->preset_count || !options ||
      !options->osc_hyperlinks_presets ||
      !styled_text_preset_uri(styled_palette_preset_const(palette, index),
                              options, uri, sizeof(uri)))
    return false;
  return styled_emit_link_open(uri, output, output_size, &used) &&
         styled_emit_link_close(output, output_size, &used);
}
