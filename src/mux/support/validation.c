/* validation.c - Input validation for names, attributes, and passwords. */

#include "mux/support/validation.h"

#include "mux/server/configuration.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "mux/support/utf8.h"

static bool ascii_is_alpha(unsigned char byte) {
  return (byte >= 'A' && byte <= 'Z') || (byte >= 'a' && byte <= 'z');
}

static bool ascii_is_alnum(unsigned char byte) {
  return ascii_is_alpha(byte) || (byte >= '0' && byte <= '9');
}

int is_integer(char *str) {
  while (*str && isspace((unsigned char)*str))
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
  if (!isdigit((unsigned char)*str)) /*
                                      * Need at least 1 integer
                                      */
    return 0;
  while (*str && isdigit((unsigned char)*str))
    str++; /*
            * The number (int)
            */
  while (*str && isspace((unsigned char)*str))
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

  while (*str && isspace((unsigned char)*str))
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
  if (isdigit((unsigned char)*str))
    got_one = 1; /*
                  * Need at least one digit
                  */
  while (*str && isdigit((unsigned char)*str))
    str++; /*
            * The number (int)
            */
  if (*str == '.')
    str++; /*
            * decimal point
            */
  if (isdigit((unsigned char)*str))
    got_one = 1; /*
                  * Need at least one digit
                  */
  while (*str && isdigit((unsigned char)*str))
    str++; /*
            * The number (fract)
            */
  while (*str && isspace((unsigned char)*str))
    str++; /*
            * Trailing spaces
            */
  return ((*str || !got_one) ? 0 : 1);
}

int ok_name(const ServerConfiguration *configuration, const char *name) {
  const char *cp;

  if (name == nullptr || *name == '\0')
    return 0;

  /* Disallow leading spaces */

  if (*name == ' ')
    return 0;

  /*
   * Only printable characters
   */

  if (!utf8_validate_printable(name, strlen(name)))
    return 0;

  /*
   * Disallow trailing spaces
   */
  cp = name + strlen(name) - 1;
  if (*cp == ' ')
    return 0;

  /*
   * Exclude names that start with or contain certain magic cookies
   */

  return (*name != LOOKUP_TOKEN && *name != NUMBER_TOKEN &&
          *name != NOT_TOKEN && !index(name, ARG_DELIMITER) &&
          !index(name, AND_TOKEN) && !index(name, OR_TOKEN) &&
          string_compare(configuration, name, "me") &&
          string_compare(configuration, name, "home") &&
          string_compare(configuration, name, "here"));
}

int ok_player_name(const ServerConfiguration *configuration, const char *name) {
  const char *cp, *good_chars;

  /*
   * No leading spaces
   */

  if (name == nullptr || !utf8_is_printable_ascii(name, strlen(name)) ||
      *name == ' ')
    return 0;

  /*
   * Not too long and a good name for a thing
   */

  if (!ok_name(configuration, name) || (strlen(name) >= PLAYER_NAME_LIMIT))
    return 0;

  if (configuration->name_spaces)
    good_chars = " `$_-.,'";
  else
    good_chars = "`$_-.,'";

  /*
   * Make sure name only contains legal characters
   */

  for (cp = name; cp && *cp; cp++) {
    if (ascii_is_alnum((unsigned char)*cp))
      continue;
    if (!index(good_chars, *cp))
      return 0;
  }
  return 1;
}

int ok_new_player_name(const ServerConfiguration *configuration,
                       const char *name) {
  return name != nullptr && strlen(name) >= 2 &&
         ascii_is_alpha((unsigned char)*name) &&
         ok_player_name(configuration, name);
}

int ok_password(const ServerConfiguration *configuration,
                const char *password) {
  Utf8DecodeResult decoded;
  size_t length;
  size_t offset = 0;

  length = strlen(password);
  if (*password == '\0' ||
      length > (size_t)configuration->player_password_length_limit ||
      !utf8_validate_printable(password, length))
    return 0;

  while (offset < length) {
    if (!utf8_decode(password + offset, length - offset, &decoded) ||
        decoded.codepoint == ' ') {
      return 0;
    }
    offset += decoded.length;
  }

  /*
   * Needed.  Change it if you like, but be sure yours is the same.
   */
  if ((strlen(password) == 13) && (password[0] == 'X') && (password[1] == 'X'))
    return 0;

  return 1;
}
