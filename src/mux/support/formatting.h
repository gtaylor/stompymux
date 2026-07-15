/* formatting.h - Safe transient-buffer printf formatting helpers. */

#pragma once

char *tprintf(const char *format, ...) __attribute__((format(printf, 1, 2)));
void safe_tprintf_str(char *string, char **buffer_pointer, const char *format,
                      ...) __attribute__((format(printf, 3, 4)));
