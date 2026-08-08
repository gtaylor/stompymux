#pragma once

#include <stdbool.h>
#include <stddef.h>

typedef struct BtechTextBuilder {
  char *text;
  size_t capacity;
  size_t length;
  bool truncated;
} BtechTextBuilder;

void btech_text_builder_initialize(BtechTextBuilder *builder, char *text,
                                   size_t capacity);
bool btech_text_builder_append(BtechTextBuilder *builder, const char *text);
bool btech_text_builder_append_count(BtechTextBuilder *builder,
                                     const char *text, size_t length);
bool btech_text_builder_append_character(BtechTextBuilder *builder,
                                         char character);
bool btech_text_builder_append_format(BtechTextBuilder *builder,
                                      const char *format, ...)
    __attribute__((format(printf, 2, 3)));
