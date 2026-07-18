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

#include "mux/server/platform.h"

#include "mux/database/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/wild.h"

static inline char fixcase(char a) { return ToLower(a); }

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

/**
 * Do a wildcard match, without remembering the wild data.
 * This routine will cause crashes if fed NULLs instead of strings.
 */
int quick_wild(const char *tstr, const char *dstr) {
  while (*tstr != '*') {
    switch (*tstr) {
    case '?':
      /*
       * Single character match.  Return false if at * end
       * * * * of data.
       */
      if (!*dstr)
        return 0;
      break;
    case '\\':
      /*
       * Escape character.  Move up, and force literal * *
       * * * match of next character.
       */
      tstr++;
      /*
       * FALL THROUGH
       */
      [[fallthrough]];
    default:
      /*
       * Literal character.  Check for a match. * If * * *
       * matching end of data, return true.
       */
      if (is_notequal(*dstr, *tstr))
        return 0;
      if (!*dstr)
        return 1;
    }
    tstr++;
    dstr++;
  }

  /*
   * Skip over '*'.
   */

  tstr++;

  /*
   * Return true on trailing '*'.
   */

  if (!*tstr)
    return 1;

  /*
   * Skip over wildcards.
   */

  while ((*tstr == '?') || (*tstr == '*')) {
    if (*tstr == '?') {
      if (!*dstr)
        return 0;
      dstr++;
    }
    tstr++;
  }

  /*
   * Skip over a backslash in the pattern string if it is there.
   */

  if (*tstr == '\\')
    tstr++;

  /*
   * Return true on trailing '*'.
   */

  if (!*tstr)
    return 1;

  /*
   * Scan for possible matches.
   */

  while (*dstr) {
    if (is_equal(*dstr, *tstr) && quick_wild(tstr + 1, dstr + 1))
      return 1;
    dstr++;
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
  const char *datapos;
  int argpos, numextra;

  while (*tstr != '*') {
    switch (*tstr) {
    case '?':
      /*
       * Single character match.  Return false if at * end
       * * * * of data.
       */
      if (!*dstr)
        return 0;
      context->arguments[arg][0] = *dstr;
      context->arguments[arg][1] = '\0';
      arg++;

      /*
       * Jump to the fast routine if we can.
       */

      if (arg >= context->argument_count)
        return quick_wild(tstr + 1, dstr + 1);
      break;
    case '\\':
      /*
       * Escape character.  Move up, and force literal * *
       * * * match of next character.
       */
      tstr++;
      /*
       * FALL THROUGH
       */
      [[fallthrough]];
    default:
      /*
       * Literal character.  Check for a match. * If * * *
       * matching end of data, return true.
       */
      if (is_notequal(*dstr, *tstr))
        return 0;
      if (!*dstr)
        return 1;
    }
    tstr++;
    dstr++;
  }

  /*
   * If at end of pattern, slurp the rest, and leave.
   */

  if (!tstr[1]) {
    StringCopyTrunc(context->arguments[arg], dstr, LBUF_SIZE - 1);
    context->arguments[arg][LBUF_SIZE - 1] = '\0';
    return 1;
  }
  /*
   * Remember current position for filling in the '*' return.
   */

  datapos = dstr;
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
      context->arguments[argpos][0] = '\0';
      argpos++;

      /*
       * Jump to the fast routine if we can.
       */

      if (argpos >= context->argument_count)
        return quick_wild(tstr, dstr);

      /*
       * Fill in any intervening '?'s
       */

      while (argpos < arg) {
        context->arguments[argpos][0] = *datapos;
        context->arguments[argpos][1] = '\0';
        datapos++;
        argpos++;

        /*
         * Jump to the fast routine if we can.
         */

        if (argpos >= context->argument_count)
          return quick_wild(tstr, dstr);
      }
    }
    /*
     * Skip over the '*' for now...
     */

    tstr++;
    arg++;

    /*
     * Skip over '?'s for now...
     */

    numextra = 0;
    while (*tstr == '?') {
      if (!*dstr)
        return 0;
      tstr++;
      dstr++;
      arg++;
      numextra++;
    }
  } while (*tstr == '*');

  /*
   * Skip over a backslash in the pattern string if it is there.
   */

  if (*tstr == '\\')
    tstr++;

  /*
   * Check for possible matches.  This loop terminates either at * end
   * * * * of data (resulting in failure), or at a successful match.
   */
  while (1) {

    /*
     * Scan forward until first character matches.
     */

    if (*tstr)
      while (is_notequal(*dstr, *tstr)) {
        if (!*dstr)
          return 0;
        dstr++;
      }
    else
      while (*dstr)
        dstr++;

    /*
     * The first character matches, now.  Check if the rest * * *
     *
     * * does, using the fastest method, as usual.
     */
    if (!*dstr || ((arg < context->argument_count)
                       ? wild1(context, tstr + 1, dstr + 1, arg)
                       : quick_wild(tstr + 1, dstr + 1))) {

      /*
       * Found a match!  Fill in all remaining arguments. *
       *
       * *  * *  * * First do the '*'...
       */
      StringCopyTrunc(context->arguments[argpos], datapos,
                      (size_t)((dstr - datapos) - numextra));
      context->arguments[argpos][(dstr - datapos) - numextra] = '\0';
      datapos = dstr - numextra;
      argpos++;

      /*
       * Fill in any trailing '?'s that are left.
       */

      while (numextra) {
        if (argpos >= context->argument_count)
          return 1;
        context->arguments[argpos][0] = *datapos;
        context->arguments[argpos][1] = '\0';
        datapos++;
        argpos++;
        numextra--;
      }

      /*
       * It's done!
       */

      return 1;
    } else {
      dstr++;
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
  const char *scan;
  WildcardContext context = {.arguments = args, .argument_count = nargs};

  /*
   * Initialize the return array.
   */

  for (i = 0; i < nargs; i++)
    args[i] = nullptr;

  /*
   * Do fast match.
   */

  while ((*tstr != '*') && (*tstr != '?')) {
    if (*tstr == '\\')
      tstr++;
    if (is_notequal(*dstr, *tstr))
      return 0;
    if (!*dstr)
      return 1;
    tstr++;
    dstr++;
  }

  /*
   * Allocate space for the return args.
   */

  i = 0;
  scan = tstr;
  while (*scan && (i < nargs)) {
    switch (*scan) {
    case '?':
      args[i] = alloc_lbuf("wild.?");
      memset(args[i], 0, LBUF_SIZE);
      i++;
      break;
    case '*':
      args[i] = alloc_lbuf("wild.*");
      memset(args[i], 0, LBUF_SIZE);
      i++;
    default:
      break;
    }
    scan++;
  }

  /*
   * Do the match.
   */

  value = nargs ? wild1(&context, tstr, dstr, 0) : quick_wild(tstr, dstr);

  /*
   * Clean out any fake match data left by wild1.
   */

  for (i = 0; i < nargs; i++)
    if ((args[i] != nullptr) && (!*args[i] || !value)) {
      free_lbuf(args[i]);
      args[i] = nullptr;
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
  switch (*tstr) {
  case '>':
    tstr++;
    if (isdigit(*tstr) || (*tstr == '-'))
      return (atoi(tstr) < atoi(dstr));
    else
      return (strcmp(tstr, dstr) < 0);
  case '<':
    tstr++;
    if (isdigit(*tstr) || (*tstr == '-'))
      return (atoi(tstr) > atoi(dstr));
    else
      return (strcmp(tstr, dstr) > 0);
  default:
    break;
  }

  return quick_wild(tstr, dstr);
}
