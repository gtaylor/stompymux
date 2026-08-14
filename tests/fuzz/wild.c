#include "mux/support/wild.h"

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"

// NOLINTNEXTLINE(readability-identifier-naming)
int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size);

// NOLINTNEXTLINE(readability-identifier-naming)
int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size) {
  constexpr size_t INPUT_LIMIT = 4096;

  if (size > INPUT_LIMIT)
    return 0;

  const size_t PATTERN_SIZE = size / 2;
  const size_t TEXT_SIZE = size - PATTERN_SIZE;
  char *pattern = checked_storage_allocate(PATTERN_SIZE + 1);
  char *text = checked_storage_allocate(TEXT_SIZE + 1);

  if (PATTERN_SIZE > 0)
    memcpy(pattern, data, PATTERN_SIZE);
  if (TEXT_SIZE > 0)
    memcpy(text,
           checked_storage_region_const(data, size, PATTERN_SIZE, TEXT_SIZE),
           TEXT_SIZE);
  *(char *)checked_storage_at(pattern, PATTERN_SIZE + 1, sizeof(char),
                              PATTERN_SIZE) = '\0';
  *(char *)checked_storage_at(text, TEXT_SIZE + 1, sizeof(char), TEXT_SIZE) =
      '\0';

  (void)quick_wild(pattern, text);
  (void)wild_match(pattern, text);

  free(pattern);
  free(text);
  return 0;
}
