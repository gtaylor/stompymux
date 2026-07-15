/* formatting.h - Safe transient-buffer printf formatting helpers. */

#pragma once

char *tprintf(const char *format, ...);
void safe_tprintf_str(char *string, char **buffer_pointer,
                      const char *format, ...);
