/* utf8.h - Strict UTF-8 and printable ASCII helpers. */

#pragma once

#include <stddef.h>
#include <stdint.h>

typedef struct Utf8DecodeResult {
  uint32_t codepoint;
  size_t length;
} Utf8DecodeResult;

typedef struct Utf8ValidatorState {
  uint32_t codepoint;
  uint32_t minimum;
  unsigned int remaining;
} Utf8ValidatorState;

bool utf8_decode(const char *text, size_t length, Utf8DecodeResult *result);
bool utf8_validate(const char *text, size_t length);
bool utf8_validate_printable(const char *text, size_t length);
bool utf8_is_printable_ascii(const char *text, size_t length);
size_t utf8_valid_prefix_length(const char *text, size_t length);
size_t utf8_previous_codepoint_start(const char *text, size_t length);
size_t utf8_copy_truncated(char *destination, size_t destination_size,
                           const char *source);
void utf8_validator_initialize(Utf8ValidatorState *state);
bool utf8_validator_feed(Utf8ValidatorState *state, const char *text,
                         size_t length);
bool utf8_validator_is_complete(const Utf8ValidatorState *state);
size_t utf8_sanitize(char *destination, size_t destination_size,
                     const char *source, size_t source_length);
