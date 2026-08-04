/* render.h - Capability-aware styled-text terminal rendering. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/support/styled_text/palette.h"

typedef enum TerminalColorDepth {
  TERMINAL_COLOR_NONE,
  TERMINAL_COLOR_ANSI_16,
  TERMINAL_COLOR_ANSI_256,
  TERMINAL_COLOR_TRUECOLOR,
} TerminalColorDepth;

typedef struct StyledTextRenderOptions {
  TerminalColorDepth color_depth;
  bool osc_hyperlinks;
  bool osc_hyperlinks_send;
  bool osc_hyperlinks_prompt;
  bool osc_hyperlinks_style_basic;
  bool osc_hyperlinks_style_states;
  bool osc_hyperlinks_tooltip;
  bool osc_hyperlinks_menu;
  bool osc_hyperlinks_visibility;
  bool osc_hyperlinks_spoiler;
  bool osc_hyperlinks_disabled;
  bool osc_hyperlinks_selection;
  bool osc_hyperlinks_compact;
  bool osc_hyperlinks_presets;
} StyledTextRenderOptions;

bool styled_text_palette_render_preset(const StyledTextPalette *palette,
                                       size_t index,
                                       const StyledTextRenderOptions *options,
                                       char *output, size_t output_size);
void styled_text_render(const StyledTextPalette *palette, const char *styled,
                        TerminalColorDepth depth, char *output,
                        size_t output_size);
void styled_text_render_with_options(const StyledTextPalette *palette,
                                     const char *styled,
                                     const StyledTextRenderOptions *options,
                                     char *output, size_t output_size);
TerminalColorDepth terminal_color_depth_from_type(const char *name);
bool terminal_mtts_parse(const char *name, TerminalColorDepth *depth,
                         bool *is_screen_reader);
