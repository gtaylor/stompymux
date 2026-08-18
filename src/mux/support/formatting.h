/** @file
 * Safe transient-buffer printf formatting helpers.
 */
#pragma once

/** Appends formatted text to a bounded transient buffer.
 * @param[in,out] string Destination buffer.
 * @param[in,out] bp Bp. @param[in] format Format. */

void safe_tprintf_str(char *string, char **bp, const char *format, ...)
    __attribute__((format(printf, 3, 4)));
