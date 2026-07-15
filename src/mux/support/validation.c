/* validation.c - Input validation for names, attributes, and passwords. */

#include "mux/support/validation.h"

#include "mux/server/platform.h"
#include "mux/server/configuration.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/support/stringutil.h"

int is_integer(char *str) {
  while (*str && isspace(*str))
    str++;           /*
                      * Leading spaces
                      */
  if (*str == '-') { /*
                      * Leading minus
                      */
    str++;
    if (!*str)
      return 0; /*
                 * but not if just a minus
                 */
  }
  if (!isdigit(*str)) /*
                       * Need at least 1 integer
                       */
    return 0;
  while (*str && isdigit(*str))
    str++; /*
            * The number (int)
            */
  while (*str && isspace(*str))
    str++; /*
            * Trailing spaces
            */
  return (*str ? 0 : 1);
}

/**
 * Checks for the presence of a number
 */
int is_number(char *str) {
  int got_one;

  while (*str && isspace(*str))
    str++;           /*
                      * Leading spaces
                      */
  if (*str == '-') { /*
                      * Leading minus
                      */
    str++;
    if (!*str)
      return 0; /*
                 * but not if just a minus
                 */
  }
  got_one = 0;
  if (isdigit(*str))
    got_one = 1; /*
                  * Need at least one digit
                  */
  while (*str && isdigit(*str))
    str++; /*
            * The number (int)
            */
  if (*str == '.')
    str++; /*
            * decimal point
            */
  if (isdigit(*str))
    got_one = 1; /*
                  * Need at least one digit
                  */
  while (*str && isdigit(*str))
    str++; /*
            * The number (fract)
            */
  while (*str && isspace(*str))
    str++; /*
            * Trailing spaces
            */
  return ((*str || !got_one) ? 0 : 1);
}

int ok_name(const char *name) {
  const char *cp;
  char new[LBUF_SIZE];

  /* Disallow pure ANSI names */
  strncpy(new, name, LBUF_SIZE - 1);
  if (strlen(strip_ansi_r(new, name, strlen(name))) == 0)
    return 0;

  /* Disallow leading spaces */

  if (isspace(*name))
    return 0;

  /*
   * Only printable characters
   */

  for (cp = name; cp && *cp; cp++) {
    if ((!isprint(*cp)) && (*cp != ESC_CHAR))
      return 0;
  }

  /*
   * Disallow trailing spaces
   */
  cp--;
  if (isspace(*cp))
    return 0;

  /*
   * Exclude names that start with or contain certain magic cookies
   */

  return (name && *name && *name != LOOKUP_TOKEN && *name != NUMBER_TOKEN &&
          *name != NOT_TOKEN && !index(name, ARG_DELIMITER) &&
          !index(name, AND_TOKEN) && !index(name, OR_TOKEN) &&
          string_compare(name, "me") && string_compare(name, "home") &&
          string_compare(name, "here"));
}

int ok_player_name(const char *name) {
  const char *cp, *good_chars;

  /*
   * No leading spaces
   */

  if (isspace(*name))
    return 0;

  /*
   * Not too long and a good name for a thing
   */

  if (!ok_name(name) || (strlen(name) >= PLAYER_NAME_LIMIT))
    return 0;

  if (mudconf.name_spaces)
    good_chars = " `$_-.,'";
  else
    good_chars = "`$_-.,'";

  /*
   * Make sure name only contains legal characters
   */

  for (cp = name; cp && *cp; cp++) {
    if (isalnum(*cp))
      continue;
    if ((!index(good_chars, *cp)) || (*cp == ESC_CHAR))
      return 0;
  }
  return 1;
}

int ok_attr_name(const char *attrname) {
  const char *scan;

  if (!isalpha(*attrname))
    return 0;
  for (scan = attrname; *scan; scan++) {
    if (isalnum(*scan))
      continue;
    if (!(index("'?!`/-_.@#$^&~=+<>()%", *scan)))
      return 0;
  }
  return 1;
}

int ok_password(const char *password) {
  const char *scan;

  if (*password == '\0')
    return 0;

  for (scan = password; *scan; scan++) {
    if (!(isprint(*scan) && !isspace(*scan))) {
      return 0;
    }
  }

  /*
   * Needed.  Change it if you like, but be sure yours is the same.
   */
  if ((strlen(password) == 13) && (password[0] == 'X') && (password[1] == 'X'))
    return 0;

  return 1;
}
