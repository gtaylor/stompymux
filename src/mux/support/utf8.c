/* utf8.c - Strict UTF-8 and printable ASCII helpers. */

#include "mux/support/utf8.h"

#include <string.h>

static bool utf8_is_continuation(unsigned char byte) {
  return byte >= 0x80 && byte <= 0xbf;
}

bool utf8_decode(const char *text, size_t length, Utf8DecodeResult *result) {
  const unsigned char *bytes = (const unsigned char *)text;
  uint32_t codepoint;
  size_t needed;

  if (length == 0)
    return false;
  if (bytes[0] <= 0x7f) {
    codepoint = bytes[0];
    needed = 1;
  } else if (bytes[0] >= 0xc2 && bytes[0] <= 0xdf) {
    codepoint = bytes[0] & 0x1f;
    needed = 2;
  } else if (bytes[0] >= 0xe0 && bytes[0] <= 0xef) {
    codepoint = bytes[0] & 0x0f;
    needed = 3;
  } else if (bytes[0] >= 0xf0 && bytes[0] <= 0xf4) {
    codepoint = bytes[0] & 0x07;
    needed = 4;
  } else {
    return false;
  }
  if (length < needed)
    return false;
  for (size_t index = 1; index < needed; index++) {
    if (!utf8_is_continuation(bytes[index]))
      return false;
    codepoint = (codepoint << 6) | (bytes[index] & 0x3f);
  }
  if ((needed == 3 && codepoint < 0x800) ||
      (needed == 4 && codepoint < 0x10000) ||
      (codepoint >= 0xd800 && codepoint <= 0xdfff) || codepoint > 0x10ffff)
    return false;
  result->codepoint = codepoint;
  result->length = needed;
  return true;
}

bool utf8_validate(const char *text, size_t length) {
  Utf8DecodeResult decoded;
  size_t offset = 0;

  while (offset < length) {
    if (!utf8_decode(text + offset, length - offset, &decoded))
      return false;
    offset += decoded.length;
  }
  return true;
}

bool utf8_validate_printable(const char *text, size_t length) {
  Utf8DecodeResult decoded;
  size_t offset = 0;

  while (offset < length) {
    if (!utf8_decode(text + offset, length - offset, &decoded))
      return false;
    if (decoded.codepoint <= 0x1f ||
        (decoded.codepoint >= 0x7f && decoded.codepoint <= 0x9f))
      return false;
    offset += decoded.length;
  }
  return true;
}

bool utf8_is_printable_ascii(const char *text, size_t length) {
  const unsigned char *bytes = (const unsigned char *)text;

  for (size_t index = 0; index < length; index++) {
    if (bytes[index] < 0x20 || bytes[index] > 0x7e)
      return false;
  }
  return true;
}

size_t utf8_valid_prefix_length(const char *text, size_t length) {
  Utf8DecodeResult decoded;
  size_t offset = 0;

  while (offset < length) {
    if (!utf8_decode(text + offset, length - offset, &decoded))
      break;
    offset += decoded.length;
  }
  return offset;
}

size_t utf8_previous_codepoint_start(const char *text, size_t length) {
  const unsigned char *bytes = (const unsigned char *)text;
  size_t start;

  if (length == 0)
    return 0;
  start = length - 1;
  while (start > 0 && utf8_is_continuation(bytes[start]))
    start--;
  return start;
}

size_t utf8_copy_truncated(char *destination, size_t destination_size,
                           const char *source) {
  Utf8DecodeResult decoded;
  size_t source_length;
  size_t source_offset = 0;
  size_t destination_offset = 0;

  if (destination_size == 0)
    return 0;
  destination[0] = '\0';
  if (source == nullptr)
    return 0;
  source_length = strlen(source);
  while (source_offset < source_length &&
         utf8_decode(source + source_offset, source_length - source_offset,
                     &decoded)) {
    if (destination_offset + decoded.length >= destination_size)
      break;
    memcpy(destination + destination_offset, source + source_offset,
           decoded.length);
    source_offset += decoded.length;
    destination_offset += decoded.length;
  }
  destination[destination_offset] = '\0';
  return destination_offset;
}

void utf8_validator_initialize(Utf8ValidatorState *state) {
  memset(state, 0, sizeof(*state));
}

bool utf8_validator_feed(Utf8ValidatorState *state, const char *text,
                         size_t length) {
  const unsigned char *bytes = (const unsigned char *)text;

  for (size_t index = 0; index < length; index++) {
    unsigned char byte = bytes[index];

    if (state->remaining == 0) {
      if (byte <= 0x7f)
        continue;
      if (byte >= 0xc2 && byte <= 0xdf) {
        state->codepoint = byte & 0x1f;
        state->minimum = 0x80;
        state->remaining = 1;
      } else if (byte >= 0xe0 && byte <= 0xef) {
        state->codepoint = byte & 0x0f;
        state->minimum = 0x800;
        state->remaining = 2;
      } else if (byte >= 0xf0 && byte <= 0xf4) {
        state->codepoint = byte & 0x07;
        state->minimum = 0x10000;
        state->remaining = 3;
      } else {
        return false;
      }
      continue;
    }
    if (!utf8_is_continuation(byte))
      return false;
    state->codepoint = (state->codepoint << 6) | (byte & 0x3f);
    state->remaining--;
    if (state->remaining == 0 &&
        (state->codepoint < state->minimum ||
         (state->codepoint >= 0xd800 && state->codepoint <= 0xdfff) ||
         state->codepoint > 0x10ffff))
      return false;
  }
  return true;
}

bool utf8_validator_is_complete(const Utf8ValidatorState *state) {
  return state->remaining == 0;
}

size_t utf8_sanitize(char *destination, size_t destination_size,
                     const char *source, size_t source_length) {
  static const char replacement[] = "\xef\xbf\xbd";
  Utf8DecodeResult decoded;
  size_t source_offset = 0;
  size_t destination_offset = 0;

  if (destination_size == 0)
    return 0;
  while (source_offset < source_length) {
    const char *value;
    size_t value_length;

    if (utf8_decode(source + source_offset, source_length - source_offset,
                    &decoded)) {
      value = source + source_offset;
      value_length = decoded.length;
      source_offset += decoded.length;
    } else {
      value = replacement;
      value_length = sizeof(replacement) - 1;
      source_offset++;
    }
    if (destination_offset + value_length >= destination_size)
      break;
    memcpy(destination + destination_offset, value, value_length);
    destination_offset += value_length;
  }
  destination[destination_offset] = '\0';
  return destination_offset;
}
