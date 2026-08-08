#include "btech_text_builder.h"

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
    text[0] = '\0';
}

bool btech_text_builder_append(BtechTextBuilder *builder, const char *text) {
  if (builder == nullptr || text == nullptr || builder->capacity == 0) {
    if (builder != nullptr)
      builder->truncated = true;
    return false;
  }

  const size_t available = builder->capacity - builder->length;
  const size_t requested = strlen(text);
  const size_t copied = requested < available ? requested : available - 1;
  memcpy(builder->text + builder->length, text, copied);
  builder->length += copied;
  builder->text[builder->length] = '\0';
  if (copied != requested)
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

  const size_t available = builder->capacity - builder->length;
  va_list arguments;
  va_start(arguments, format);
  const int count = vsnprintf( // NOLINT(clang-analyzer-security.VAList)
      builder->text + builder->length, available, format, arguments);
  va_end(arguments);

  if (count < 0 || (size_t)count >= available) {
    builder->length = builder->capacity - 1;
    builder->truncated = true;
    return false;
  }
  builder->length += (size_t)count;
  return true;
}
