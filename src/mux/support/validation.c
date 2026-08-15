/* validation.c - Input validation for names, attributes, and passwords. */

#include "mux/support/validation.h"
#include "mux/server/server_config.h" // IWYU pragma: keep

#include <ctype.h>
#include <string.h>
#include <strings.h>

#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mux/support/utf8.h"

static char validation_character(const char *text, size_t length,
                                 size_t index) {
  return *(const char *)checked_storage_at_const(text, length + 1, sizeof(char),
                                                 index);
}

static bool validation_is_space(char character) {
  return (isspace)((unsigned char)character) != 0;
}

static bool validation_is_digit(char character) {
  return (isdigit)((unsigned char)character) != 0;
}

static bool ascii_is_alpha(unsigned char byte) {
  return (byte >= 'A' && byte <= 'Z') || (byte >= 'a' && byte <= 'z');
}

static bool ascii_is_alnum(unsigned char byte) {
  return ascii_is_alpha(byte) || (byte >= '0' && byte <= '9');
}

bool is_integer(char *str) {
  if (str == nullptr)
    return 0;
  const size_t LENGTH = strlen(str);
  size_t index = 0;
  while (index < LENGTH &&
         validation_is_space(validation_character(str, LENGTH, index)))
    index++;
  if (index < LENGTH && validation_character(str, LENGTH, index) == '-')
    index++;
  if (index >= LENGTH ||
      !validation_is_digit(validation_character(str, LENGTH, index)))
    return 0;
  while (index < LENGTH &&
         validation_is_digit(validation_character(str, LENGTH, index)))
    index++;
  while (index < LENGTH &&
         validation_is_space(validation_character(str, LENGTH, index)))
    index++;
  return index == LENGTH;
}

/**
 * Checks for the presence of a number
 */
bool is_number(const char *str) {
  if (str == nullptr)
    return 0;
  const size_t LENGTH = strlen(str);
  size_t index = 0;
  bool got_digit = false;
  while (index < LENGTH &&
         validation_is_space(validation_character(str, LENGTH, index)))
    index++;
  if (index < LENGTH && validation_character(str, LENGTH, index) == '-')
    index++;
  while (index < LENGTH &&
         validation_is_digit(validation_character(str, LENGTH, index))) {
    got_digit = true;
    index++;
  }
  if (index < LENGTH && validation_character(str, LENGTH, index) == '.')
    index++;
  while (index < LENGTH &&
         validation_is_digit(validation_character(str, LENGTH, index))) {
    got_digit = true;
    index++;
  }
  while (index < LENGTH &&
         validation_is_space(validation_character(str, LENGTH, index)))
    index++;
  return got_digit && index == LENGTH;
}

bool ok_name(const ServerConfiguration *configuration, const char *name) {
  if (name == nullptr || *name == '\0')
    return 0;

  const size_t LENGTH = strlen(name);

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
  if (validation_character(name, LENGTH, LENGTH - 1) == ' ')
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

static bool ok_player_name_with_limit(const ServerConfiguration *configuration,
                                      const char *name, size_t maximum_length,
                                      bool allow_spaces) {
  const char *good_chars;

  /*
   * No leading spaces
   */

  if (name == nullptr || !utf8_is_printable_ascii(name, strlen(name)) ||
      *name == ' ')
    return 0;

  /*
   * Not too long and a good name for a thing
   */

  if (!ok_name(configuration, name) || strlen(name) > maximum_length)
    return 0;

  if (allow_spaces)
    good_chars = " `$_-.,'";
  else
    good_chars = "`$_-.,'";

  /*
   * Make sure name only contains legal characters
   */

  const size_t LENGTH = strlen(name);
  for (size_t index = 0; index < LENGTH; index++) {
    const char CHARACTER = validation_character(name, LENGTH, index);
    if (ascii_is_alnum((unsigned char)CHARACTER))
      continue;
    if (!strchr(good_chars, CHARACTER))
      return 0;
  }
  return 1;
}

bool ok_stored_player_name(const ServerConfiguration *configuration,
                           const char *name) {
  return ok_player_name_with_limit(configuration, name,
                                   PLAYER_NAME_STORAGE_LIMIT, true);
}

bool ok_player_name(const ServerConfiguration *configuration,
                    const char *name) {
  /* An unset limit uses the storage ceiling so zero-initialized test and
   * utility configurations remain permissive. */
  const size_t MAXIMUM_LENGTH =
      configuration->player_name_length_limit > 0
          ? (size_t)configuration->player_name_length_limit
          : PLAYER_NAME_STORAGE_LIMIT;

  return ok_player_name_with_limit(configuration, name, MAXIMUM_LENGTH,
                                   configuration->name_spaces);
}

bool ok_new_player_name(const ServerConfiguration *configuration,
                        const char *name) {
  return name != nullptr && strlen(name) >= 2 &&
         ascii_is_alpha(
             (unsigned char)validation_character(name, strlen(name), 0)) &&
         ok_player_name(configuration, name);
}

bool ok_password(const ServerConfiguration *configuration,
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
    if (!utf8_decode(checked_storage_region_const(password, length, offset,
                                                  length - offset),
                     length - offset, &decoded) ||
        decoded.codepoint == ' ') {
      return 0;
    }
    offset += decoded.length;
  }

  /*
   * Needed.  Change it if you like, but be sure yours is the same.
   */
  if (length == 13 && validation_character(password, length, 0) == 'X' &&
      validation_character(password, length, 1) == 'X')
    return 0;

  return 1;
}
