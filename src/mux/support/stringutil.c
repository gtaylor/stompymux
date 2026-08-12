/*
 * stringutil.c -- string utilities
 */

#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"

/** Returns whether a parser stopped at only trailing whitespace. */
static char checked_character_at(const char *text, size_t length,
                                 size_t index) {
  return *(const char *)checked_storage_at_const(text, length, sizeof(char),
                                                 index);
}

static char *checked_character_slot(char *text, size_t capacity, size_t index) {
  return checked_storage_at(text, capacity, sizeof(char), index);
}

static bool character_is_space(char character) {
  return (isspace)((unsigned char)character) != 0;
}

static bool character_is_alphanumeric(char character) {
  return (isalnum)((unsigned char)character) != 0;
}

static bool parse_finished(const char *end) {
  const size_t LENGTH = strlen(end);
  for (size_t index = 0; index < LENGTH; index++) {
    if (!character_is_space(checked_character_at(end, LENGTH + 1, index)))
      return false;
  }
  return true;
}

/**
 * Parses a complete base-10 long, allowing surrounding whitespace.
 * Returns false for missing, malformed, trailing, or overflowing input and
 * leaves value unchanged on failure.
 */
bool parse_long_checked(const char *text, long *value) {
  if (text == nullptr || value == nullptr)
    return false;

  errno = 0;
  char *end;
  long parsed = strtol(text, &end, 10);
  if (text == end || errno == ERANGE || !parse_finished(end))
    return false;

  *value = parsed;
  return true;
}

/**
 * Parses a complete base-10 int, allowing surrounding whitespace.
 * Returns false when checked long parsing fails or the result exceeds int.
 */
bool parse_int_checked(const char *text, int *value) {
  long parsed;
  if (value == nullptr || !parse_long_checked(text, &parsed) ||
      parsed < INT_MIN || parsed > INT_MAX) {
    return false;
  }

  *value = (int)parsed;
  return true;
}

/**
 * Parses a complete finite float, allowing surrounding whitespace.
 * Returns false for malformed, trailing, out-of-range, or non-finite input.
 */
bool parse_float_checked(const char *text, float *value) {
  if (text == nullptr || value == nullptr)
    return false;

  errno = 0;
  char *end;
  float parsed = strtof(text, &end);
  if (text == end || errno == ERANGE || !isfinite(parsed) ||
      !parse_finished(end)) {
    return false;
  }

  *value = parsed;
  return true;
}

/**
 * Parses a complete base-10 time_t, accepting surrounding whitespace.
 * Returns false for malformed, trailing, overflowing, or unrepresentable input.
 */
bool parse_time_checked(const char *text, time_t *value) {
  long parsed;
  if (value == nullptr || !parse_long_checked(text, &parsed))
    return false;

  const time_t CONVERTED = (time_t)parsed;
  if ((long)CONVERTED != parsed)
    return false;

  *value = CONVERTED;
  return true;
}

/** Converts one ASCII lowercase letter to uppercase, leaving others intact. */
char ascii_to_upper(char character) {
  if (character >= 'a' && character <= 'z')
    return (char)(character - ('a' - 'A'));
  return character;
}

/** Converts one ASCII uppercase letter to lowercase, leaving others intact. */
char ascii_to_lower(char character) {
  if (character >= 'A' && character <= 'Z')
    return (char)(character + ('a' - 'A'));
  return character;
}

/**
 * Parses a base-10 long prefix, clamping overflow to LONG_MIN or LONG_MAX.
 * A null or non-numeric input produces zero; trailing text is ignored.
 */
long clamped_atol(const char *str) {
  char *end;
  long value;

  if (str == nullptr)
    return 0;

  errno = 0;
  value = strtol(str, &end, 10);
  if (errno == ERANGE)
    value = (str[0] == '-') ? LONG_MIN : LONG_MAX;
  return value;
}

/**
 * Parses a base-10 int prefix, clamping overflow to INT_MIN or INT_MAX.
 * A null or non-numeric input produces zero; trailing text is ignored.
 */
int clamped_atoi(const char *str) {
  long value = clamped_atol(str);

  if (value > INT_MAX)
    return INT_MAX;
  if (value < INT_MIN)
    return INT_MIN;
  return (int)value;
}

/** Converts a mutable string to ASCII uppercase in place; accepts nullptr. */
char *upcasestr(char *s) {
  if (s == nullptr)
    return nullptr;
  const size_t LENGTH = strlen(s);
  for (size_t index = 0; index < LENGTH; index++) {
    char *slot = checked_character_slot(s, LENGTH + 1, index);
    *slot = ascii_to_upper(*slot);
  }
  return s;
}

static char *normalize_spaces(const char *string) {
  char *buffer = alloc_lbuf("normalize_spaces");
  if (buffer == nullptr)
    return nullptr;

  size_t output_index = 0;
  bool pending_space = false;
  if (string != nullptr) {
    const size_t LENGTH = strlen(string);
    for (size_t input_index = 0; input_index < LENGTH; input_index++) {
      const char CHARACTER =
          checked_character_at(string, LENGTH + 1, input_index);
      if (character_is_space(CHARACTER)) {
        pending_space = output_index > 0;
      } else {
        if (pending_space && output_index < (size_t)LBUF_SIZE - 1) {
          *checked_character_slot(buffer, LBUF_SIZE, output_index++) = ' ';
        }
        pending_space = false;
        if (output_index < (size_t)LBUF_SIZE - 1) {
          *checked_character_slot(buffer, LBUF_SIZE, output_index++) =
              CHARACTER;
        }
      }
    }
  }
  *checked_character_slot(buffer, LBUF_SIZE, output_index) = '\0';
  return buffer;
}

/**
 * Allocates an lbuf with whitespace runs compressed to single spaces and
 * leading and trailing whitespace removed. The caller must free the lbuf.
 */
char *munge_space(const char *string) { return normalize_spaces(string); }

/**
 * Allocates an lbuf with leading and trailing whitespace removed and internal
 * whitespace runs compressed. The caller must free the lbuf.
 */
char *trim_spaces(const char *string) { return normalize_spaces(string); }

/**
 * Replaces the next targ in a mutable string with a terminator, returns the
 * current field, and advances the caller's pointer to the following field.
 */
char *grabto(char **str, char targ) {
  if (!str || !*str || !**str)
    return nullptr;

  char *field = *str;
  const size_t LENGTH = strlen(field);
  size_t index = 0;
  while (index < LENGTH &&
         checked_character_at(field, LENGTH + 1, index) != targ) {
    index++;
  }
  if (index < LENGTH) {
    *checked_character_slot(field, LENGTH + 1, index) = '\0';
    index++;
  }
  *str = checked_mutable_string_suffix(field, index);
  return field;
}

/**
 * Compares two strings case-insensitively. When space compression is enabled,
 * leading whitespace and the lengths of whitespace runs are ignored.
 */
int string_compare(const ServerConfiguration *configuration, const char *s1,
                   const char *s2) {
  const size_t LENGTH1 = strlen(s1);
  const size_t LENGTH2 = strlen(s2);
  size_t index1 = 0;
  size_t index2 = 0;

  if (!configuration->space_compress) {
    while (index1 < LENGTH1 && index2 < LENGTH2 &&
           ascii_to_lower(checked_character_at(s1, LENGTH1 + 1, index1)) ==
               ascii_to_lower(checked_character_at(s2, LENGTH2 + 1, index2))) {
      index1++;
      index2++;
    }
    return ascii_to_lower(checked_character_at(s1, LENGTH1 + 1, index1)) -
           ascii_to_lower(checked_character_at(s2, LENGTH2 + 1, index2));
  }
  while (index1 < LENGTH1 &&
         character_is_space(checked_character_at(s1, LENGTH1 + 1, index1)))
    index1++;
  while (index2 < LENGTH2 &&
         character_is_space(checked_character_at(s2, LENGTH2 + 1, index2)))
    index2++;
  while (index1 < LENGTH1 && index2 < LENGTH2) {
    const char CHARACTER1 = checked_character_at(s1, LENGTH1 + 1, index1);
    const char CHARACTER2 = checked_character_at(s2, LENGTH2 + 1, index2);
    if (character_is_space(CHARACTER1) && character_is_space(CHARACTER2)) {
      while (index1 < LENGTH1 &&
             character_is_space(checked_character_at(s1, LENGTH1 + 1, index1)))
        index1++;
      while (index2 < LENGTH2 &&
             character_is_space(checked_character_at(s2, LENGTH2 + 1, index2)))
        index2++;
    } else if (ascii_to_lower(CHARACTER1) == ascii_to_lower(CHARACTER2)) {
      index1++;
      index2++;
    } else {
      break;
    }
  }
  if (index1 < LENGTH1 && index2 < LENGTH2)
    return (1);
  while (index1 < LENGTH1 &&
         character_is_space(checked_character_at(s1, LENGTH1 + 1, index1)))
    index1++;
  if (index1 < LENGTH1) {
    return checked_character_at(s1, LENGTH1 + 1, index1);
  }
  while (index2 < LENGTH2 &&
         character_is_space(checked_character_at(s2, LENGTH2 + 1, index2)))
    index2++;
  if (index2 < LENGTH2) {
    return checked_character_at(s2, LENGTH2 + 1, index2);
  }
  return (0);
}

/**
 * Performs a case-insensitive prefix comparison and returns the number of
 * matched characters, or zero if the complete nonempty prefix does not match.
 */
int string_prefix(const char *string, const char *prefix) {
  const size_t STRING_LENGTH = strlen(string);
  const size_t PREFIX_LENGTH = strlen(prefix);
  size_t count = 0;

  while (
      count < STRING_LENGTH && count < PREFIX_LENGTH &&
      ascii_to_lower(checked_character_at(string, STRING_LENGTH + 1, count)) ==
          ascii_to_lower(
              checked_character_at(prefix, PREFIX_LENGTH + 1, count))) {
    count++;
  }
  if (count != PREFIX_LENGTH)
    return 0;
  return count > (size_t)INT_MAX ? INT_MAX : (int)count;
}

/**
 * Finds a nonempty, case-insensitive substring only at the start of a word.
 * Returns a pointer into src, or nullptr when no word matches.
 */
const char *string_match(const char *src, const char *sub) {
  if (src == nullptr || sub == nullptr || *sub == '\0')
    return nullptr;

  const size_t LENGTH = strlen(src);
  size_t index = 0;
  while (index < LENGTH) {
    const char *word = checked_string_suffix(src, index);
    if (string_prefix(word, sub))
      return word;
    while (index < LENGTH && character_is_alphanumeric(
                                 checked_character_at(src, LENGTH + 1, index)))
      index++;
    while (index < LENGTH && !character_is_alphanumeric(
                                 checked_character_at(src, LENGTH + 1, index)))
      index++;
  }
  return nullptr;
}

/**
 * Allocates an lbuf containing string with every occurrence of old replaced
 * by new. The caller must free the returned lbuf.
 */
char *replace_string(const char *old, const char *new, const char *string) {
  if (old == nullptr || new == nullptr || string == nullptr)
    return nullptr;

  const size_t OLD_LENGTH = strlen(old);
  const size_t STRING_LENGTH = strlen(string);
  char *result = alloc_lbuf("replace_string");
  if (result == nullptr)
    return nullptr;
  char *output = result;

  size_t index = 0;
  while (index < STRING_LENGTH) {
    const bool MATCHES =
        OLD_LENGTH > 0 && OLD_LENGTH <= STRING_LENGTH - index &&
        memcmp(old, checked_string_suffix(string, index), OLD_LENGTH) == 0;
    if (MATCHES) {
      safe_str(new, result, &output);
      index += OLD_LENGTH;
    } else {
      safe_chr(checked_character_at(string, STRING_LENGTH + 1, index), result,
               &output);
      index++;
    }
  }
  *output = '\0';
  return result;
}

/**
 * Tests whether str is a case-insensitive abbreviation of target containing
 * at least min characters, unless str exactly consumes target.
 */
int minmatch(const char *str, const char *target, int min) {
  const size_t STR_LENGTH = strlen(str);
  const size_t TARGET_LENGTH = strlen(target);
  size_t index = 0;
  while (index < STR_LENGTH && index < TARGET_LENGTH &&
         ascii_to_lower(checked_character_at(str, STR_LENGTH + 1, index)) ==
             ascii_to_lower(
                 checked_character_at(target, TARGET_LENGTH + 1, index))) {
    index++;
    min--;
  }
  if (index < STR_LENGTH)
    return 0;
  if (index == TARGET_LENGTH)
    return 1;
  return ((min <= 0) ? 1 : 0);
}

/** Duplicates s with malloc; returns nullptr on failure and must be freed. */
char *strsave(const char *s) {
  if (s == nullptr)
    return nullptr;
  const size_t SIZE = strlen(s) + 1;
  char *copy = malloc(SIZE);
  if (copy != nullptr)
    memcpy(copy, s, SIZE);
  return copy;
}

/**
 * Copies as much of src as fits before max, advances bufp, and returns the
 * number of source bytes not copied. This function does not add a terminator.
 */
int safe_copy_str(const char *src, char *buff, char **bufp, int max) {
  if (src == nullptr || buff == nullptr || bufp == nullptr || *bufp == nullptr)
    return 0;
  const uintptr_t BASE_ADDRESS = (uintptr_t)buff;
  const uintptr_t CURSOR_ADDRESS = (uintptr_t)*bufp;
  if (max < 0 || CURSOR_ADDRESS < BASE_ADDRESS ||
      CURSOR_ADDRESS - BASE_ADDRESS > (uintptr_t)max) {
    return strlen(src) > (size_t)INT_MAX ? INT_MAX : (int)strlen(src);
  }

  const size_t SOURCE_LENGTH = strlen(src);
  size_t source_index = 0;
  size_t output_index = (size_t)(CURSOR_ADDRESS - BASE_ADDRESS);
  while (source_index < SOURCE_LENGTH && output_index < (size_t)max) {
    *checked_character_slot(buff, (size_t)max + 1, output_index) =
        checked_character_at(src, SOURCE_LENGTH + 1, source_index);
    source_index++;
    output_index++;
  }
  *bufp = checked_character_slot(buff, (size_t)max + 1, output_index);
  const size_t REMAINING = SOURCE_LENGTH - source_index;
  return REMAINING > (size_t)INT_MAX ? INT_MAX : (int)REMAINING;
}

/**
 * Copies src and advances bufp when its offset is below max. Returns zero on
 * success or one when no space remains; it does not add a terminator.
 */
int safe_copy_chr(char src, char *buff, char **bufp, int max) {
  if (buff == nullptr || bufp == nullptr || *bufp == nullptr || max < 0)
    return 1;
  const uintptr_t BASE_ADDRESS = (uintptr_t)buff;
  const uintptr_t CURSOR_ADDRESS = (uintptr_t)*bufp;
  if (CURSOR_ADDRESS < BASE_ADDRESS ||
      CURSOR_ADDRESS - BASE_ADDRESS > (uintptr_t)max) {
    return 1;
  }

  const size_t OUTPUT_INDEX = (size_t)(CURSOR_ADDRESS - BASE_ADDRESS);
  if (OUTPUT_INDEX >= (size_t)max)
    return 1;
  *checked_character_slot(buff, (size_t)max + 1, OUTPUT_INDEX) = src;
  *bufp = checked_character_slot(buff, (size_t)max + 1, OUTPUT_INDEX + 1);
  return 0;
}
