#include "mux/support/styled_text/markup.h"

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/palette.h"

static StyledTextPalette *styled_text_fuzz_palette(void) {
  static StyledTextPalette *palette;

  if (palette != nullptr)
    return palette;

  palette = styled_text_palette_create();
  if (palette == nullptr)
    return nullptr;

  char error[256] = "";
  if (!styled_text_palette_set_rgb(palette, "brand-blue", 32, 96, 192, error,
                                   sizeof(error)) ||
      !styled_text_palette_set_preset(
          palette,
          &(StyledPresetDefinition){.name = "danger",
                                    .directives = "color=brand-blue bold",
                                    .error = error,
                                    .error_size = sizeof(error)})) {
    styled_text_palette_destroy(palette);
    abort();
  }
  return palette;
}

// NOLINTNEXTLINE(readability-identifier-naming)
int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size);

// NOLINTNEXTLINE(readability-identifier-naming)
int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size) {
  constexpr size_t INPUT_LIMIT = 4096;
  constexpr size_t OUTPUT_SIZE = 16384;

  if (size > INPUT_LIMIT)
    return 0;

  char *input = checked_storage_allocate(size + 1);
  if (size > 0)
    memcpy(input, data, size);
  *(char *)checked_storage_at(input, size + 1, sizeof(char), size) = '\0';

  StyledTextPalette *palette = styled_text_fuzz_palette();
  if (palette == nullptr) {
    free(input);
    return 0;
  }
  char compiled[OUTPUT_SIZE];
  char stripped[OUTPUT_SIZE];
  char error[256];

  const bool COMPILED = styled_text_compile(
      palette, input, compiled, sizeof(compiled), error, sizeof(error));
  styled_text_strip(palette, input, stripped, sizeof(stripped));
  if (COMPILED)
    styled_text_strip(palette, compiled, stripped, sizeof(stripped));

  free(input);
  return 0;
}
