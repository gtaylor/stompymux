#include <errno.h>
#include <limits.h>
#include <math.h>
#include <stdbool.h>
#include <string.h>
#include <time.h>

#include "btech_text_builder.h"
#include "checked_conversion.h"
#include "mux/support/stringutil.h"

static int test_parsing(void) {
  int integer = 0;
  long long_value = 0;
  time_t timestamp = 0;
  float float_value = 0.0F;

  if (!parse_int_checked(" -42 \t", &integer) || integer != -42 ||
      parse_int_checked("42x", &integer) || parse_int_checked("", &integer) ||
      parse_int_checked("999999999999999999999", &integer) ||
      !parse_long_checked("2147483648", &long_value) ||
      long_value != 2147483648L || !parse_time_checked("12345", &timestamp) ||
      timestamp != (time_t)12345 || parse_time_checked("12345x", &timestamp) ||
      !parse_float_checked(" 1.25 ", &float_value) ||
      fabsf(float_value - 1.25F) > 0.0001F ||
      parse_float_checked("nan", &float_value) ||
      parse_float_checked("1.0 trailing", &float_value)) {
    return 1;
  }
  return 0;
}

static int test_system_error_message(void) {
  char known[128];
  char unknown[128];
  char small[1] = {'x'};

  if (system_error_message(EINVAL, known, sizeof(known)) != known ||
      known[0] == '\0' || known[sizeof(known) - 1] != '\0' ||
      system_error_message(INT_MAX, unknown, sizeof(unknown)) != unknown ||
      unknown[0] == '\0' || unknown[sizeof(unknown) - 1] != '\0' ||
      system_error_message(EINVAL, small, sizeof(small)) != small ||
      small[0] != '\0') {
    return 1;
  }
  return 0;
}

static int test_text_builder(void) {
  char text[12];
  BtechTextBuilder builder;
  btech_text_builder_initialize(&builder, text, sizeof(text));
  if (!btech_text_builder_append(&builder, "value=") ||
      !btech_text_builder_append_format(&builder, "%d", 12) ||
      !btech_text_builder_append_character(&builder, '3') ||
      strcmp(text, "value=123") != 0 || builder.truncated) {
    return 1;
  }
  if (btech_text_builder_append(&builder, "-overflow") || !builder.truncated ||
      text[sizeof(text) - 1] != '\0') {
    return 1;
  }
  btech_text_builder_initialize(&builder, nullptr, 10);
  if (btech_text_builder_append(&builder, "text") || !builder.truncated)
    return 1;
  return 0;
}

static int test_checked_conversions(void) {
  if (clamp_int_to_char(CHAR_MIN - 1) != CHAR_MIN ||
      clamp_int_to_char(CHAR_MAX + 1) != CHAR_MAX ||
      clamp_int_to_short(SHRT_MIN - 1) != SHRT_MIN ||
      clamp_int_to_short(SHRT_MAX + 1) != SHRT_MAX ||
      clamp_int_to_unsigned_char(-1) != 0 ||
      clamp_int_to_unsigned_char(UCHAR_MAX + 1) != UCHAR_MAX ||
      clamp_intptr_to_int((intptr_t)INT_MIN - 1) != INT_MIN ||
      clamp_intptr_to_int((intptr_t)INT_MAX + 1) != INT_MAX ||
      clamp_size_to_int((size_t)INT_MAX + 1) != INT_MAX) {
    return 1;
  }
  return 0;
}

int main(void) {
  return test_parsing() || test_system_error_message() ||
         test_text_builder() || test_checked_conversions();
}
