/*
 * wild.c - wildcard routines
 * *
 * * Written by T. Alexander Popiel, 24 June 1993
 * * Last modified by T. Alexander Popiel, 19 August 1993
 * *
 * * Thanks go to Andrew Molitor for debugging
 * * Thanks also go to Rich $alz for code to benchmark against
 * *
 * * Copyright (c) 1993 by T. Alexander Popiel
 * * This code is hereby placed under GNU copyleft,
 * * see docs/COPYRIGHT.md for details.
 * *
 */

#include <ctype.h>
#include <stdlib.h>
#include <string.h>

#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mux/support/wild.h"

static inline char fixcase(char a) { return ascii_to_lower(a); }

static inline bool is_equal(char a, char b) {
  return (a == b) || (fixcase(a) == fixcase(b));
}

static inline bool is_notequal(char a, char b) {
  return (a != b) && (fixcase(a) != fixcase(b));
}

typedef struct WildcardContext WildcardContext;
struct WildcardContext {
  char **arguments;
  int argument_count;
};

typedef struct WildCursor {
  const char *text;
  size_t length;
  size_t offset;
} WildCursor;

static WildCursor wild_cursor(const char *text) {
  return (WildCursor){.text = text, .length = strlen(text), .offset = 0};
}

static char wild_cursor_current(const WildCursor *cursor) {
  if (cursor->offset == cursor->length)
    return '\0';
  return *(const char *)checked_storage_at_const(cursor->text, cursor->length,
                                                 sizeof(char), cursor->offset);
}

static void wild_cursor_advance(WildCursor *cursor) {
  if (cursor->offset < cursor->length)
    cursor->offset++;
}

static const char *wild_cursor_suffix(const WildCursor *cursor,
                                      size_t additional) {
  return checked_string_suffix(cursor->text, cursor->offset + additional);
}

static char *wild_argument_at(const WildcardContext *context, int argument) {
  return *(char *const *)checked_storage_at_const(
      context->arguments, (size_t)context->argument_count,
      sizeof(*context->arguments), (size_t)argument);
}

static void wild_capture_character(const WildcardContext *context, int argument,
                                   char character) {
  char *capture = wild_argument_at(context, argument);
  *(char *)checked_storage_at(capture, LBUF_SIZE, sizeof(char), 0) = character;
  *(char *)checked_storage_at(capture, LBUF_SIZE, sizeof(char), 1) = '\0';
}

/**
 * Do a wildcard match, without remembering the wild data.
 * This routine will cause crashes if fed NULLs instead of strings.
 */
int quick_wild(const char *tstr, const char *dstr) {
  WildCursor pattern = wild_cursor(tstr);
  WildCursor data = wild_cursor(dstr);

  while (wild_cursor_current(&pattern) != '*') {
    switch (wild_cursor_current(&pattern)) {
    case '?':
      /*
       * Single character match.  Return false if at * end
       * * * * of data.
       */
      if (!wild_cursor_current(&data))
        return 0;
      break;
    case '\\':
      /*
       * Escape character.  Move up, and force literal * *
       * * * match of next character.
       */
      wild_cursor_advance(&pattern);
      /*
       * FALL THROUGH
       */
      [[fallthrough]];
    default:
      /*
       * Literal character.  Check for a match. * If * * *
       * matching end of data, return true.
       */
      if (is_notequal(wild_cursor_current(&data),
                      wild_cursor_current(&pattern)))
        return 0;
      if (!wild_cursor_current(&data))
        return 1;
    }
    wild_cursor_advance(&pattern);
    wild_cursor_advance(&data);
  }

  /*
   * Skip over '*'.
   */

  wild_cursor_advance(&pattern);

  /*
   * Return true on trailing '*'.
   */

  if (!wild_cursor_current(&pattern))
    return 1;

  /*
   * Skip over wildcards.
   */

  while (wild_cursor_current(&pattern) == '?' ||
         wild_cursor_current(&pattern) == '*') {
    if (wild_cursor_current(&pattern) == '?') {
      if (!wild_cursor_current(&data))
        return 0;
      wild_cursor_advance(&data);
    }
    wild_cursor_advance(&pattern);
  }

  /*
   * Skip over a backslash in the pattern string if it is there.
   */

  if (wild_cursor_current(&pattern) == '\\')
    wild_cursor_advance(&pattern);

  /*
   * Return true on trailing '*'.
   */

  if (!wild_cursor_current(&pattern))
    return 1;

  /*
   * Scan for possible matches.
   */

  while (wild_cursor_current(&data)) {
    if (is_equal(wild_cursor_current(&data), wild_cursor_current(&pattern)) &&
        quick_wild(wild_cursor_suffix(&pattern, 1),
                   wild_cursor_suffix(&data, 1)))
      return 1;
    wild_cursor_advance(&data);
  }
  return 0;
}

/**
 * wild1: INTERNAL: do a wildcard match, remembering the wild data.
 *
 * DO NOT CALL THIS FUNCTION DIRECTLY - DOING SO MAY RESULT IN
 * SERVER CRASHES AND IMPROPER ARGUMENT RETURN.
 *
 * Captures are stored in the stack-owned context supplied by wild().
 */
static int wild1(WildcardContext *context, const char *tstr, const char *dstr,
                 int arg) {
  WildCursor pattern = wild_cursor(tstr);
  WildCursor data = wild_cursor(dstr);
  size_t data_capture_offset;
  int argpos, numextra;

  while (wild_cursor_current(&pattern) != '*') {
    switch (wild_cursor_current(&pattern)) {
    case '?':
      /*
       * Single character match.  Return false if at * end
       * * * * of data.
       */
      if (!wild_cursor_current(&data))
        return 0;
      wild_capture_character(context, arg, wild_cursor_current(&data));
      arg++;

      /*
       * Jump to the fast routine if we can.
       */

      if (arg >= context->argument_count)
        return quick_wild(wild_cursor_suffix(&pattern, 1),
                          wild_cursor_suffix(&data, 1));
      break;
    case '\\':
      /*
       * Escape character.  Move up, and force literal * *
       * * * match of next character.
       */
      wild_cursor_advance(&pattern);
      /*
       * FALL THROUGH
       */
      [[fallthrough]];
    default:
      /*
       * Literal character.  Check for a match. * If * * *
       * matching end of data, return true.
       */
      if (is_notequal(wild_cursor_current(&data),
                      wild_cursor_current(&pattern)))
        return 0;
      if (!wild_cursor_current(&data))
        return 1;
    }
    wild_cursor_advance(&pattern);
    wild_cursor_advance(&data);
  }

  /*
   * If at end of pattern, slurp the rest, and leave.
   */

  if (!*wild_cursor_suffix(&pattern, 1)) {
    char *capture = wild_argument_at(context, arg);
    StringCopyTrunc(capture, wild_cursor_suffix(&data, 0), LBUF_SIZE - 1);
    *(char *)checked_storage_at(capture, LBUF_SIZE, sizeof(char),
                                LBUF_SIZE - 1) = '\0';
    return 1;
  }
  /*
   * Remember current position for filling in the '*' return.
   */

  data_capture_offset = data.offset;
  argpos = arg;

  /*
   * Scan forward until we find a non-wildcard.
   */

  do {
    if (argpos < arg) {
      /*
       * Fill in arguments if someone put another '*' * * *
       *
       * * before a fixed string.
       */
      *(char *)checked_storage_at(wild_argument_at(context, argpos), LBUF_SIZE,
                                  sizeof(char), 0) = '\0';
      argpos++;

      /*
       * Jump to the fast routine if we can.
       */

      if (argpos >= context->argument_count)
        return quick_wild(wild_cursor_suffix(&pattern, 0),
                          wild_cursor_suffix(&data, 0));

      /*
       * Fill in any intervening '?'s
       */

      while (argpos < arg) {
        char character = *(const char *)checked_storage_at_const(
            data.text, data.length, sizeof(char), data_capture_offset);
        wild_capture_character(context, argpos, character);
        data_capture_offset++;
        argpos++;

        /*
         * Jump to the fast routine if we can.
         */

        if (argpos >= context->argument_count)
          return quick_wild(wild_cursor_suffix(&pattern, 0),
                            wild_cursor_suffix(&data, 0));
      }
    }
    /*
     * Skip over the '*' for now...
     */

    wild_cursor_advance(&pattern);
    arg++;

    /*
     * Skip over '?'s for now...
     */

    numextra = 0;
    while (wild_cursor_current(&pattern) == '?') {
      if (!wild_cursor_current(&data))
        return 0;
      wild_cursor_advance(&pattern);
      wild_cursor_advance(&data);
      arg++;
      numextra++;
    }
  } while (wild_cursor_current(&pattern) == '*');

  /*
   * Skip over a backslash in the pattern string if it is there.
   */

  if (wild_cursor_current(&pattern) == '\\')
    wild_cursor_advance(&pattern);

  /*
   * Check for possible matches.  This loop terminates either at * end
   * * * * of data (resulting in failure), or at a successful match.
   */
  while (1) {

    /*
     * Scan forward until first character matches.
     */

    if (wild_cursor_current(&pattern))
      while (is_notequal(wild_cursor_current(&data),
                         wild_cursor_current(&pattern))) {
        if (!wild_cursor_current(&data))
          return 0;
        wild_cursor_advance(&data);
      }
    else
      while (wild_cursor_current(&data))
        wild_cursor_advance(&data);

    /*
     * The first character matches, now.  Check if the rest * * *
     *
     * * does, using the fastest method, as usual.
     */
    if (!wild_cursor_current(&data) ||
        ((arg < context->argument_count)
             ? wild1(context, wild_cursor_suffix(&pattern, 1),
                     wild_cursor_suffix(&data, 1), arg)
             : quick_wild(wild_cursor_suffix(&pattern, 1),
                          wild_cursor_suffix(&data, 1)))) {

      /*
       * Found a match!  Fill in all remaining arguments. *
       *
       * *  * *  * * First do the '*'...
       */
      size_t capture_length =
          data.offset - data_capture_offset - (size_t)numextra;
      char *capture = wild_argument_at(context, argpos);
      StringCopyTrunc(capture,
                      checked_string_suffix(data.text, data_capture_offset),
                      capture_length);
      *(char *)checked_storage_at(capture, LBUF_SIZE, sizeof(char),
                                  capture_length) = '\0';
      data_capture_offset = data.offset - (size_t)numextra;
      argpos++;

      /*
       * Fill in any trailing '?'s that are left.
       */

      while (numextra) {
        if (argpos >= context->argument_count)
          return 1;
        char character = *(const char *)checked_storage_at_const(
            data.text, data.length, sizeof(char), data_capture_offset);
        wild_capture_character(context, argpos, character);
        data_capture_offset++;
        argpos++;
        numextra--;
      }

      /*
       * It's done!
       */

      return 1;
    } else {
      wild_cursor_advance(&data);
    }
  }
}

/**
 * wild: do a wildcard match, remembering the wild data.
 *
 * This routine will cause crashes if fed NULLs instead of strings.
 *
 * This function may crash if alloc_lbuf() fails.
 *
 * Capture recursion is scoped to this invocation.
 */
int wild(const char *tstr, const char *dstr, char *args[], int nargs) {
  int i, value;
  WildcardContext context = {.arguments = args, .argument_count = nargs};
  WildCursor pattern = wild_cursor(tstr);
  WildCursor data = wild_cursor(dstr);

  /*
   * Initialize the return array.
   */

  for (i = 0; i < nargs; i++)
    *(char **)checked_storage_at(args, (size_t)nargs, sizeof(*args),
                                 (size_t)i) = nullptr;

  /*
   * Do fast match.
   */

  while (wild_cursor_current(&pattern) != '*' &&
         wild_cursor_current(&pattern) != '?') {
    if (wild_cursor_current(&pattern) == '\\')
      wild_cursor_advance(&pattern);
    if (is_notequal(wild_cursor_current(&data), wild_cursor_current(&pattern)))
      return 0;
    if (!wild_cursor_current(&data))
      return 1;
    wild_cursor_advance(&pattern);
    wild_cursor_advance(&data);
  }

  /*
   * Allocate space for the return args.
   */

  i = 0;
  WildCursor scan = pattern;
  while (wild_cursor_current(&scan) && i < nargs) {
    switch (wild_cursor_current(&scan)) {
    case '?':
      *(char **)checked_storage_at(args, (size_t)nargs, sizeof(*args),
                                   (size_t)i) = alloc_lbuf("wild.?");
      memset(wild_argument_at(&context, i), 0, LBUF_SIZE);
      i++;
      break;
    case '*':
      *(char **)checked_storage_at(args, (size_t)nargs, sizeof(*args),
                                   (size_t)i) = alloc_lbuf("wild.*");
      memset(wild_argument_at(&context, i), 0, LBUF_SIZE);
      i++;
      break;
    default:
      break;
    }
    wild_cursor_advance(&scan);
  }

  /*
   * Do the match.
   */

  value = nargs ? wild1(&context, wild_cursor_suffix(&pattern, 0),
                        wild_cursor_suffix(&data, 0), 0)
                : quick_wild(wild_cursor_suffix(&pattern, 0),
                             wild_cursor_suffix(&data, 0));

  /*
   * Clean out any fake match data left by wild1.
   */

  for (i = 0; i < nargs; i++) {
    char **argument =
        checked_storage_at(args, (size_t)nargs, sizeof(*args), (size_t)i);
    if ((*argument != nullptr) && (!**argument || !value)) {
      free_lbuf(*argument);
      *argument = nullptr;
    }
  }
  return value;
}

/**
 * wild_match: do either an order comparison or a wildcard match,
 * remembering the wild data, if wildcard match is done.
 *
 * This routine will cause crashes if fed NULLs instead of strings.
 */
int wild_match(const char *tstr, const char *dstr) {
  int target;
  int data;

  switch (*tstr) {
  case '>':
    tstr = checked_string_suffix(tstr, 1);
    if (parse_int_checked(tstr, &target) && parse_int_checked(dstr, &data))
      return target < data;
    return strcmp(tstr, dstr) < 0;
  case '<':
    tstr = checked_string_suffix(tstr, 1);
    if (parse_int_checked(tstr, &target) && parse_int_checked(dstr, &data))
      return target > data;
    return strcmp(tstr, dstr) > 0;
  default:
    break;
  }

  return quick_wild(tstr, dstr);
}
