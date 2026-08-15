/* Bounded formatting helpers for player notification lines. */

#pragma once

#include "mux/server/platform.h"

#include <stdarg.h>
#include <stdio.h>

[[nodiscard, gnu::format(printf, 3, 0)]] static inline bool
network_output_format_line_v(char *buffer, size_t size, const char *format,
                             va_list arguments) {
  if (size == 0)
    return false;

  buffer[0] = '\0';
  if (size < 3)
    return false;

  const size_t FORMAT_SIZE = size - 2;
  const int FORMATTED_LENGTH =
      vsnprintf( // NOLINT(clang-analyzer-security.VAList)
          buffer, FORMAT_SIZE, format, arguments);
  const bool FORMAT_FITS =
      FORMATTED_LENGTH >= 0 && (size_t)FORMATTED_LENGTH < FORMAT_SIZE;
  const bool SUFFIX_FITS = string_append_bounded(buffer, size, "\r\n");
  return FORMAT_FITS && SUFFIX_FITS;
}
