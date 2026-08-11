/* utf8.c -- UTF-8 and printable ASCII unit tests. */

#include <string.h>

#include "mux/support/utf8.h"

int main(void) {
  char copied[6];
  char sanitized[16];
  Utf8ValidatorState validator;
  static const char embedded_nul[] = {'a', '\0', 'b'};
  static const char truncated[] = {(char)0xe2, (char)0x82};
  static const char overlong[] = {(char)0xc0, (char)0xaf};
  static const char surrogate[] = {(char)0xed, (char)0xa0, (char)0x80};
  static const char too_large[] = {(char)0xf4, (char)0x90, (char)0x80,
                                   (char)0x80};
  static const char del[] = {0x7f};
  static const char c1_control[] = {(char)0xc2, (char)0x80};
  static const char text[] = "caf\xc3\xa9 \xf0\x9f\x98\x80";

  if (!utf8_validate("", 0) || !utf8_validate(text, strlen(text)) ||
      utf8_validate(truncated, sizeof(truncated)) ||
      utf8_validate(overlong, sizeof(overlong)) ||
      utf8_validate(surrogate, sizeof(surrogate)) ||
      utf8_validate(too_large, sizeof(too_large)))
    return 1;
  if (!utf8_validate_printable(text, strlen(text)) ||
      utf8_validate_printable(embedded_nul, sizeof(embedded_nul)) ||
      utf8_validate_printable(del, sizeof(del)) ||
      utf8_validate_printable(c1_control, sizeof(c1_control)))
    return 1;
  if (!utf8_is_printable_ascii("", 0) ||
      !utf8_is_printable_ascii("A B!~", strlen("A B!~")) ||
      utf8_is_printable_ascii(text, strlen(text)) ||
      utf8_is_printable_ascii(embedded_nul, sizeof(embedded_nul)) ||
      utf8_is_printable_ascii(del, sizeof(del)))
    return 1;
  if (utf8_valid_prefix_length(text, strlen(text)) != strlen(text) ||
      utf8_valid_prefix_length(truncated, sizeof(truncated)) != 0 ||
      utf8_previous_codepoint_start(text, strlen(text)) != strlen(text) - 4)
    return 1;
  if (utf8_copy_truncated(copied, sizeof(copied), "caf\xc3\xa9!") != 5 ||
      strcmp(copied, "caf\xc3\xa9") != 0)
    return 1;
  utf8_validator_initialize(&validator);
  if (!utf8_validator_feed(&validator, "\xf0\x9f", 2) ||
      utf8_validator_is_complete(&validator) ||
      !utf8_validator_feed(&validator, "\x98\x80", 2) ||
      !utf8_validator_is_complete(&validator))
    return 1;
  utf8_validator_initialize(&validator);
  if (utf8_validator_feed(&validator, overlong, sizeof(overlong)))
    return 1;
  if (utf8_sanitize(sanitized, sizeof(sanitized), "x\xc0\xafy", 4) != 8 ||
      strcmp(sanitized, "x\xef\xbf\xbd\xef\xbf\xbdy") != 0)
    return 1;
  return 0;
}
