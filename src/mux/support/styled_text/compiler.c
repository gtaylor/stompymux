/* compiler.c - Styled-text markup compilation and link orchestration. */

#include <ctype.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>

#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/internal.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/styled_text/palette.h"
#include "mux/support/styled_text/render.h"
#include "mux/support/utf8.h"

typedef struct StyledLinkTagDefinition {
  const char *name;
  StyledLinkKind kind;
} StyledLinkTagDefinition;

static const char *compiler_suffix(const char *text, size_t length,
                                   size_t offset) {
  return checked_storage_at_const(text, length + 1, sizeof(char), offset);
}

static char compiler_character(const char *text, size_t length, size_t index) {
  return *compiler_suffix(text, length, index);
}

static bool compiler_is_space(char character) {
  return (isspace)((unsigned char)character) != 0;
}

static StyledState *styled_stack_slot(StyledState *stack, size_t index) {
  return checked_storage_at(stack, STYLE_STACK_LIMIT, sizeof(*stack), index);
}

static const StyledLinkProperties *
styled_link_state_properties(const StyledLinkStyle *style, size_t index) {
  return checked_storage_at_const(style->states, STYLED_LINK_STATE_COUNT,
                                  sizeof(*style->states), index);
}

typedef struct StyledTagCloseRequest {
  const char *text;
  size_t length;
  size_t start;
} StyledTagCloseRequest;

typedef struct StyledTagCloseResult {
  bool found;
  size_t offset;
} StyledTagCloseResult;

static StyledTagCloseResult
styled_tag_close_offset(const StyledTagCloseRequest *request) {
  bool quoted = false;
  bool escaped = false;
  for (size_t index = request->start; index < request->length; index++) {
    const char CHARACTER =
        compiler_character(request->text, request->length, index);
    if (escaped) {
      escaped = false;
    } else if (quoted && CHARACTER == '\\') {
      escaped = true;
    } else if (CHARACTER == '"') {
      quoted = ((!quoted) != 0);
    } else if (!quoted && CHARACTER == ']') {
      return (StyledTagCloseResult){.found = true, .offset = index};
    }
  }
  return (StyledTagCloseResult){};
}

const char *styled_find_tag_close(const char *start) {
  const size_t LENGTH = strlen(start);
  StyledTagCloseResult close = styled_tag_close_offset(
      &(StyledTagCloseRequest){.text = start, .length = LENGTH});
  if (close.found)
    return compiler_suffix(start, LENGTH, close.offset);
  return nullptr;
}

static bool parse_link_tag(const char *start, size_t available,
                           StyledLinkKind *kind, const char **target) {
  static const StyledLinkTagDefinition TAGS[] = {
      {"link=", STYLED_LINK_EXTERNAL},
      {"send=", STYLED_LINK_SEND},
      {"prompt=", STYLED_LINK_PROMPT},
  };

  for (size_t index = 0; index < sizeof(TAGS) / sizeof(TAGS[0]); index++) {
    const StyledLinkTagDefinition *tag = checked_storage_at_const(
        TAGS, sizeof(TAGS) / sizeof(TAGS[0]), sizeof(*TAGS), index);
    size_t length = strlen(tag->name);

    if (available > length && !strncasecmp(start, tag->name, length)) {
      *kind = tag->kind;
      *target = compiler_suffix(start, available, length);
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
  const size_t TAG_LENGTH = strlen(tag);
  size_t start_offset = 0;
  size_t end_offset = TAG_LENGTH;
  StyledState updated = *state;
  bool have_directive = false;

  while (start_offset < TAG_LENGTH &&
         compiler_is_space(compiler_character(tag, TAG_LENGTH, start_offset)))
    start_offset++;
  while (end_offset > start_offset &&
         compiler_is_space(compiler_character(tag, TAG_LENGTH, end_offset - 1)))
    end_offset--;
  start = compiler_suffix(tag, TAG_LENGTH, start_offset);
  end = compiler_suffix(tag, TAG_LENGTH, end_offset);
  const size_t TRIMMED_LENGTH = end_offset - start_offset;

  if (TRIMMED_LENGTH == 1 && *start == '/') {
    StyledState restored;

    if (*stack_size == 0) {
      styled_set_error(error, error_size,
                       "style close tag has no matching open tag");
      return false;
    }
    restored = *styled_stack_slot(stack, --*stack_size);
    bool closed_link = (state->has_link && !restored.has_link) != 0;
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
  if (TRIMMED_LENGTH == 5 && !strncasecmp(start, "reset", 5)) {
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
  if (parse_link_tag(start, TRIMMED_LENGTH, &link_kind, &target_start)) {
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
      if (!styled_text_preset_name_valid(config.preset)) {
        styled_link_config_destroy(&config);
        styled_set_error(error, error_size, "unknown OSC 8 preset");
        return false;
      }
      preset = styled_text_palette_find_preset(palette, config.preset);
      if (!preset) {
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
    include_base =
        ((enabled && options && options->osc_hyperlinks_style_basic &&
          styled_link_properties_present(&serialized->style.base)) != 0);
    if (enabled && options && options->osc_hyperlinks_style_states) {
      for (size_t index = 0; index < STYLED_LINK_STATE_COUNT; index++) {
        if (styled_link_properties_present(
                styled_link_state_properties(&serialized->style, index))) {
          include_states = true;
          break;
        }
      }
    }
    include_tooltip = ((enabled && options && options->osc_hyperlinks_tooltip &&
                        serialized->tooltip) != 0);
    include_menu =
        ((enabled && options && options->osc_hyperlinks_menu &&
          styled_link_menu_has_enabled_action(serialized, options)) != 0);
    include_title =
        ((enabled && options && options->osc_hyperlinks_menu &&
          serialized->title &&
          styled_link_menu_has_enabled_action(effective, options)) != 0);
    include_title_style =
        ((include_title && options->osc_hyperlinks_style_basic &&
          serialized->title &&
          styled_link_properties_present(&serialized->title_style)) != 0);
    include_visibility =
        ((enabled && options && options->osc_hyperlinks_visibility &&
          styled_link_visibility_present(&serialized->visibility)) != 0);
    include_selection =
        ((enabled && options && options->osc_hyperlinks_selection &&
          styled_link_selection_present(&serialized->selection)) != 0);
    include_spoiler = ((enabled && options && options->osc_hyperlinks_spoiler &&
                        serialized->spoiler != STYLED_BOOLEAN_UNSET) != 0);
    include_disabled =
        ((enabled && options && options->osc_hyperlinks_disabled &&
          serialized->disabled != STYLED_BOOLEAN_UNSET) != 0);
    append_config =
        ((include_base || include_states || include_tooltip || include_menu ||
          include_title || include_visibility || include_selection ||
          include_spoiler || include_disabled) != 0);
    reserve_config = ((enabled && link_kind == STYLED_LINK_EXTERNAL &&
                       styled_config_capability_advertised(options)) != 0);
    reserve_preset = ((enabled && link_kind == STYLED_LINK_EXTERNAL &&
                       options && options->osc_hyperlinks_presets) != 0);
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

    *styled_stack_slot(stack, (*stack_size)++) = *state;
    updated.has_link = true;
    updated.link_emitted = enabled;
    *state = updated;
    if (state->link_emitted &&
        !styled_emit_link_open(rendered_uri, output, output_size, used)) {
      (*stack_size)--;
      *state = *styled_stack_slot(stack, *stack_size);
      styled_set_error(error, error_size, "styled text is too long");
      return false;
    }
    if (!styled_format_equal(state,
                             styled_stack_slot(stack, *stack_size - 1)) &&
        !styled_emit_state(state, output,
                           styled_output_size(state, output_size), used)) {
      if (state->link_emitted)
        styled_emit_link_close(output, output_size, used);
      (*stack_size)--;
      *state = *styled_stack_slot(stack, *stack_size);
      styled_set_error(error, error_size, "styled text is too long");
      return false;
    }
    return true;
  }

  size_t directive_offset = start_offset;
  while (directive_offset < end_offset) {
    char directive[64];
    size_t directive_length;

    while (directive_offset < end_offset &&
           compiler_is_space(
               compiler_character(tag, TAG_LENGTH, directive_offset)))
      directive_offset++;
    if (directive_offset == end_offset)
      break;
    size_t directive_end = directive_offset;
    while (directive_end < end_offset && !compiler_is_space(compiler_character(
                                             tag, TAG_LENGTH, directive_end)))
      directive_end++;
    directive_length = directive_end - directive_offset;
    if (directive_length >= sizeof(directive)) {
      styled_set_error(error, error_size, "style directive is too long");
      return false;
    }
    memcpy(directive, compiler_suffix(tag, TAG_LENGTH, directive_offset),
           directive_length);
    *(char *)checked_storage_at(directive, sizeof(directive), sizeof(char),
                                directive_length) = '\0';
    if (!styled_style_directive_apply(palette, directive, &updated, error,
                                      error_size))
      return false;
    have_directive = true;
    directive_offset = directive_end;
  }
  if (!have_directive) {
    styled_set_error(error, error_size, "unknown style tag");
    return false;
  }

  *styled_stack_slot(stack, (*stack_size)++) = *state;
  *state = updated;
  return styled_emit_state(state, output,
                           styled_output_size(state, output_size), used);
}

bool styled_text_compile(const StyledTextPalette *palette, const char *markup,
                         char *output, size_t output_size, char *error,
                         size_t error_size) {
  static const StyledTextRenderOptions OPTIONS = {
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

  const size_t MARKUP_LENGTH = strlen(markup);
  for (size_t cursor_offset = 0; cursor_offset < MARKUP_LENGTH;) {
    const char *cursor = compiler_suffix(markup, MARKUP_LENGTH, cursor_offset);
    const char CHARACTER =
        compiler_character(markup, MARKUP_LENGTH, cursor_offset);
    if ((unsigned char)CHARACTER == 0x1b) {
      styled_set_error(error, error_size,
                       "literal escape sequences are not allowed");
      return false;
    }
    if (CHARACTER != '[') {
      Utf8DecodeResult decoded;
      const size_t REMAINING = MARKUP_LENGTH - cursor_offset;
      if (!utf8_decode(cursor, REMAINING < 4 ? REMAINING : 4, &decoded)) {
        styled_set_error(error, error_size, "text is not valid UTF-8");
        return false;
      }
      if (!styled_append_bytes(output, styled_output_size(&state, output_size),
                               &used, cursor, decoded.length))
        goto too_long;
      cursor_offset += decoded.length;
      continue;
    }
    if (cursor_offset + 1 < MARKUP_LENGTH &&
        compiler_character(markup, MARKUP_LENGTH, cursor_offset + 1) == '[') {
      if (!styled_append_bytes(output, styled_output_size(&state, output_size),
                               &used, "[", 1))
        goto too_long;
      cursor_offset += 2;
      continue;
    }

    StyledTagCloseResult close;
    char tag[OSC8_URI_LIMIT + 32];
    size_t tag_length;
    close = styled_tag_close_offset(&(StyledTagCloseRequest){
        .text = markup, .length = MARKUP_LENGTH, .start = cursor_offset + 1});
    if (!close.found) {
      styled_set_error(error, error_size, "unterminated style tag");
      return false;
    }
    tag_length = close.offset - cursor_offset - 1;
    if (tag_length == 0 || tag_length >= sizeof(tag)) {
      styled_set_error(error, error_size, "invalid style tag");
      return false;
    }
    memcpy(tag, compiler_suffix(markup, MARKUP_LENGTH, cursor_offset + 1),
           tag_length);
    *(char *)checked_storage_at(tag, sizeof(tag), sizeof(char), tag_length) =
        '\0';
    if (!apply_tag(palette, tag, &state, stack, &stack_size, output,
                   output_size, &used, &OPTIONS, error, error_size)) {
      if (error && error[0] == '\0')
        goto too_long;
      return false;
    }
    cursor_offset = close.offset + 1;
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
  const size_t LENGTH = strlen(text);
  for (size_t cursor_offset = 0; cursor_offset < LENGTH;) {
    Utf8DecodeResult decoded;
    const char *cursor = compiler_suffix(text, LENGTH, cursor_offset);
    const char CHARACTER = compiler_character(text, LENGTH, cursor_offset);

    if (CHARACTER == '\033')
      return false;
    if (CHARACTER == '[' &&
        !styled_append_bytes(output, output_size, &used, "[", 1))
      return false;
    const size_t REMAINING = LENGTH - cursor_offset;
    if (!utf8_decode(cursor, REMAINING < 4 ? REMAINING : 4, &decoded) ||
        !styled_append_bytes(output, output_size, &used, cursor,
                             decoded.length))
      return false;
    cursor_offset += decoded.length;
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
  const size_t INPUT_LENGTH = strlen(input);
  for (size_t cursor_offset = 0; cursor_offset < INPUT_LENGTH;) {
    const char *cursor = compiler_suffix(input, INPUT_LENGTH, cursor_offset);
    const char CHARACTER =
        compiler_character(input, INPUT_LENGTH, cursor_offset);
    if (CHARACTER == '\033') {
      const char *end;
      int parameters[SGR_PARAMETER_LIMIT];
      size_t parameter_count;

      if (styled_sgr_parse(cursor, &end, parameters, &parameter_count)) {
        const size_t CONSUMED = strlen(cursor) - strlen(end);
        if (!styled_append_bytes(output,
                                 styled_output_size(&state, output_size), &used,
                                 cursor, CONSUMED))
          return;
        cursor_offset += CONSUMED;
      } else {
        cursor_offset += strlen(cursor) - strlen(styled_skip_escape(cursor));
      }
      continue;
    }
    if (CHARACTER != '[') {
      size_t consumed;
      if (!styled_append_utf8_codepoint(output,
                                        styled_output_size(&state, output_size),
                                        &used, cursor, &consumed))
        return;
      cursor_offset += consumed;
      continue;
    }
    if (cursor_offset + 1 < INPUT_LENGTH &&
        compiler_character(input, INPUT_LENGTH, cursor_offset + 1) == '[') {
      if (!styled_append_bytes(output, styled_output_size(&state, output_size),
                               &used, "[", 1))
        return;
      cursor_offset += 2;
      continue;
    }

    StyledTagCloseResult close =
        styled_tag_close_offset(&(StyledTagCloseRequest){
            .text = input, .length = INPUT_LENGTH, .start = cursor_offset + 1});
    size_t tag_length = close.found ? close.offset - cursor_offset - 1 : 0;
    if (close.found && tag_length > 0 && tag_length < OSC8_URI_LIMIT + 32) {
      StyledState candidate_state = state;
      StyledState candidate_stack[STYLE_STACK_LIMIT];
      size_t candidate_stack_size = stack_size;
      char rendered[LBUF_SIZE] = "";
      size_t rendered_size = 0;
      char error[128] = "";
      char tag[OSC8_URI_LIMIT + 32];

      memcpy(tag, compiler_suffix(input, INPUT_LENGTH, cursor_offset + 1),
             tag_length);
      *(char *)checked_storage_at(tag, sizeof(tag), sizeof(char), tag_length) =
          '\0';
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
        cursor_offset = close.offset + 1;
        continue;
      }
    }
    if (!styled_append_bytes(output, styled_output_size(&state, output_size),
                             &used, cursor, 1))
      return;
    cursor_offset++;
  }
  if (state.link_emitted)
    styled_emit_link_close(output, output_size, &used);
}

void styled_text_truncate(const StyledTextPalette *palette, const char *styled,
                          size_t width, char *output, size_t output_size) {
  static const StyledTextRenderOptions OPTIONS = {0};
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
  const size_t STYLED_LENGTH = strlen(styled);
  for (size_t cursor_offset = 0;
       cursor_offset < STYLED_LENGTH && visible < width;) {
    const char *cursor = compiler_suffix(styled, STYLED_LENGTH, cursor_offset);
    const char CHARACTER =
        compiler_character(styled, STYLED_LENGTH, cursor_offset);
    if (CHARACTER == '\033') {
      const char *end;
      int parameters[SGR_PARAMETER_LIMIT];
      size_t parameter_count;
      if (styled_sgr_parse(cursor, &end, parameters, &parameter_count)) {
        const size_t CONSUMED = strlen(cursor) - strlen(end);
        if (!styled_append_bytes(output, output_size, &used, cursor, CONSUMED))
          break;
        saw_sgr = true;
        cursor_offset += CONSUMED;
      } else {
        cursor_offset += strlen(cursor) - strlen(styled_skip_escape(cursor));
      }
    } else if (CHARACTER == '[' && cursor_offset + 1 < STYLED_LENGTH &&
               compiler_character(styled, STYLED_LENGTH, cursor_offset + 1) ==
                   '[') {
      if (!styled_append_bytes(output, output_size, &used, cursor, 2))
        break;
      cursor_offset += 2;
      visible++;
    } else if (CHARACTER == '[') {
      StyledTagCloseResult close = styled_tag_close_offset(&(
          StyledTagCloseRequest){
          .text = styled, .length = STYLED_LENGTH, .start = cursor_offset + 1});
      size_t tag_length = close.found ? close.offset - cursor_offset - 1 : 0;
      bool applied = false;

      if (close.found && tag_length > 0 && tag_length < OSC8_URI_LIMIT + 32) {
        StyledState candidate_state = state;
        StyledState candidate_stack[STYLE_STACK_LIMIT];
        size_t candidate_stack_size = stack_size;
        char rendered[LBUF_SIZE] = "";
        size_t rendered_size = 0;
        char error[128] = "";
        char tag[OSC8_URI_LIMIT + 32];

        memcpy(tag, compiler_suffix(styled, STYLED_LENGTH, cursor_offset + 1),
               tag_length);
        *(char *)checked_storage_at(tag, sizeof(tag), sizeof(char),
                                    tag_length) = '\0';
        memcpy(candidate_stack, stack, sizeof(stack));
        if (apply_tag(palette, tag, &candidate_state, candidate_stack,
                      &candidate_stack_size, rendered, sizeof(rendered),
                      &rendered_size, &OPTIONS, error, sizeof(error)) &&
            styled_append_bytes(output, output_size, &used, cursor,
                                close.offset - cursor_offset + 1)) {
          state = candidate_state;
          memcpy(stack, candidate_stack, sizeof(stack));
          stack_size = candidate_stack_size;
          cursor_offset = close.offset + 1;
          applied = true;
        }
      }
      if (!applied) {
        if (!styled_append_bytes(output, output_size, &used, cursor, 1))
          break;
        cursor_offset++;
        visible++;
      }
    } else {
      Utf8DecodeResult decoded;
      const size_t REMAINING = STYLED_LENGTH - cursor_offset;
      if (!utf8_decode(cursor, REMAINING < 4 ? REMAINING : 4, &decoded) ||
          visible + decoded.length > width ||
          !styled_append_bytes(output, output_size, &used, cursor,
                               decoded.length))
        break;
      cursor_offset += decoded.length;
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
