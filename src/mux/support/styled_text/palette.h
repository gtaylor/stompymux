/* palette.h - Styled-text colors and OSC 8 preset catalog. */

#pragma once

#include <stddef.h>

typedef struct StyledTextPalette StyledTextPalette;

typedef struct StyledPresetDefinition {
  const char *name;
  const char *directives;
  char *error;
  size_t error_size;
} StyledPresetDefinition;

StyledTextPalette *styled_text_palette_create(void);
void styled_text_palette_destroy(StyledTextPalette *palette);
bool styled_text_palette_set_rgb(StyledTextPalette *palette, const char *name,
                                 int red, int green, int blue, char *error,
                                 size_t error_size);
bool styled_text_palette_set_preset(StyledTextPalette *palette,
                                    const StyledPresetDefinition *definition);
size_t styled_text_palette_preset_count(const StyledTextPalette *palette);
