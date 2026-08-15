/* link_config.c - OSC 8 configuration ownership and merge semantics. */

#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/internal.h"

bool styled_link_text_replace(char **destination, const char *value,
                              char *error, size_t error_size) {
  char *copy = strdup(value);

  if (!copy) {
    styled_set_error(error, error_size,
                     "out of memory parsing OSC 8 configuration");
    return false;
  }
  free(*destination);
  *destination = copy;
  return true;
}

void styled_link_config_destroy(StyledLinkConfig *config) {
  if (!config)
    return;
  free(config->tooltip);
  free(config->title);
  free(config->selection.group);
  free(config->selection.value);
  free(config->preset);
  for (size_t index = 0; index < config->menu_count; index++) {
    StyledLinkMenuItem *item = styled_link_menu_item_at(config, index);
    free(item->label);
    free(item->action);
  }
  free(config->menu);
  *config = (StyledLinkConfig){0};
}

bool styled_link_visibility_present(const StyledLinkVisibility *visibility) {
  return visibility->action != STYLED_VISIBILITY_ACTION_UNSET ||
         visibility->has_delay || visibility->has_output_delay ||
         visibility->expire_input != STYLED_BOOLEAN_UNSET ||
         visibility->expire_prompt != STYLED_BOOLEAN_UNSET ||
         visibility->expire_output != STYLED_BOOLEAN_UNSET ||
         visibility->wholeline != STYLED_BOOLEAN_UNSET;
}

bool styled_link_selection_present(const StyledLinkSelection *selection) {
  return selection->group || selection->value ||
         selection->toggle != STYLED_BOOLEAN_UNSET ||
         selection->selected != STYLED_BOOLEAN_UNSET ||
         selection->exclusive != STYLED_BOOLEAN_UNSET ||
         selection->disabled != STYLED_BOOLEAN_UNSET;
}

bool styled_link_config_valid(const StyledLinkConfig *config, char *error,
                              size_t error_size) {
  const StyledLinkVisibility *visibility = &config->visibility;

  if (config->title && config->menu_count == 0) {
    styled_set_error(error, error_size, "OSC 8 menu title requires a menu");
    return false;
  }
  if (styled_link_properties_present(&config->title_style) && !config->title) {
    styled_set_error(error, error_size,
                     "OSC 8 menu title style requires title text");
    return false;
  }
  for (size_t index = 0; index < config->menu_count; index++) {
    const StyledLinkMenuItem *item =
        styled_link_menu_item_at_const(config, index);

    if (item->separator)
      continue;
    if (!item->label && !item->has_action) {
      styled_set_error(error, error_size,
                       "OSC 8 menu indices must be contiguous");
      return false;
    }
    if (!item->label || !item->has_action) {
      styled_set_error(error, error_size, "incomplete OSC 8 menu item");
      return false;
    }
  }
  if (styled_link_visibility_present(visibility) &&
      visibility->action == STYLED_VISIBILITY_ACTION_UNSET) {
    styled_set_error(error, error_size, "OSC 8 visibility requires an action");
    return false;
  }
  bool have_expire = visibility->expire_input != STYLED_BOOLEAN_UNSET ||
                     visibility->expire_prompt != STYLED_BOOLEAN_UNSET ||
                     visibility->expire_output != STYLED_BOOLEAN_UNSET ||
                     visibility->has_output_delay;
  bool enabled_expire = visibility->expire_input == STYLED_BOOLEAN_TRUE ||
                        visibility->expire_prompt == STYLED_BOOLEAN_TRUE ||
                        visibility->expire_output == STYLED_BOOLEAN_TRUE;
  if (have_expire && !enabled_expire) {
    styled_set_error(error, error_size,
                     "OSC 8 visibility expire requires an enabled trigger");
    return false;
  }
  if (visibility->has_output_delay &&
      visibility->expire_output != STYLED_BOOLEAN_TRUE) {
    styled_set_error(error, error_size,
                     "OSC 8 visibility outputDelay requires output expiry");
    return false;
  }
  if (styled_link_selection_present(&config->selection) &&
      (!config->selection.group || !config->selection.value)) {
    styled_set_error(error, error_size,
                     "OSC 8 selection requires both group and value");
    return false;
  }
  return true;
}

bool styled_link_preset_config_valid(const StyledLinkConfig *config,
                                     char *error, size_t error_size) {
  for (size_t index = 0; index < config->menu_count; index++) {
    const StyledLinkMenuItem *item =
        styled_link_menu_item_at_const(config, index);

    if (item->separator)
      continue;
    if (!item->label && !item->has_action) {
      styled_set_error(error, error_size,
                       "OSC 8 menu indices must be contiguous");
      return false;
    }
    if (!item->label || !item->has_action) {
      styled_set_error(error, error_size, "incomplete OSC 8 menu item");
      return false;
    }
  }
  return true;
}

static bool copy_link_text(char **destination, const char *source) {
  if (!source)
    return true;
  *destination = strdup(source);
  return *destination != nullptr;
}

bool styled_link_config_copy(StyledLinkConfig *destination,
                             const StyledLinkConfig *source) {
  *destination = *source;
  destination->tooltip = nullptr;
  destination->title = nullptr;
  destination->menu = nullptr;
  destination->menu_count = 0;
  destination->selection.group = nullptr;
  destination->selection.value = nullptr;
  destination->preset = nullptr;
  if (!copy_link_text(&destination->tooltip, source->tooltip) ||
      !copy_link_text(&destination->title, source->title) ||
      !copy_link_text(&destination->selection.group, source->selection.group) ||
      !copy_link_text(&destination->selection.value, source->selection.value) ||
      !copy_link_text(&destination->preset, source->preset))
    goto fail;
  if (source->menu_count > 0) {
    destination->menu = checked_storage_try_allocate_array(
        source->menu_count, sizeof(*destination->menu));
    if (!destination->menu)
      goto fail;
    destination->menu_count = source->menu_count;
    for (size_t index = 0; index < source->menu_count; index++) {
      StyledLinkMenuItem *destination_item =
          styled_link_menu_item_at(destination, index);
      const StyledLinkMenuItem *source_item =
          styled_link_menu_item_at_const(source, index);
      *destination_item = *source_item;
      destination_item->label = nullptr;
      destination_item->action = nullptr;
      if (!copy_link_text(&destination_item->label, source_item->label) ||
          !copy_link_text(&destination_item->action, source_item->action))
        goto fail;
    }
  }
  return true;

fail:
  styled_link_config_destroy(destination);
  return false;
}

static void merge_link_properties(StyledLinkProperties *base,
                                  const StyledLinkProperties *overlay) {
  if (overlay->has_foreground) {
    base->foreground = overlay->foreground;
    base->has_foreground = true;
  }
  if (overlay->has_background) {
    base->background = overlay->background;
    base->has_background = true;
  }
  if (overlay->has_decoration_color) {
    base->decoration_color = overlay->decoration_color;
    base->has_decoration_color = true;
  }
  if (overlay->bold != STYLED_BOOLEAN_UNSET)
    base->bold = overlay->bold;
  if (overlay->italic != STYLED_BOOLEAN_UNSET)
    base->italic = overlay->italic;
  if (overlay->underline != STYLED_DECORATION_UNSET)
    base->underline = overlay->underline;
  if (overlay->overline != STYLED_DECORATION_UNSET)
    base->overline = overlay->overline;
  if (overlay->strikethrough != STYLED_DECORATION_UNSET)
    base->strikethrough = overlay->strikethrough;
}

static bool merge_link_text(char **base, const char *overlay) {
  if (!overlay)
    return true;
  char *copy = strdup(overlay);
  if (!copy)
    return false;
  free(*base);
  *base = copy;
  return true;
}

bool styled_link_config_merge(StyledLinkConfig *base,
                              const StyledLinkConfig *overlay) {
  merge_link_properties(&base->style.base, &overlay->style.base);
  for (size_t index = 0; index < STYLED_LINK_STATE_COUNT; index++)
    merge_link_properties(
        styled_link_style_state(&base->style, index),
        styled_link_style_state_const(&overlay->style, index));
  merge_link_properties(&base->title_style, &overlay->title_style);
  if (!merge_link_text(&base->tooltip, overlay->tooltip) ||
      !merge_link_text(&base->title, overlay->title))
    return false;
  if (overlay->menu_count > 0) {
    for (size_t index = 0; index < base->menu_count; index++) {
      StyledLinkMenuItem *item = styled_link_menu_item_at(base, index);
      free(item->label);
      free(item->action);
    }
    free(base->menu);
    base->menu = nullptr;
    base->menu_count = 0;
    StyledLinkConfig menu_source = {.menu = overlay->menu,
                                    .menu_count = overlay->menu_count};
    StyledLinkConfig menu_copy = {0};
    if (!styled_link_config_copy(&menu_copy, &menu_source))
      return false;
    base->menu = menu_copy.menu;
    base->menu_count = menu_copy.menu_count;
    menu_copy.menu = nullptr;
    menu_copy.menu_count = 0;
  }
  const StyledLinkVisibility *visibility = &overlay->visibility;
  if (visibility->action != STYLED_VISIBILITY_ACTION_UNSET)
    base->visibility.action = visibility->action;
  if (visibility->has_delay) {
    base->visibility.delay = visibility->delay;
    base->visibility.has_delay = true;
  }
  if (visibility->has_output_delay) {
    base->visibility.output_delay = visibility->output_delay;
    base->visibility.has_output_delay = true;
  }
  if (visibility->expire_input != STYLED_BOOLEAN_UNSET)
    base->visibility.expire_input = visibility->expire_input;
  if (visibility->expire_prompt != STYLED_BOOLEAN_UNSET)
    base->visibility.expire_prompt = visibility->expire_prompt;
  if (visibility->expire_output != STYLED_BOOLEAN_UNSET)
    base->visibility.expire_output = visibility->expire_output;
  if (visibility->wholeline != STYLED_BOOLEAN_UNSET)
    base->visibility.wholeline = visibility->wholeline;
  if (!merge_link_text(&base->selection.group, overlay->selection.group) ||
      !merge_link_text(&base->selection.value, overlay->selection.value))
    return false;
  if (overlay->selection.toggle != STYLED_BOOLEAN_UNSET)
    base->selection.toggle = overlay->selection.toggle;
  if (overlay->selection.selected != STYLED_BOOLEAN_UNSET)
    base->selection.selected = overlay->selection.selected;
  if (overlay->selection.exclusive != STYLED_BOOLEAN_UNSET)
    base->selection.exclusive = overlay->selection.exclusive;
  if (overlay->selection.disabled != STYLED_BOOLEAN_UNSET)
    base->selection.disabled = overlay->selection.disabled;
  if (overlay->spoiler != STYLED_BOOLEAN_UNSET)
    base->spoiler = overlay->spoiler;
  if (overlay->disabled != STYLED_BOOLEAN_UNSET)
    base->disabled = overlay->disabled;
  return true;
}

void styled_link_fallback_apply(const StyledLinkProperties *properties,
                                StyledState *state) {
  if (properties->has_foreground)
    state->foreground = properties->foreground;
  if (properties->has_background)
    state->background = properties->background;
  if (properties->bold != STYLED_BOOLEAN_UNSET)
    state->bold = properties->bold == STYLED_BOOLEAN_TRUE;
  if (properties->italic != STYLED_BOOLEAN_UNSET)
    state->italic = properties->italic == STYLED_BOOLEAN_TRUE;
  if (properties->underline != STYLED_DECORATION_UNSET)
    state->underline = properties->underline != STYLED_DECORATION_FALSE;
  if (properties->overline != STYLED_DECORATION_UNSET)
    state->overline = properties->overline != STYLED_DECORATION_FALSE;
  if (properties->strikethrough != STYLED_DECORATION_UNSET)
    state->strikethrough = properties->strikethrough != STYLED_DECORATION_FALSE;
}

bool styled_link_properties_present(const StyledLinkProperties *properties) {
  return properties->has_foreground || properties->has_background ||
         properties->has_decoration_color ||
         properties->bold != STYLED_BOOLEAN_UNSET ||
         properties->italic != STYLED_BOOLEAN_UNSET ||
         properties->underline != STYLED_DECORATION_UNSET ||
         properties->overline != STYLED_DECORATION_UNSET ||
         properties->strikethrough != STYLED_DECORATION_UNSET;
}

bool styled_link_config_present(const StyledLinkConfig *config) {
  if (styled_link_properties_present(&config->style.base) || config->tooltip ||
      config->title || config->menu_count > 0 ||
      styled_link_properties_present(&config->title_style) ||
      styled_link_visibility_present(&config->visibility) ||
      styled_link_selection_present(&config->selection) ||
      config->spoiler != STYLED_BOOLEAN_UNSET ||
      config->disabled != STYLED_BOOLEAN_UNSET)
    return true;
  for (size_t index = 0; index < STYLED_LINK_STATE_COUNT; index++) {
    if (styled_link_properties_present(
            styled_link_style_state_const(&config->style, index)))
      return true;
  }
  return false;
}

bool styled_link_has_states(const StyledLinkConfig *config) {
  for (size_t index = 0; index < STYLED_LINK_STATE_COUNT; index++) {
    if (styled_link_properties_present(
            styled_link_style_state_const(&config->style, index)))
      return true;
  }
  return false;
}
