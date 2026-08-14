/* formatting.h - Safe transient-buffer printf formatting helpers. */

#pragma once

void safe_tprintf_str(char *string, char **bp, const char *format, ...)
    __attribute__((format(printf, 3, 4)));
