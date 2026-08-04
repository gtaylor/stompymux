/* compiler.c - Styled-text markup compilation and link orchestration. */

#include <ctype.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>

#include "mux/support/alloc.h"
#include "mux/support/styled_text/internal.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/styled_text/palette.h"
#include "mux/support/styled_text/render.h"
#include "mux/support/utf8.h"

const char *styled_find_tag_close(const char *start) {
  bool quoted = false;
  bool escaped = false;

  for (const char *cursor = start; *cursor; cursor++) {
    if (escaped) {
      escaped = false;
      continue;
    }
    if (quoted && *cursor == '\\') {
      escaped = true;
      continue;
    }
    if (*cursor == '"') {
      quoted = !quoted;
      continue;
    }
    if (!quoted && *cursor == ']')
      return cursor;
  }
  return nullptr;
}

static bool parse_link_tag(const char *start, const char *end,
                           StyledLinkKind *kind, const char **target) {
  static const struct {
    const char *name;
    StyledLinkKind kind;
  } tags[] = {
      {"link=", STYLED_LINK_EXTERNAL},
      {"send=", STYLED_LINK_SEND},
      {"prompt=", STYLED_LINK_PROMPT},
  };

  for (size_t index = 0; index < sizeof(tags) / sizeof(tags[0]); index++) {
    size_t length = strlen(tags[index].name);

    if ((size_t)(end - start) > length &&
        !strncasecmp(start, tags[index].name, length)) {
      *kind = tags[index].kind;
      *target = start + length;
      return true;
    }
  }
  return false;
}

static bool apply_tag(const StyledTextPalette *palette, const char *tag,
                      StyledState *state, StyledState *stack,
                      size_t *stack_size, char *output, size_t output_size,
                      size_t *used, const StyledTextRenderOptions *options,
                      char *error, size_t error_size) {
  const char *start = tag;
  const char *end;
  StyledState updated = *state;
  bool have_directive = false;

  while (isspace((unsigned char)*start))
    start++;
  end = start + strlen(start);
  while (end > start && isspace((unsigned char)end[-1]))
    end--;

  if (end - start == 1 && *start == '/') {
    StyledState restored;

    if (*stack_size == 0) {
      styled_set_error(error, error_size,
                       "style close tag has no matching open tag");
      return false;
    }
    restored = stack[--*stack_size];
    bool closed_link = state->has_link && !restored.has_link;
    if (state->link_emitted && !restored.link_emitted) {
      if (!styled_emit_link_close(output, output_size, used))
        return false;
    }
    if (closed_link && styled_format_equal(state, &restored)) {
      *state = restored;
      return true;
    }
    *state = restored;
    return styled_emit_state(state, output,
                             styled_output_size(state, output_size), used);
  }
  if (end - start == 5 && !strncasecmp(start, "reset", 5)) {
    if (state->link_emitted &&
        !styled_emit_link_close(output, output_size, used))
      return false;
    *state = (StyledState){0};
    *stack_size = 0;
    return styled_emit_state(state, output, output_size, used);
  }
  if (*stack_size >= STYLE_STACK_LIMIT) {
    styled_set_error(error, error_size, "style nesting is too deep");
    return false;
  }

  StyledLinkKind link_kind;
  const char *target_start;
  if (parse_link_tag(start, end, &link_kind, &target_start)) {
    char target[OSC8_URI_LIMIT + 1];
    char uri[OSC8_URI_LIMIT + 1];
    char rendered_uri[OSC8_URI_LIMIT + 1];
    const char *directives;
    StyledLinkConfig config = {0};
    StyledLinkConfig effective_config = {0};
    const StyledLinkConfig *effective = &config;
    const StyledLinkConfig *serialized = &config;
    const StyledTextPreset *preset = nullptr;
    bool enabled;
    bool include_base;
    bool include_states = false;
    bool include_tooltip;
    bool include_menu;
    bool include_title;
    bool include_title_style;
    bool include_visibility;
    bool include_selection;
    bool include_spoiler;
    bool include_disabled;
    bool append_config;
    bool reserve_config;
    bool reserve_preset;
    const char *preset_name = nullptr;

    if (state->has_link) {
      styled_set_error(error, error_size, "links cannot be nested");
      return false;
    }
    if (!styled_link_target_unquote(target_start, end, target, sizeof(target),
                                    &directives, error, error_size))
      return false;
    if (link_kind == STYLED_LINK_EXTERNAL) {
      if (!styled_external_uri_valid(target, error, error_size))
        return false;
      memcpy(uri, target, strlen(target) + 1);
    } else if (!styled_command_uri_encode(link_kind, target, uri, sizeof(uri),
                                          error, error_size)) {
      return false;
    }

    if (!styled_link_directives_parse(palette, directives, end, &config,
                                      &updated, true, false, error,
                                      error_size)) {
      styled_link_config_destroy(&config);
      return false;
    }

    if (config.preset) {
      if (!styled_text_preset_name_valid(config.preset) ||
          !(preset = styled_text_palette_find_preset(palette, config.preset))) {
        styled_link_config_destroy(&config);
        styled_set_error(error, error_size, "unknown OSC 8 preset");
        return false;
      }
      if (!styled_link_config_copy(&effective_config, &preset->config) ||
          !styled_link_config_merge(&effective_config, &config)) {
        styled_link_config_destroy(&effective_config);
        styled_link_config_destroy(&config);
        styled_set_error(error, error_size,
                         "out of memory resolving OSC 8 preset");
        return false;
      }
      effective = &effective_config;
      if (options && options->osc_hyperlinks_presets) {
        preset_name = config.preset;
        if ((config.title ||
             styled_link_properties_present(&config.title_style)) &&
            effective->title) {
          if (!styled_link_text_replace(&config.title, effective->title, error,
                                        error_size)) {
            styled_link_config_destroy(&effective_config);
            styled_link_config_destroy(&config);
            return false;
          }
          config.title_style = effective->title_style;
        }
      } else {
        serialized = effective;
      }
    }
    if (!styled_link_config_valid(effective, error, error_size)) {
      styled_link_config_destroy(&effective_config);
      styled_link_config_destroy(&config);
      return false;
    }

    enabled = styled_link_enabled(link_kind, options);
    if (enabled && effective->disabled == STYLED_BOOLEAN_TRUE &&
        (!options || !options->osc_hyperlinks_disabled))
      enabled = false;
    include_base = enabled && options && options->osc_hyperlinks_style_basic &&
                   styled_link_properties_present(&serialized->style.base);
    if (enabled && options && options->osc_hyperlinks_style_states) {
      for (size_t index = 0; index < STYLED_LINK_STATE_COUNT; index++) {
        if (styled_link_properties_present(&serialized->style.states[index])) {
          include_states = true;
          break;
        }
      }
    }
    include_tooltip = enabled && options && options->osc_hyperlinks_tooltip &&
                      serialized->tooltip;
    include_menu = enabled && options && options->osc_hyperlinks_menu &&
                   styled_link_menu_has_enabled_action(serialized, options);
    include_title = enabled && options && options->osc_hyperlinks_menu &&
                    serialized->title &&
                    styled_link_menu_has_enabled_action(effective, options);
    include_title_style =
        include_title && options->osc_hyperlinks_style_basic &&
        serialized->title &&
        styled_link_properties_present(&serialized->title_style);
    include_visibility =
        enabled && options && options->osc_hyperlinks_visibility &&
        styled_link_visibility_present(&serialized->visibility);
    include_selection = enabled && options &&
                        options->osc_hyperlinks_selection &&
                        styled_link_selection_present(&serialized->selection);
    include_spoiler = enabled && options && options->osc_hyperlinks_spoiler &&
                      serialized->spoiler != STYLED_BOOLEAN_UNSET;
    include_disabled = enabled && options && options->osc_hyperlinks_disabled &&
                       serialized->disabled != STYLED_BOOLEAN_UNSET;
    append_config = include_base || include_states || include_tooltip ||
                    include_menu || include_title || include_visibility ||
                    include_selection || include_spoiler || include_disabled;
    reserve_config = enabled && link_kind == STYLED_LINK_EXTERNAL &&
                     styled_config_capability_advertised(options);
    reserve_preset = enabled && link_kind == STYLED_LINK_EXTERNAL && options &&
                     options->osc_hyperlinks_presets;
    if (!include_base)
      styled_link_fallback_apply(&effective->style.base, &updated);
    if (append_config || reserve_config || reserve_preset || preset_name) {
      if (!styled_build_configured_uri(
              uri, serialized, include_base, include_states, include_tooltip,
              include_menu, include_title, include_title_style,
              include_visibility, include_selection, include_spoiler,
              include_disabled, preset_name, append_config, reserve_config,
              reserve_preset, options, rendered_uri, sizeof(rendered_uri))) {
        styled_link_config_destroy(&effective_config);
        styled_link_config_destroy(&config);
        styled_set_error(error, error_size, "styled link URI is too long");
        return false;
      }
    } else {
      memcpy(rendered_uri, uri, strlen(uri) + 1);
    }
    styled_link_config_destroy(&effective_config);
    styled_link_config_destroy(&config);

    stack[(*stack_size)++] = *state;
    updated.has_link = true;
    updated.link_emitted = enabled;
    *state = updated;
    if (state->link_emitted &&
        !styled_emit_link_open(rendered_uri, output, output_size, used)) {
      (*stack_size)--;
      *state = stack[*stack_size];
      styled_set_error(error, error_size, "styled text is too long");
      return false;
    }
    if (!styled_format_equal(state, &stack[*stack_size - 1]) &&
        !styled_emit_state(state, output,
                           styled_output_size(state, output_size), used)) {
      if (state->link_emitted)
        styled_emit_link_close(output, output_size, used);
      (*stack_size)--;
      *state = stack[*stack_size];
      styled_set_error(error, error_size, "styled text is too long");
      return false;
    }
    return true;
  }

  while (start < end) {
    const char *directive_end;
    char directive[64];
    size_t directive_length;

    while (start < end && isspace((unsigned char)*start))
      start++;
    if (start == end)
      break;
    directive_end = start;
    while (directive_end < end && !isspace((unsigned char)*directive_end))
      directive_end++;
    directive_length = (size_t)(directive_end - start);
    if (directive_length >= sizeof(directive)) {
      styled_set_error(error, error_size, "style directive is too long");
      return false;
    }
    memcpy(directive, start, directive_length);
    directive[directive_length] = '\0';
    if (!styled_style_directive_apply(palette, directive, &updated, error,
                                      error_size))
      return false;
    have_directive = true;
    start = directive_end;
  }
  if (!have_directive) {
    styled_set_error(error, error_size, "unknown style tag");
    return false;
  }

  stack[(*stack_size)++] = *state;
  *state = updated;
  return styled_emit_state(state, output,
                           styled_output_size(state, output_size), used);
}

bool styled_text_compile(const StyledTextPalette *palette, const char *markup,
                         char *output, size_t output_size, char *error,
                         size_t error_size) {
  static const StyledTextRenderOptions options = {
      .color_depth = TERMINAL_COLOR_TRUECOLOR,
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
  StyledState state = {0};
  StyledState stack[STYLE_STACK_LIMIT];
  size_t stack_size = 0;
  size_t used = 0;

  if (!markup || !output || output_size == 0) {
    styled_set_error(error, error_size, "invalid style input");
    return false;
  }
  output[0] = '\0';
  if (error && error_size > 0)
    error[0] = '\0';

  for (const char *cursor = markup; *cursor;) {
    if ((unsigned char)*cursor == 0x1b) {
      styled_set_error(error, error_size,
                       "literal escape sequences are not allowed");
      return false;
    }
    if (*cursor != '[') {
      Utf8DecodeResult decoded;
      if (!utf8_decode(cursor, strnlen(cursor, 4), &decoded)) {
        styled_set_error(error, error_size, "text is not valid UTF-8");
        return false;
      }
      if (!styled_append_bytes(output, styled_output_size(&state, output_size),
                               &used, cursor, decoded.length))
        goto too_long;
      cursor += decoded.length;
      continue;
    }
    if (cursor[1] == '[') {
      if (!styled_append_bytes(output, styled_output_size(&state, output_size),
                               &used, "[", 1))
        goto too_long;
      cursor += 2;
      continue;
    }

    const char *close = styled_find_tag_close(cursor + 1);
    char tag[OSC8_URI_LIMIT + 32];
    size_t tag_length;
    if (!close) {
      styled_set_error(error, error_size, "unterminated style tag");
      return false;
    }
    tag_length = (size_t)(close - cursor - 1);
    if (tag_length == 0 || tag_length >= sizeof(tag)) {
      styled_set_error(error, error_size, "invalid style tag");
      return false;
    }
    memcpy(tag, cursor + 1, tag_length);
    tag[tag_length] = '\0';
    if (!apply_tag(palette, tag, &state, stack, &stack_size, output,
                   output_size, &used, &options, error, error_size)) {
      if (error && error[0] == '\0')
        goto too_long;
      return false;
    }
    cursor = close + 1;
  }
  if (stack_size != 0) {
    styled_set_error(error, error_size, "style tag is not closed");
    return false;
  }
  return true;

too_long:
  styled_set_error(error, error_size, "styled text is too long");
  return false;
}

bool styled_text_escape(const char *text, char *output, size_t output_size) {
  size_t used = 0;

  if (!text || !output || output_size == 0)
    return false;
  output[0] = '\0';
  for (const char *cursor = text; *cursor;) {
    Utf8DecodeResult decoded;

    if (*cursor == '\033')
      return false;
    if (*cursor == '[' &&
        !styled_append_bytes(output, output_size, &used, "[", 1))
      return false;
    if (!utf8_decode(cursor, strnlen(cursor, 4), &decoded) ||
        !styled_append_bytes(output, output_size, &used, cursor,
                             decoded.length))
      return false;
    cursor += decoded.length;
  }
  return true;
}

void styled_text_compile_permissive(const StyledTextPalette *palette,
                                    const char *input, char *output,
                                    size_t output_size,
                                    const StyledTextRenderOptions *options) {
  StyledState state = {0};
  StyledState stack[STYLE_STACK_LIMIT];
  size_t stack_size = 0;
  size_t used = 0;

  output[0] = '\0';
  for (const char *cursor = input; *cursor;) {
    if (*cursor == '\033') {
      const char *end;
      int parameters[SGR_PARAMETER_LIMIT];
      size_t parameter_count;

      if (styled_sgr_parse(cursor, &end, parameters, &parameter_count)) {
        if (!styled_append_bytes(output,
                                 styled_output_size(&state, output_size), &used,
                                 cursor, (size_t)(end - cursor)))
          return;
        cursor = end;
      } else {
        cursor = styled_skip_escape(cursor);
      }
      continue;
    }
    if (*cursor != '[') {
      size_t consumed;
      if (!styled_append_utf8_codepoint(output,
                                        styled_output_size(&state, output_size),
                                        &used, cursor, &consumed))
        return;
      cursor += consumed;
      continue;
    }
    if (cursor[1] == '[') {
      if (!styled_append_bytes(output, styled_output_size(&state, output_size),
                               &used, "[", 1))
        return;
      cursor += 2;
      continue;
    }

    const char *close = styled_find_tag_close(cursor + 1);
    size_t tag_length = close ? (size_t)(close - cursor - 1) : 0;
    if (close && tag_length > 0 && tag_length < OSC8_URI_LIMIT + 32) {
      StyledState candidate_state = state;
      StyledState candidate_stack[STYLE_STACK_LIMIT];
      size_t candidate_stack_size = stack_size;
      char rendered[LBUF_SIZE] = "";
      size_t rendered_size = 0;
      char error[128] = "";
      char tag[OSC8_URI_LIMIT + 32];

      memcpy(tag, cursor + 1, tag_length);
      tag[tag_length] = '\0';
      memcpy(candidate_stack, stack, sizeof(stack));
      if (apply_tag(palette, tag, &candidate_state, candidate_stack,
                    &candidate_stack_size, rendered, sizeof(rendered),
                    &rendered_size, options, error, sizeof(error))) {
        if (!styled_append_string(
                output, styled_output_size(&candidate_state, output_size),
                &used, rendered))
          return;
        state = candidate_state;
        memcpy(stack, candidate_stack, sizeof(stack));
        stack_size = candidate_stack_size;
        cursor = close + 1;
        continue;
      }
    }
    if (!styled_append_bytes(output, styled_output_size(&state, output_size),
                             &used, cursor, 1))
      return;
    cursor++;
  }
  if (state.link_emitted)
    styled_emit_link_close(output, output_size, &used);
}

void styled_text_truncate(const StyledTextPalette *palette, const char *styled,
                          size_t width, char *output, size_t output_size) {
  static const StyledTextRenderOptions options = {0};
  StyledState state = {0};
  StyledState stack[STYLE_STACK_LIMIT];
  size_t stack_size = 0;
  size_t used = 0;
  size_t visible = 0;
  bool saw_sgr = false;

  if (!output || output_size == 0)
    return;
  output[0] = '\0';
  if (!styled)
    return;
  for (const char *cursor = styled; *cursor && visible < width;) {
    if (*cursor == '\033') {
      const char *end;
      int parameters[SGR_PARAMETER_LIMIT];
      size_t parameter_count;
      if (styled_sgr_parse(cursor, &end, parameters, &parameter_count)) {
        if (!styled_append_bytes(output, output_size, &used, cursor,
                                 (size_t)(end - cursor)))
          break;
        saw_sgr = true;
        cursor = end;
      } else {
        cursor = styled_skip_escape(cursor);
      }
    } else if (*cursor == '[' && cursor[1] == '[') {
      if (!styled_append_bytes(output, output_size, &used, cursor, 2))
        break;
      cursor += 2;
      visible++;
    } else if (*cursor == '[') {
      const char *close = styled_find_tag_close(cursor + 1);
      size_t tag_length = close ? (size_t)(close - cursor - 1) : 0;
      bool applied = false;

      if (close && tag_length > 0 && tag_length < OSC8_URI_LIMIT + 32) {
        StyledState candidate_state = state;
        StyledState candidate_stack[STYLE_STACK_LIMIT];
        size_t candidate_stack_size = stack_size;
        char rendered[LBUF_SIZE] = "";
        size_t rendered_size = 0;
        char error[128] = "";
        char tag[OSC8_URI_LIMIT + 32];

        memcpy(tag, cursor + 1, tag_length);
        tag[tag_length] = '\0';
        memcpy(candidate_stack, stack, sizeof(stack));
        if (apply_tag(palette, tag, &candidate_state, candidate_stack,
                      &candidate_stack_size, rendered, sizeof(rendered),
                      &rendered_size, &options, error, sizeof(error)) &&
            styled_append_bytes(output, output_size, &used, cursor,
                                (size_t)(close - cursor + 1))) {
          state = candidate_state;
          memcpy(stack, candidate_stack, sizeof(stack));
          stack_size = candidate_stack_size;
          cursor = close + 1;
          applied = true;
        }
      }
      if (!applied) {
        if (!styled_append_bytes(output, output_size, &used, cursor, 1))
          break;
        cursor++;
        visible++;
      }
    } else {
      Utf8DecodeResult decoded;
      if (!utf8_decode(cursor, strnlen(cursor, 4), &decoded) ||
          visible + decoded.length > width ||
          !styled_append_bytes(output, output_size, &used, cursor,
                               decoded.length))
        break;
      cursor += decoded.length;
      visible += decoded.length;
    }
  }
  while (stack_size > 0) {
    if (!styled_append_string(output, output_size, &used, "[/]"))
      break;
    stack_size--;
  }
  if (saw_sgr)
    styled_append_string(output, output_size, &used, "\033[0m");
}
