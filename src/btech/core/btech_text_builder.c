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
  return text != nullptr
             ? btech_text_builder_append_count(builder, text, strlen(text))
             : false;
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
  const size_t available = builder->capacity - builder->length;
  const size_t copied = length < available ? length : available - 1;
  memcpy(checked_storage_region(builder->text, builder->capacity,
                                builder->length, copied),
         text, copied);
  builder->length += copied;
  *(char *)checked_storage_at(builder->text, builder->capacity, sizeof(char),
                              builder->length) = '\0';
  if (copied != length)
    builder->truncated = true;
  return !builder->truncated;
}

bool btech_text_builder_append_character(BtechTextBuilder *builder,
                                         char character) {
  const char text[] = {character, '\0'};
  return btech_text_builder_append(builder, text);
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
  const size_t available = builder->capacity - builder->length;
  va_list arguments;
  va_start(arguments, format);
  const int count = vsnprintf( // NOLINT(clang-analyzer-security.VAList)
      checked_storage_region(builder->text, builder->capacity, builder->length,
                             available),
      available, format, arguments);
  va_end(arguments);

  if (count < 0 || (size_t)count >= available) {
    builder->length = builder->capacity - 1;
    builder->truncated = true;
    return false;
  }
  builder->length += (size_t)count;
  return true;
}
