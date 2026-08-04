/* markup.h - Styled-text markup compilation and plain-text operations. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/support/styled_text/palette.h"

bool styled_text_compile(const StyledTextPalette *palette, const char *markup,
                         char *output, size_t output_size, char *error,
                         size_t error_size);
bool styled_text_escape(const char *text, char *output, size_t output_size);
size_t styled_text_width(const StyledTextPalette *palette, const char *styled);
void styled_text_strip(const StyledTextPalette *palette, const char *styled,
                       char *output, size_t output_size);
void styled_text_truncate(const StyledTextPalette *palette, const char *styled,
                          size_t width, char *output, size_t output_size);
