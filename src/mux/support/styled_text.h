/* styled_text.h - Safe color markup and terminal-specific ANSI rendering. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

typedef enum TerminalColorDepth {
  TERMINAL_COLOR_NONE,
  TERMINAL_COLOR_ANSI_16,
  TERMINAL_COLOR_ANSI_256,
  TERMINAL_COLOR_TRUECOLOR,
} TerminalColorDepth;

bool styled_text_compile(const char *markup, char *output, size_t output_size,
                         char *error, size_t error_size);
bool styled_text_escape(const char *text, char *output, size_t output_size);
void styled_text_render(const char *styled, TerminalColorDepth depth,
                        char *output, size_t output_size);
size_t styled_text_width(const char *styled);
void styled_text_strip(const char *styled, char *output, size_t output_size);
void styled_text_truncate(const char *styled, size_t width, char *output,
                          size_t output_size);
TerminalColorDepth terminal_color_depth_from_type(const char *name);
bool terminal_mtts_parse(const char *name, TerminalColorDepth *depth,
                         bool *is_screen_reader);
