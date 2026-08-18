/** @file
 * Styled-text colors and OSC 8 preset catalog.
 */
#pragma once

#include <stddef.h>

typedef struct StyledTextPalette StyledTextPalette;

typedef struct StyledPresetDefinition {
  const char *name;
  const char *directives;
  char *error;
  size_t error_size;
} StyledPresetDefinition;

/** Creates styled text palette. */

StyledTextPalette *styled_text_palette_create(void);
/** Destroys styled text palette. @param[in,out] palette Palette. */

void styled_text_palette_destroy(StyledTextPalette *palette);
/** Sets rgb on styled text palette. @param[in,out] palette Palette. @param[in]
 * name Name to use. @param[in] red Red. @param[in] green Green. @param[in] blue
 * Blue. @param[out] error Storage receiving an error description. @param[in]
 * error_size Size of error in bytes. */

bool styled_text_palette_set_rgb(StyledTextPalette *palette, const char *name,
                                 int red, int green, int blue, char *error,
                                 size_t error_size);
/** Sets preset on styled text palette. @param[in,out] palette Palette.
 * @param[in] definition Definition. */

bool styled_text_palette_set_preset(StyledTextPalette *palette,
                                    const StyledPresetDefinition *definition);
/** Counts styled text palette preset. @param[in] palette Palette. */

size_t styled_text_palette_preset_count(const StyledTextPalette *palette);
