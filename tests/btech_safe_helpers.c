#include <limits.h>
#include <math.h>
#include <stdbool.h>
#include <string.h>

#include "btech_text_builder.h"
#include "mux/support/stringutil.h"

static int test_parsing(void) {
  int integer = 0;
  long long_value = 0;
  float float_value = 0.0F;

  if (!parse_int_checked(" -42 \t", &integer) || integer != -42 ||
      parse_int_checked("42x", &integer) || parse_int_checked("", &integer) ||
      parse_int_checked("999999999999999999999", &integer) ||
      !parse_long_checked("2147483648", &long_value) ||
      long_value != 2147483648L ||
      !parse_float_checked(" 1.25 ", &float_value) ||
      fabsf(float_value - 1.25F) > 0.0001F ||
      parse_float_checked("nan", &float_value) ||
      parse_float_checked("1.0 trailing", &float_value)) {
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

int main(void) { return test_parsing() || test_text_builder(); }
