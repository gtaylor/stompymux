#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_help_internal.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static bool oversized_first_word_is_bounded(void) {
  const size_t WORD_LENGTH = LBUF_SIZE + 16;
  const size_t INPUT_SIZE = WORD_LENGTH + sizeof(" tail");
  char *input = checked_storage_allocate(INPUT_SIZE);
  char *output = alloc_lbuf("registry help test output");

  memset(input, 'x', WORD_LENGTH);
  memcpy(checked_mutable_string_suffix(input, WORD_LENGTH), " tail",
         sizeof(" tail"));
  memset(output, 'y', LBUF_SIZE);

  registry_help_color_initialize(input, output);
  const bool bounded =
      *(const char *)checked_storage_at_const(output, LBUF_SIZE, sizeof(char),
                                              LBUF_SIZE - 1) == '\0' &&
      strnlen(output, LBUF_SIZE) < LBUF_SIZE;

  free(input);
  free_lbuf(output);
  return bounded;
}

int main(void) {
  if (oversized_first_word_is_bounded())
    return 0;

  (void)fprintf(stderr, "oversized registry-help word was not bounded\n");
  return 1;
}
