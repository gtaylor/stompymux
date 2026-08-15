/* stringutil.h - String parsing, comparison, and transformation helpers. */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <time.h>

#include "mux/support/lbuf_text.h"

typedef struct ServerConfiguration ServerConfiguration;

enum { SYSTEM_ERROR_MESSAGE_SIZE = 128 };

/** Renders an error number into caller-owned storage. */
char *system_error_message(int error_number, char *buffer, size_t capacity);
/** Converts one ASCII lowercase letter to uppercase. */
char ascii_to_upper(char character);
/** Converts one ASCII uppercase letter to lowercase. */
char ascii_to_lower(char character);
/** Returns owned lbuf text with whitespace normalized. */
LbufText munge_space(const char *string);
/** Returns owned lbuf text containing trimmed, normalized text. */
LbufText trim_spaces(const char *string);
/** Splits a mutable string at targ and advances the caller's cursor. */
char *grabto(char **str, char targ);
/** Compares strings case-insensitively using the configured space policy. */
int string_compare(const ServerConfiguration *configuration, const char *s1,
                   const char *s2);
/** Returns the matched prefix length, or zero when prefix does not match. */
int string_prefix(const char *string, const char *prefix);
/** Finds a nonempty, case-insensitive match at the start of a word. */
const char *string_match(const char *src, const char *sub);
/** Returns a copy of str with dollar signs converted to spaces. */
char *dollar_to_space(const char *str);
/** Tests whether str is a sufficiently long abbreviation of target. */
int minmatch(const char *str, const char *target, int min);
/** Duplicates a string with malloc; the caller must free the result. */
char *strsave(const char *s);
/** Copies up to max bytes and returns the number of source bytes left. */
int safe_copy_str(const char *src, char *buff, char **bufp, int max);
/** Copies one byte when space remains and reports whether it overflowed. */
int safe_copy_chr(char src, char *buff, char **bufp, int max);
/** Converts all ASCII lowercase letters in a mutable string to uppercase. */
char *upcasestr(char *s);
/** Parses a decimal int, clamping overflow and accepting a numeric prefix. */
int clamped_atoi(const char *str);
/** Parses a decimal long, clamping overflow and accepting a numeric prefix. */
long clamped_atol(const char *str);
/** Parses a complete decimal int without modifying value on failure. */
bool parse_int_checked(const char *text, int *value);
/** Parses a complete decimal long without modifying value on failure. */
bool parse_long_checked(const char *text, long *value);
/** Parses a complete finite float without modifying value on failure. */
bool parse_float_checked(const char *text, float *value);
/** Parses a complete decimal time_t without modifying value on failure. */
bool parse_time_checked(const char *text, time_t *value);
