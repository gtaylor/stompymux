/*
 * stringutil.c -- string utilities
 */

#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"

#ifdef __linux__
char *___strtok;

#endif

/** Returns whether a parser stopped at only trailing whitespace. */
static bool parse_finished(const char *end) {
  while (isspace((unsigned char)*end))
    end++;
  return *end == '\0';
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

/** Converts one ASCII lowercase letter to uppercase, leaving others intact. */
char ascii_to_upper(char character) {
  if (character >= 'a' && character <= 'z')
    return "ABCDEFGHIJKLMNOPQRSTUVWXYZ"[character - 'a'];
  return character;
}

/** Converts one ASCII uppercase letter to lowercase, leaving others intact. */
char ascii_to_lower(char character) {
  if (character >= 'A' && character <= 'Z')
    return "abcdefghijklmnopqrstuvwxyz"[character - 'A'];
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
  char *p;

  for (p = s; p && *p; p++)
    *p = ascii_to_upper(*p);
  return s;
}

/**
 * Allocates an lbuf with whitespace runs compressed to single spaces and
 * leading and trailing whitespace removed. The caller must free the lbuf.
 */
char *munge_space(char *string) {
  char *buffer, *p, *q;

  buffer = alloc_lbuf("munge_space");
  p = string;
  q = buffer;
  while (p && *p && isspace((unsigned char)*p))
    p++; /*
          * remove inital spaces
          */
  while (p && *p) {
    while (*p && !isspace((unsigned char)*p))
      *q++ = *p++;
    while (*p && isspace((unsigned char)*++p))
      ;
    if (*p)
      *q++ = ' ';
  }
  *q = '\0'; /*
              * remove terminal spaces and terminate * * *
              *
              * * string
              */
  return (buffer);
}

/**
 * Allocates an lbuf with leading and trailing whitespace removed and internal
 * whitespace runs compressed. The caller must free the lbuf.
 */
char *trim_spaces(char *string) {
  char *buffer, *p, *q;

  buffer = alloc_lbuf("trim_spaces");
  p = string;
  q = buffer;
  while (p && *p && isspace((unsigned char)*p)) /*
                                                 * remove inital spaces
                                                 */
    p++;
  while (p && *p) {
    while (*p && !isspace((unsigned char)*p)) /*
                                               * copy nonspace chars
                                               */
      *q++ = *p++;
    while (*p && isspace((unsigned char)*p)) /*
                                              * compress spaces
                                              */
      p++;
    if (*p)
      *q++ = ' '; /*
                   * leave one space
                   */
  }
  *q = '\0'; /*
              * terminate string
              */
  return (buffer);
}

/**
 * Replaces the next targ in a mutable string with a terminator, returns the
 * current field, and advances the caller's pointer to the following field.
 */
char *grabto(char **str, char targ) {
  char *savec, *cp;

  if (!str || !*str || !**str)
    return nullptr;

  savec = cp = *str;
  while (*cp && *cp != targ)
    cp++;
  if (*cp)
    *cp++ = '\0';
  *str = cp;
  return savec;
}

/**
 * Compares two strings case-insensitively. When space compression is enabled,
 * leading whitespace and the lengths of whitespace runs are ignored.
 */
int string_compare(const ServerConfiguration *configuration, const char *s1,
                   const char *s2) {
  if (!configuration->space_compress) {
    while (*s1 && *s2 && ascii_to_lower(*s1) == ascii_to_lower(*s2))
      s1++, s2++;

    return (ascii_to_lower(*s1) - ascii_to_lower(*s2));
  } else {
    while (isspace((unsigned char)*s1))
      s1++;
    while (isspace((unsigned char)*s2))
      s2++;
    while (*s1 && *s2 &&
           ((ascii_to_lower(*s1) == ascii_to_lower(*s2)) ||
            (isspace((unsigned char)*s1) && isspace((unsigned char)*s2)))) {
      if (isspace((unsigned char)*s1) &&
          isspace((unsigned char)*s2)) { /*
                                          * skip all
                                          * other
                                          * spaces
                                          */
        while (isspace((unsigned char)*s1))
          s1++;
        while (isspace((unsigned char)*s2))
          s2++;
      } else {
        s1++;
        s2++;
      }
    }
    if ((*s1) && (*s2))
      return (1);
    if (isspace((unsigned char)*s1)) {
      while (isspace((unsigned char)*s1))
        s1++;
      return (*s1);
    }
    if (isspace((unsigned char)*s2)) {
      while (isspace((unsigned char)*s2))
        s2++;
      return (*s2);
    }
    if ((*s1) || (*s2))
      return (1);
    return (0);
  }
}

/**
 * Performs a case-insensitive prefix comparison and returns the number of
 * matched characters, or zero if the complete nonempty prefix does not match.
 */
int string_prefix(const char *string, const char *prefix) {
  int count = 0;

  while (*string && *prefix &&
         ascii_to_lower(*string) == ascii_to_lower(*prefix))
    string++, prefix++, count++;
  if (*prefix == '\0') /*
                        * Matched all of prefix
                        */
    return (count);
  else
    return (0);
}

/**
 * Finds a nonempty, case-insensitive substring only at the start of a word.
 * Returns a pointer into src, or nullptr when no word matches.
 */
const char *string_match(const char *src, const char *sub) {
  if ((*sub != '\0') && (src)) {
    while (*src) {
      if (string_prefix(src, sub))
        return src;
      /*
       * else scan to beginning of next word
       */
      while (*src && isalnum((unsigned char)*src))
        src++;
      while (*src && !isalnum((unsigned char)*src))
        src++;
    }
  }
  return 0;
}

/**
 * Allocates an lbuf containing string with every occurrence of old replaced
 * by new. The caller must free the returned lbuf.
 */
char *replace_string(const char *old, const char *new, const char *string) {
  char *result, *r;
  const char *s;
  int olen;

  if (string == nullptr)
    return nullptr;
  s = string;
  olen = (int)strlen(old);
  r = result = alloc_lbuf("replace_string");
  while (*s) {

    /*
     * Copy up to the next occurrence of the first char of OLD
     */

    while (*s && *s != *old) {
      safe_chr(*s, result, &r);
      s++;
    }

    /*
     * If we are really at an OLD, append NEW to the result and *
     *
     * *  * *  * * bump the input string past the occurrence of
     * OLD. *  * * * Otherwise, copy the char and try again.
     */

    if (*s) {
      if (!strncmp(old, s, (size_t)olen)) {
        safe_str(new, result, &r);
        s += olen;
      } else {
        safe_chr(*s, result, &r);
        s++;
      }
    }
  }
  *r = '\0';
  return result;
}

/**
 * Tests whether str is a case-insensitive abbreviation of target containing
 * at least min characters, unless str exactly consumes target.
 */
int minmatch(const char *str, const char *target, int min) {
  while (*str && *target && (ascii_to_lower(*str) == ascii_to_lower(*target))) {
    str++;
    target++;
    min--;
  }
  if (*str)
    return 0;
  if (!*target)
    return 1;
  return ((min <= 0) ? 1 : 0);
}

/** Duplicates s with malloc; returns nullptr on failure and must be freed. */
char *strsave(const char *s) {
  char *p;
  p = (char *)malloc(sizeof(char) * (strlen(s) + 1));

  if (p)
    StringCopy(p, s);
  return p;
}

/**
 * Copies as much of src as fits before max, advances bufp, and returns the
 * number of source bytes not copied. This function does not add a terminator.
 */
int safe_copy_str(const char *src, char *buff, char **bufp, int max) {
  char *tp;

  tp = *bufp;
  if (src == nullptr)
    return 0;
  while (*src && ((tp - buff) < max))
    *tp++ = *src++;
  *bufp = tp;
  return (int)strlen(src);
}

/**
 * Copies src and advances bufp when its offset is below max. Returns zero on
 * success or one when no space remains; it does not add a terminator.
 */
int safe_copy_chr(char src, char *buff, char **bufp, int max) {
  char *tp;
  int retval;

  tp = *bufp;
  retval = 0;
  if ((tp - buff) < max) {
    *tp++ = src;
  } else {
    retval = 1;
  }
  *bufp = tp;
  return retval;
}
