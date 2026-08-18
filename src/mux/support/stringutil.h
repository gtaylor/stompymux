/** @file
 * String parsing, comparison, and transformation helpers.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <time.h>

#include "mux/support/owned_text.h"

typedef struct ServerConfiguration ServerConfiguration;

enum { SYSTEM_ERROR_MESSAGE_SIZE = 128 };

/** Renders an error number into caller-owned storage. @param[in] error_number
 * Error number. @param[out] buffer Caller-owned output storage. @param[in]
 * capacity Capacity. */

char *system_error_message(int error_number, char *buffer, size_t capacity);
/** Converts one ASCII lowercase letter to uppercase. @param[in] character
 * Character. */

char ascii_to_upper(char character);
/** Converts one ASCII uppercase letter to lowercase. @param[in] character
 * Character. */

char ascii_to_lower(char character);
/** Returns owned lbuf text with whitespace normalized. @param[in] string String
 * to process. */

OwnedText munge_space(const char *string);
/** Returns owned lbuf text containing trimmed, normalized text. @param[in]
 * string String to process. */

OwnedText trim_spaces(const char *string);
/** Splits a mutable string at targ and advances the caller's cursor.
 * @param[in,out] str String to process. @param[in] targ Targ. */

char *grabto(char **str, char targ);
/** Compares strings case-insensitively using the configured space policy.
 * @param[in] configuration Server configuration. @param[in] s1 S1. @param[in]
 * s2 S2. */

int string_compare(const ServerConfiguration *configuration, const char *s1,
                   const char *s2);
/** Returns the matched prefix length, or zero when prefix does not match.
 * @param[in] string String to process. @param[in] prefix Prefix. */

int string_prefix(const char *string, const char *prefix);
/** Finds a nonempty, case-insensitive match at the start of a word. @param[in]
 * src Src. @param[in] sub Sub. */

const char *string_match(const char *src, const char *sub);
/** Returns a copy of str with dollar signs converted to spaces. @param[in] str
 * String to process. */

char *dollar_to_space(const char *str);
/** Tests whether str is a sufficiently long abbreviation of target. @param[in]
 * str String to process. @param[in] target Target object or value. @param[in]
 * min Min. */

int minmatch(const char *str, const char *target, int min);
/** Duplicates a string with malloc; the caller must free the result. @param[in]
 * s String or object to process. */

char *strsave(const char *s);
/** Copies up to max bytes and returns the number of source bytes left.
 * @param[in] src Src. @param[out] buff Caller-owned output storage.
 * @param[in,out] bufp Current output cursor. @param[in] max Max. */

int safe_copy_str(const char *src, char *buff, char **bufp, int max);
/** Copies one byte when space remains and reports whether it overflowed.
 * @param[in] src Src. @param[out] buff Caller-owned output storage.
 * @param[in,out] bufp Current output cursor. @param[in] max Max. */

int safe_copy_chr(char src, char *buff, char **bufp, int max);
/** Converts all ASCII lowercase letters in a mutable string to uppercase.
 * @param[in,out] s String or object to process. */

char *upcasestr(char *s);
/** Parses a decimal int, clamping overflow and accepting a numeric prefix.
 * @param[in] str String to process. */

int clamped_atoi(const char *str);
/** Parses a decimal long, clamping overflow and accepting a numeric prefix.
 * @param[in] str String to process. */

long clamped_atol(const char *str);
/** Parses a complete decimal int without modifying value on failure. @param[in]
 * text Text to process. @param[in] value Value to use. */

bool parse_int_checked(const char *text, int *value);
/** Parses a complete decimal long without modifying value on failure.
 * @param[in] text Text to process. @param[in] value Value to use. */

bool parse_long_checked(const char *text, long *value);
/** Parses a complete finite float without modifying value on failure.
 * @param[in] text Text to process. @param[in] value Value to use. */

bool parse_float_checked(const char *text, float *value);
/** Parses a complete decimal time_t without modifying value on failure.
 * @param[in] text Text to process. @param[in] value Value to use. */

bool parse_time_checked(const char *text, time_t *value);
