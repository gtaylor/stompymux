#include "mux/network/network_output_format.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

[[gnu::format(printf, 3, 4)]] static bool
format_line(char *buffer, size_t size, const char *format, ...) {
  va_list arguments;
  va_start(arguments, format);
  const bool RESULT =
      network_output_format_line_v(buffer, size, format, arguments);
  va_end(arguments);
  return RESULT;
}

static bool exact_fit_includes_suffix(void) {
  char buffer[8];

  return format_line(buffer, sizeof(buffer), "%s", "12345") &&
         strcmp(buffer, "12345\r\n") == 0;
}

static bool truncation_preserves_suffix(void) {
  char buffer[8];

  return !format_line(buffer, sizeof(buffer), "%s", "123456") &&
         strcmp(buffer, "12345\r\n") == 0;
}

static bool output_stays_within_bounds(void) {
  unsigned char storage[9];
  memset(storage, 0xAA, sizeof(storage));

  return !format_line((char *)storage, 8, "%s", "123456789") &&
         strcmp((char *)storage, "12345\r\n") == 0 && storage[8] == 0xAA;
}

static bool undersized_buffer_is_safe(void) {
  char buffer[2] = {'x', 'y'};

  return !format_line(buffer, sizeof(buffer), "%s", "text") &&
         buffer[0] == '\0' && buffer[1] == 'y';
}

static bool zero_size_writes_nothing(void) {
  char buffer = 'x';

  return !format_line(&buffer, 0, "%s", "text") && buffer == 'x';
}

int main(void) {
  int failures = 0;

  if (!exact_fit_includes_suffix()) {
    (void)fprintf(stderr, "exact-fit line lost its suffix\n");
    failures++;
  }
  if (!truncation_preserves_suffix()) {
    (void)fprintf(stderr, "truncated line lost its suffix\n");
    failures++;
  }
  if (!output_stays_within_bounds()) {
    (void)fprintf(stderr, "formatted line wrote out of bounds\n");
    failures++;
  }
  if (!undersized_buffer_is_safe()) {
    (void)fprintf(stderr, "undersized line buffer was not handled safely\n");
    failures++;
  }
  if (!zero_size_writes_nothing()) {
    (void)fprintf(stderr, "zero-sized line buffer was modified\n");
    failures++;
  }
  return failures != 0;
}
