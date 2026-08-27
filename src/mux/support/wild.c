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

#include <stdlib.h>
#include <string.h>

#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mux/support/wild.h"

static inline char fixcase(char a) { return ascii_to_lower(a); }

static inline bool is_equal(char a, char b) {
  return ((a == b) || (fixcase(a) == fixcase(b))) != 0;
}

static inline bool is_notequal(char a, char b) {
  return ((a != b) && (fixcase(a) != fixcase(b))) != 0;
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
      (const void *)context->arguments, (size_t)context->argument_count,
      sizeof(*context->arguments), (size_t)argument);
}

typedef struct WildCharacterCapture {
  const WildcardContext *context;
  int argument;
  char character;
} WildCharacterCapture;

static void wild_capture_character(const WildCharacterCapture *capture_call) {
  char *capture =
      wild_argument_at(capture_call->context, capture_call->argument);
  char character = capture_call->character;
  *(char *)checked_storage_at(capture, LBUF_SIZE, sizeof(char), 0) = character;
  *(char *)checked_storage_at(capture, LBUF_SIZE, sizeof(char), 1) = '\0';
}

/**
 * Do a wildcard match, without remembering the wild data.
 * This routine will cause crashes if fed NULLs instead of strings.
 */
bool quick_wild(const char *tstr, const char *dstr) {
  WildCursor pattern = wild_cursor(tstr);
  WildCursor data = wild_cursor(dstr);
  WildCursor star_pattern = {};
  WildCursor star_data = {};
  bool has_star = false;

  while (wild_cursor_current(&data)) {
    char pattern_character = wild_cursor_current(&pattern);
    if (pattern_character == '*') {
      wild_cursor_advance(&pattern);
      star_pattern = pattern;
      star_data = data;
      has_star = true;
      continue;
    }

    WildCursor next_pattern = pattern;
    if (pattern_character == '\\') {
      wild_cursor_advance(&next_pattern);
      pattern_character = wild_cursor_current(&next_pattern);
    }
    if (pattern_character == '?' ||
        is_equal(pattern_character, wild_cursor_current(&data))) {
      wild_cursor_advance(&next_pattern);
      pattern = next_pattern;
      wild_cursor_advance(&data);
      continue;
    }

    if (!has_star)
      return false;
    wild_cursor_advance(&star_data);
    data = star_data;
    pattern = star_pattern;
  }

  while (wild_cursor_current(&pattern) == '*')
    wild_cursor_advance(&pattern);
  if (wild_cursor_current(&pattern) == '\\')
    wild_cursor_advance(&pattern);
  return wild_cursor_current(&pattern) == '\0';
}

/**
 * wild1: INTERNAL: do a wildcard match, remembering the wild data.
 *
 * DO NOT CALL THIS FUNCTION DIRECTLY - DOING SO MAY RESULT IN
 * SERVER CRASHES AND IMPROPER ARGUMENT RETURN.
 *
 * Captures are stored in the stack-owned context supplied by wild().
 */
static constexpr size_t WILD_CAPTURE_MAX_DEPTH = 64;

// NOLINTNEXTLINE(misc-no-recursion): bounded by WILD_CAPTURE_MAX_DEPTH.
static bool wild1(WildcardContext *context, const char *tstr, const char *dstr,
                  int arg, size_t depth) {
  WildCursor pattern = wild_cursor(tstr);
  WildCursor data = wild_cursor(dstr);
  size_t data_capture_offset;
  int argpos;
  int numextra;

  if (depth > WILD_CAPTURE_MAX_DEPTH)
    return false;

  while (wild_cursor_current(&pattern) != '*') {
    switch (wild_cursor_current(&pattern)) {
    case '?':
      /*
       * Single character match.  Return false if at * end
       * * * * of data.
       */
      if (!wild_cursor_current(&data))
        return false;
      wild_capture_character(
          &(WildCharacterCapture){.context = context,
                                  .argument = arg,
                                  .character = wild_cursor_current(&data)});
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
        return false;
      if (!wild_cursor_current(&data))
        return true;
    }
    wild_cursor_advance(&pattern);
    wild_cursor_advance(&data);
  }

  /*
   * If at end of pattern, slurp the rest, and leave.
   */

  if (!*wild_cursor_suffix(&pattern, 1)) {
    char *capture = wild_argument_at(context, arg);
    (void)string_copy_bounded(capture, LBUF_SIZE, wild_cursor_suffix(&data, 0));
    return true;
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
        wild_capture_character(&(WildCharacterCapture){
            .context = context, .argument = argpos, .character = character});
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
        return false;
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

    if (wild_cursor_current(&pattern)) {
      while (is_notequal(wild_cursor_current(&data),
                         wild_cursor_current(&pattern))) {
        if (!wild_cursor_current(&data))
          return false;
        wild_cursor_advance(&data);
      }
    } else {
      while (wild_cursor_current(&data))
        wild_cursor_advance(&data);
    }

    /*
     * The first character matches, now.  Check if the rest * * *
     *
     * * does, using the fastest method, as usual.
     */
    if (!wild_cursor_current(&data) ||
        ((arg < context->argument_count)
             ? wild1(context, wild_cursor_suffix(&pattern, 1),
                     wild_cursor_suffix(&data, 1), arg, depth + 1)
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
      /* Malformed or unexpectedly large input should yield a bounded capture,
       * not turn a wildcard match into a process-wide abort. */
      const size_t CAPTURE_SIZE =
          capture_length < LBUF_SIZE ? capture_length + 1 : LBUF_SIZE;
      (void)string_copy_bounded(
          capture, CAPTURE_SIZE,
          checked_string_suffix(data.text, data_capture_offset));
      data_capture_offset = data.offset - (size_t)numextra;
      argpos++;

      /*
       * Fill in any trailing '?'s that are left.
       */

      while (numextra) {
        if (argpos >= context->argument_count)
          return true;
        char character = *(const char *)checked_storage_at_const(
            data.text, data.length, sizeof(char), data_capture_offset);
        wild_capture_character(&(WildCharacterCapture){
            .context = context, .argument = argpos, .character = character});
        data_capture_offset++;
        argpos++;
        numextra--;
      }

      /*
       * It's done!
       */

      return true;
    }
    wild_cursor_advance(&data);
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
bool wild(const char *tstr, const char *dstr, char *args[], int nargs) {
  int i;
  bool value;
  WildcardContext context = {.arguments = args, .argument_count = nargs};
  WildCursor pattern = wild_cursor(tstr);
  WildCursor data = wild_cursor(dstr);

  /*
   * Initialize the return array.
   */

  for (i = 0; i < nargs; i++)
    *(char **)checked_storage_at((void *)args, (size_t)nargs, sizeof(*args),
                                 (size_t)i) = nullptr;

  /*
   * Do fast match.
   */

  while (wild_cursor_current(&pattern) != '*' &&
         wild_cursor_current(&pattern) != '?') {
    if (wild_cursor_current(&pattern) == '\\')
      wild_cursor_advance(&pattern);
    if (is_notequal(wild_cursor_current(&data), wild_cursor_current(&pattern)))
      return false;
    if (!wild_cursor_current(&data))
      return true;
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
      *(char **)checked_storage_at((void *)args, (size_t)nargs, sizeof(*args),
                                   (size_t)i) = alloc_lbuf("wild.?");
      memset(wild_argument_at(&context, i), 0, LBUF_SIZE);
      i++;
      break;
    case '*':
      *(char **)checked_storage_at((void *)args, (size_t)nargs, sizeof(*args),
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

  value = ((nargs ? wild1(&context, wild_cursor_suffix(&pattern, 0),
                          wild_cursor_suffix(&data, 0), 0, 0)
                  : quick_wild(wild_cursor_suffix(&pattern, 0),
                               wild_cursor_suffix(&data, 0))) != 0);

  /*
   * Clean out any fake match data left by wild1.
   */

  for (i = 0; i < nargs; i++) {
    char **argument = (char **)checked_storage_at((void *)args, (size_t)nargs,
                                                  sizeof(*args), (size_t)i);
    if ((*argument != nullptr) && (!**argument || !value)) {
      free_buf(*argument);
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
bool wild_match(const char *tstr, const char *dstr) {
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
