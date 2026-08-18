/** @file
 * Styled-text markup compilation and plain-text operations.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/support/styled_text/palette.h"

/** Executes styled text compile. @param[in] palette Palette. @param[in] markup
 * Markup. @param[out] output Caller-owned output storage. @param[in]
 * output_size Size of output in bytes. @param[out] error Storage receiving an
 * error description. @param[in] error_size Size of error in bytes. */

bool styled_text_compile(const StyledTextPalette *palette, const char *markup,
                         char *output, size_t output_size, char *error,
                         size_t error_size);
/** Executes styled text escape. @param[in] text Text to process. @param[out]
 * output Caller-owned output storage. @param[in] output_size Size of output in
 * bytes. */

bool styled_text_escape(const char *text, char *output, size_t output_size);
/** Executes styled text width. @param[in] palette Palette. @param[in] styled
 * Styled. */

size_t styled_text_width(const StyledTextPalette *palette, const char *styled);
/** Executes styled text strip. @param[in] palette Palette. @param[in] styled
 * Styled. @param[out] output Caller-owned output storage. @param[in]
 * output_size Size of output in bytes. */

void styled_text_strip(const StyledTextPalette *palette, const char *styled,
                       char *output, size_t output_size);
/** Executes styled text truncate. @param[in] palette Palette. @param[in] styled
 * Styled. @param[in] width Requested display width. @param[out] output
 * Caller-owned output storage. @param[in] output_size Size of output in bytes.
 */

void styled_text_truncate(const StyledTextPalette *palette, const char *styled,
                          size_t width, char *output, size_t output_size);
