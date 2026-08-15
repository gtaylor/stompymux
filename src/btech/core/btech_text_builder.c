#include "btech_text_builder.h"

#include "mux/support/checked_storage.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

void btech_text_builder_initialize(BtechTextBuilder *builder, char *text,
                                   size_t capacity) {
  if (builder == nullptr)
    return;
  if (text == nullptr)
    capacity = 0;
  *builder = (BtechTextBuilder){
      .text = text,
      .capacity = capacity,
  };
  if (capacity > 0)
    *(char *)checked_storage_at(text, capacity, sizeof(char), 0) = '\0';
}

bool btech_text_builder_append(BtechTextBuilder *builder, const char *text) {
  return (text != nullptr
              ? btech_text_builder_append_count(builder, text, strlen(text))
              : false) != 0;
}

bool btech_text_builder_append_count(BtechTextBuilder *builder,
                                     const char *text, size_t length) {
  if (builder == nullptr || text == nullptr || builder->capacity == 0) {
    if (builder != nullptr)
      builder->truncated = true;
    return false;
  }

  if (builder->length >= builder->capacity) {
    builder->truncated = true;
    return false;
  }
  const size_t AVAILABLE = builder->capacity - builder->length;
  const size_t COPIED = length < AVAILABLE ? length : AVAILABLE - 1;
  memcpy(checked_storage_region(builder->text, builder->capacity,
                                builder->length, COPIED),
         text, COPIED);
  builder->length += COPIED;
  *(char *)checked_storage_at(builder->text, builder->capacity, sizeof(char),
                              builder->length) = '\0';
  if (COPIED != length)
    builder->truncated = true;
  return (!builder->truncated) != 0;
}

bool btech_text_builder_append_character(BtechTextBuilder *builder,
                                         char character) {
  const char TEXT[] = {character, '\0'};
  return btech_text_builder_append(builder, TEXT);
}

bool btech_text_builder_append_format(BtechTextBuilder *builder,
                                      const char *format, ...) {
  if (builder == nullptr || format == nullptr || builder->capacity == 0) {
    if (builder != nullptr)
      builder->truncated = true;
    return false;
  }

  if (builder->length >= builder->capacity) {
    builder->truncated = true;
    return false;
  }
  const size_t AVAILABLE = builder->capacity - builder->length;
  va_list arguments;
  va_start(arguments, format);
  const int COUNT = vsnprintf( // NOLINT(clang-analyzer-security.VAList)
      checked_storage_region(builder->text, builder->capacity, builder->length,
                             AVAILABLE),
      AVAILABLE, format, arguments);
  va_end(arguments);

  if (COUNT < 0 || (size_t)COUNT >= AVAILABLE) {
    builder->length = builder->capacity - 1;
    builder->truncated = true;
    return false;
  }
  builder->length += (size_t)COUNT;
  return true;
}
