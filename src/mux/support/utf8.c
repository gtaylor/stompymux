/* utf8.c - Strict UTF-8 and printable ASCII helpers. */

#include "mux/support/utf8.h"

#include <stdint.h>
#include <string.h>

#include "mux/support/checked_storage.h"

static unsigned char utf8_byte_at(const char *text, size_t length,
                                  size_t index) {
  return *(const unsigned char *)checked_storage_at_const(text, length,
                                                          sizeof(char), index);
}

static const char *utf8_suffix(const char *text, size_t length, size_t offset) {
  return checked_storage_region_const(text, length, offset, length - offset);
}

static char *utf8_destination_suffix(char *text, size_t capacity,
                                     size_t offset) {
  return checked_storage_region(text, capacity, offset, capacity - offset);
}

static void utf8_write_byte(char *text, size_t capacity, size_t index,
                            char value) {
  *(char *)checked_storage_at(text, capacity, sizeof(char), index) = value;
}

static bool utf8_is_continuation(unsigned char byte) {
  return (byte >= 0x80 && byte <= 0xbf) != 0;
}

bool utf8_decode(const char *text, size_t length, Utf8DecodeResult *result) {
  uint32_t codepoint;
  size_t needed;

  if (text == nullptr || result == nullptr || length == 0)
    return false;
  const unsigned char FIRST = utf8_byte_at(text, length, 0);
  if (FIRST <= 0x7f) {
    codepoint = FIRST;
    needed = 1;
  } else if (FIRST >= 0xc2 && FIRST <= 0xdf) {
    codepoint = FIRST & 0x1f;
    needed = 2;
  } else if (FIRST >= 0xe0 && FIRST <= 0xef) {
    codepoint = FIRST & 0x0f;
    needed = 3;
  } else if (FIRST >= 0xf0 && FIRST <= 0xf4) {
    codepoint = FIRST & 0x07;
    needed = 4;
  } else {
    return false;
  }
  if (length < needed)
    return false;
  for (size_t index = 1; index < needed; index++) {
    const unsigned char BYTE = utf8_byte_at(text, length, index);
    if (!utf8_is_continuation(BYTE))
      return false;
    codepoint = (codepoint << 6) | (BYTE & 0x3f);
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
    if (!utf8_decode(utf8_suffix(text, length, offset), length - offset,
                     &decoded))
      return false;
    offset += decoded.length;
  }
  return true;
}

bool utf8_validate_printable(const char *text, size_t length) {
  Utf8DecodeResult decoded;
  size_t offset = 0;

  while (offset < length) {
    if (!utf8_decode(utf8_suffix(text, length, offset), length - offset,
                     &decoded))
      return false;
    if (decoded.codepoint <= 0x1f ||
        (decoded.codepoint >= 0x7f && decoded.codepoint <= 0x9f))
      return false;
    offset += decoded.length;
  }
  return true;
}

bool utf8_is_printable_ascii(const char *text, size_t length) {
  for (size_t index = 0; index < length; index++) {
    const unsigned char BYTE = utf8_byte_at(text, length, index);
    if (BYTE < 0x20 || BYTE > 0x7e)
      return false;
  }
  return true;
}

size_t utf8_valid_prefix_length(const char *text, size_t length) {
  Utf8DecodeResult decoded;
  size_t offset = 0;

  while (offset < length) {
    if (!utf8_decode(utf8_suffix(text, length, offset), length - offset,
                     &decoded))
      break;
    offset += decoded.length;
  }
  return offset;
}

size_t utf8_previous_codepoint_start(const char *text, size_t length) {
  size_t start;

  if (length == 0)
    return 0;
  start = length - 1;
  while (start > 0 && utf8_is_continuation(utf8_byte_at(text, length, start)))
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
  utf8_write_byte(destination, destination_size, 0, '\0');
  if (source == nullptr)
    return 0;
  source_length = strlen(source);
  while (source_offset < source_length &&
         utf8_decode(utf8_suffix(source, source_length, source_offset),
                     source_length - source_offset, &decoded)) {
    if (decoded.length >= destination_size - destination_offset)
      break;
    memcpy(utf8_destination_suffix(destination, destination_size,
                                   destination_offset),
           utf8_suffix(source, source_length, source_offset), decoded.length);
    source_offset += decoded.length;
    destination_offset += decoded.length;
  }
  utf8_write_byte(destination, destination_size, destination_offset, '\0');
  return destination_offset;
}

void utf8_validator_initialize(Utf8ValidatorState *state) {
  memset(state, 0, sizeof(*state));
}

bool utf8_validator_feed(Utf8ValidatorState *state, const char *text,
                         size_t length) {
  for (size_t index = 0; index < length; index++) {
    unsigned char byte = utf8_byte_at(text, length, index);

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
  static const char REPLACEMENT[] = "\xef\xbf\xbd";
  Utf8DecodeResult decoded;
  size_t source_offset = 0;
  size_t destination_offset = 0;

  if (destination_size == 0)
    return 0;
  while (source_offset < source_length) {
    const char *value;
    size_t value_length;

    if (utf8_decode(utf8_suffix(source, source_length, source_offset),
                    source_length - source_offset, &decoded)) {
      value = utf8_suffix(source, source_length, source_offset);
      value_length = decoded.length;
      source_offset += decoded.length;
    } else {
      value = REPLACEMENT;
      value_length = sizeof(REPLACEMENT) - 1;
      source_offset++;
    }
    if (value_length >= destination_size - destination_offset)
      break;
    memcpy(utf8_destination_suffix(destination, destination_size,
                                   destination_offset),
           value, value_length);
    destination_offset += value_length;
  }
  utf8_write_byte(destination, destination_size, destination_offset, '\0');
  return destination_offset;
}
