/** @file
 * Capability-aware styled-text terminal rendering.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/support/styled_text/palette.h"

typedef enum TerminalColorDepth : int {
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

/** Executes styled text palette render preset. @param[in] palette Palette.
 * @param[in] index Zero-based index. @param[in] options Formatting or operation
 * options. @param[in] output Caller-owned output storage. @param[in]
 * output_size Size of output in bytes. */

bool styled_text_palette_render_preset(const StyledTextPalette *palette,
                                       size_t index,
                                       const StyledTextRenderOptions *options,
                                       char *output, size_t output_size);
/** Renders styled text. @param[in] palette Palette. @param[in] styled Styled.
 * @param[in] depth Terminal color depth. @param[out] output Caller-owned output
 * storage. @param[in] output_size Size of output in bytes. */

void styled_text_render(const StyledTextPalette *palette, const char *styled,
                        TerminalColorDepth depth, char *output,
                        size_t output_size);
/** Executes styled text render with options. @param[in] palette Palette.
 * @param[in] styled Styled. @param[in] options Formatting or operation options.
 * @param[out] output Caller-owned output storage. @param[in] output_size Size
 * of output in bytes. */

void styled_text_render_with_options(const StyledTextPalette *palette,
                                     const char *styled,
                                     const StyledTextRenderOptions *options,
                                     char *output, size_t output_size);
/** Executes terminal color depth from type. @param[in] name Name to use. */

TerminalColorDepth terminal_color_depth_from_type(const char *name);
/** Parses terminal mtts. @param[in] name Name to use. @param[in,out] depth
 * Terminal color depth. @param[in,out] is_screen_reader Is screen reader. */

bool terminal_mtts_parse(const char *name, TerminalColorDepth *depth,
                         bool *is_screen_reader);
