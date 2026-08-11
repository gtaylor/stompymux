/* output.c - Bounded output and terminal-state helpers. */

#include <stdio.h>
#include <string.h>

#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/internal.h"
#include "mux/support/utf8.h"

bool styled_append_bytes(char *output, size_t output_size, size_t *used,
                         const char *value, size_t length) {
  if (length >= output_size || *used >= output_size - length)
    return false;
  memcpy(checked_storage_region(output, output_size, *used, length), value,
         length);
  *used += length;
  *(char *)checked_storage_at(output, output_size, sizeof(char), *used) = '\0';
  return true;
}

bool styled_append_string(char *output, size_t output_size, size_t *used,
                          const char *value) {
  return styled_append_bytes(output, output_size, used, value, strlen(value));
}

bool styled_append_utf8_codepoint(char *output, size_t output_size,
                                  size_t *used, const char *value,
                                  size_t *consumed) {
  static const char REPLACEMENT[] = "\xef\xbf\xbd";
  Utf8DecodeResult decoded;
  size_t available = strnlen(value, 4);

  if (utf8_decode(value, available, &decoded)) {
    *consumed = decoded.length;
    return styled_append_bytes(output, output_size, used, value,
                               decoded.length);
  }
  *consumed = 1;
  return styled_append_bytes(output, output_size, used, REPLACEMENT,
                             sizeof(REPLACEMENT) - 1);
}

void styled_set_error(char *error, size_t error_size, const char *message) {
  if (error && error_size > 0)
    (void)snprintf(error, error_size, "%s", message);
}

bool styled_emit_state(const StyledState *state, char *output,
                       size_t output_size, size_t *used) {
  char sequence[64];

  if (!styled_append_string(output, output_size, used, "\033[0m"))
    return false;
  if (state->bold &&
      !styled_append_string(output, output_size, used, "\033[1m"))
    return false;
  if (state->italic &&
      !styled_append_string(output, output_size, used, "\033[3m"))
    return false;
  if (state->blink &&
      !styled_append_string(output, output_size, used, "\033[5m"))
    return false;
  if (state->underline &&
      !styled_append_string(output, output_size, used, "\033[4m"))
    return false;
  if (state->overline &&
      !styled_append_string(output, output_size, used, "\033[53m"))
    return false;
  if (state->strikethrough &&
      !styled_append_string(output, output_size, used, "\033[9m"))
    return false;
  if (state->inverse &&
      !styled_append_string(output, output_size, used, "\033[7m"))
    return false;
  if (state->foreground.kind == STYLED_COLOR_RGB) {
    (void)snprintf(sequence, sizeof(sequence), "\033[38;2;%d;%d;%dm",
                   state->foreground.red, state->foreground.green,
                   state->foreground.blue);
    if (!styled_append_string(output, output_size, used, sequence))
      return false;
  }
  if (state->background.kind == STYLED_COLOR_RGB) {
    (void)snprintf(sequence, sizeof(sequence), "\033[48;2;%d;%d;%dm",
                   state->background.red, state->background.green,
                   state->background.blue);
    if (!styled_append_string(output, output_size, used, sequence))
      return false;
  }
  return true;
}

size_t styled_output_size(const StyledState *state, size_t output_size) {
  if (!state->link_emitted || output_size <= OSC8_CLOSE_SIZE)
    return output_size;
  return output_size - OSC8_CLOSE_SIZE;
}

bool styled_format_equal(const StyledState *left, const StyledState *right) {
  return left->foreground.kind == right->foreground.kind &&
         left->foreground.red == right->foreground.red &&
         left->foreground.green == right->foreground.green &&
         left->foreground.blue == right->foreground.blue &&
         left->background.kind == right->background.kind &&
         left->background.red == right->background.red &&
         left->background.green == right->background.green &&
         left->background.blue == right->background.blue &&
         left->bold == right->bold && left->italic == right->italic &&
         left->blink == right->blink && left->underline == right->underline &&
         left->overline == right->overline &&
         left->strikethrough == right->strikethrough &&
         left->inverse == right->inverse;
}
