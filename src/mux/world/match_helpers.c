/* match_helpers.c - Supplemental object matching helpers and command parsing.
 */

#include <ctype.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/commands/command_context.h"
#include "mux/commands/command_parser.h"
#include "mux/lua/lua_runtime.h" // IWYU pragma: keep
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h" // IWYU pragma: keep
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/world/match.h"

static DbRef promote_dflt(DbRef old, DbRef new) {
  switch (new) {
  case NOPERM:
    return NOPERM;
  case AMBIGUOUS:
    if (old == NOPERM)
      return old;
    else
      return new;
  default:
    break;
  }

  if ((old == NOPERM) || (old == AMBIGUOUS))
    return old;

  return NOTHING;
}

DbRef match_possessed(MatchContext *match_context, DbRef player, DbRef thing,
                      char *target, DbRef dflt) {
  DbRef result, result1;
  int control;
  char *buff;
  size_t target_length = strlen(target);
  size_t target_offset = 0;

  /*
   * First, check normally
   */

  if (is_good_obj(match_context->evaluation->world->database, dflt))
    return dflt;

  /*
   * Didn't find it directly.  Recursively do a contents check
   */

  while (target_offset < target_length) {

    /*
     * Fail if no ' characters
     */

    size_t apostrophe_offset = target_offset;
    while (apostrophe_offset < target_length &&
           *(const char *)checked_storage_at_const(
               target, target_length, sizeof(char), apostrophe_offset) != '\'')
      apostrophe_offset++;
    if (apostrophe_offset == target_length)
      return dflt;

    /*
     * If string started with a ', skip past it
     */

    if (target_offset == apostrophe_offset) {
      target_offset++;
      continue;
    }
    /*
     * If next character is not an s or a space, skip past
     */

    size_t container_length = apostrophe_offset;
    target_offset = apostrophe_offset + 1;
    if (target_offset == target_length)
      return dflt;
    char character = *(const char *)checked_storage_at_const(
        target, target_length, sizeof(char), target_offset);
    if (character != 's' && character != 'S' && character != ' ')
      continue;

    /*
     * If character was not a space make sure the following * * *
     *
     * * character is a space.
     */

    if (character != ' ') {
      target_offset++;
      if (target_offset == target_length)
        return dflt;
      if (*(const char *)checked_storage_at_const(
              target, target_length, sizeof(char), target_offset) != ' ')
        continue;
    }
    /*
     * Copy the container name to a new buffer so we can * * * *
     * terminate it.
     */

    buff = alloc_lbuf("is_posess");
    memcpy(buff, target, container_length);
    *(char *)checked_storage_at(buff, LBUF_SIZE, sizeof(char),
                                container_length) = '\0';

    /*
     * Look for the container here and in our inventory.  Skip *
     * * * * past if we can't find it.
     */

    init_match(match_context, thing, buff, OBJECT_TYPE_NOTYPE);
    if (player == thing) {
      match_neighbor(match_context);
      match_possession(match_context);
    } else {
      match_possession(match_context);
    }
    result1 = match_result(match_context);

    free_lbuf(buff);
    if (!is_good_obj(match_context->evaluation->world->database, result1)) {
      dflt = promote_dflt(dflt, result1);
      continue;
    }
    /*
     * If we don't control it and it is either dark or opaque, *
     * * * * skip past.
     */

    control = is_controls(match_context->evaluation->world->database, player,
                          result1);
    if (is_dark(match_context->evaluation->world->database, result1) &&
        !control) {
      dflt = promote_dflt(dflt, NOTHING);
      continue;
    }
    /*
     * Look for the object in the container
     */

    char *remainder = checked_mutable_string_suffix(target, target_offset);
    init_match(match_context, result1, remainder, OBJECT_TYPE_NOTYPE);
    match_possession(match_context);
    result = match_result(match_context);
    result = match_possessed(match_context, player, result1, remainder, result);
    if (is_good_obj(match_context->evaluation->world->database, result))
      return result;
    dflt = promote_dflt(dflt, result);
  }
  return dflt;
}

/**
 * break up <what>,<low>,<high> syntax
 */
void parse_range(GameDatabase *database,
                 const ServerConfiguration *configuration, char **name,
                 DbRef *low_bound, DbRef *high_bound) {
  char *buff1, *buff2;

  buff1 = *name;
  if (buff1 && *buff1)
    *name = parse_to(configuration, &buff1, ',', COMMAND_PARSE_STRIP_TRAILING);
  if (buff1 && *buff1) {
    buff2 = parse_to(configuration, &buff1, ',', COMMAND_PARSE_STRIP_TRAILING);
    if (buff1 && *buff1) {
      size_t offset = 0;
      size_t length = strlen(buff1);
      while (offset < length &&
             (isspace)(*(const unsigned char *)checked_storage_at_const(
                 buff1, length, sizeof(char), offset)))
        offset++;
      buff1 = checked_mutable_string_suffix(buff1, offset);
      if (*buff1 == NUMBER_TOKEN)
        buff1 = checked_mutable_string_suffix(buff1, 1);
      *high_bound = atoi(buff1);
      if (*high_bound >= database->top)
        *high_bound = database->top - 1;
    } else {
      *high_bound = database->top - 1;
    }
    size_t offset = 0;
    size_t length = strlen(buff2);
    while (offset < length &&
           (isspace)(*(const unsigned char *)checked_storage_at_const(
               buff2, length, sizeof(char), offset)))
      offset++;
    buff2 = checked_mutable_string_suffix(buff2, offset);
    if (*buff2 == NUMBER_TOKEN)
      buff2 = checked_mutable_string_suffix(buff2, 1);
    *low_bound = atoi(buff2);
    if (*low_bound < 0)
      *low_bound = 0;
  } else {
    *low_bound = 0;
    *high_bound = database->top - 1;
  }
}

int parse_thing_slash(MatchContext *match_context, DbRef player, char *thing,
                      char **after, DbRef *it) {
  size_t length = strlen(thing);
  size_t offset = 0;

  /*
   * get name up to /
   */
  while (offset < length && *(const char *)checked_storage_at_const(
                                thing, length, sizeof(char), offset) != '/')
    offset++;

  /*
   * If no / in string, return failure
   */

  if (offset == length) {
    *after = nullptr;
    *it = NOTHING;
    return 0;
  }
  *(char *)checked_storage_at(thing, length + 1, sizeof(char), offset) = '\0';
  *after = checked_storage_at(thing, length + 1, sizeof(char), offset + 1);

  /*
   * Look for the object
   */

  init_match(match_context, player, thing, OBJECT_TYPE_NOTYPE);
  match_everything(match_context, 0);
  *it = match_result(match_context);

  /*
   * Return status of search
   */

  return (is_good_obj(match_context->evaluation->world->database, *it));
}
