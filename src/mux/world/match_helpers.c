/* match_helpers.c - Supplemental object matching helpers and command parsing.
 */

#include "mux/world/match.h"
#include "mux/world/world_context.h"

#include "mux/server/platform.h"

#include "mux/database/attrs.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"

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
                      char *target, DbRef dflt, int check_enter) {
  DbRef result, result1;
  int control;
  char *buff, *start, *place, *s1, *d1, *temp;

  /*
   * First, check normally
   */

  if (is_good_obj(match_context->evaluation->world->database, dflt))
    return dflt;

  /*
   * Didn't find it directly.  Recursively do a contents check
   */

  start = target;
  while (*target) {

    /*
     * Fail if no ' characters
     */

    place = target;
    target = (char *)index(place, '\'');
    if ((target == nullptr) || !*target)
      return dflt;

    /*
     * If string started with a ', skip past it
     */

    if (place == target) {
      target++;
      continue;
    }
    /*
     * If next character is not an s or a space, skip past
     */

    temp = target++;
    if (!*target)
      return dflt;
    if ((*target != 's') && (*target != 'S') && (*target != ' '))
      continue;

    /*
     * If character was not a space make sure the following * * *
     *
     * * character is a space.
     */

    if (*target != ' ') {
      target++;
      if (!*target)
        return dflt;
      if (*target != ' ')
        continue;
    }
    /*
     * Copy the container name to a new buffer so we can * * * *
     * terminate it.
     */

    buff = alloc_lbuf("is_posess");
    for (s1 = start, d1 = buff; *s1 && (s1 < temp); *d1++ = (*s1++))
      ;
    *d1 = '\0';

    /*
     * Look for the container here and in our inventory.  Skip *
     * * * * past if we can't find it.
     */

    init_match(match_context, thing, buff, NOTYPE);
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

    control = is_controls(match_context->evaluation, player, result1);
    if ((is_dark(match_context->evaluation->world->database, result1) ||
         is_opaque(match_context->evaluation->world->database, result1)) &&
        !control) {
      dflt = promote_dflt(dflt, NOTHING);
      continue;
    }
    /*
     * Validate object has the ENTER bit set, if requested
     */

    if ((check_enter) &&
        !is_enter_ok(match_context->evaluation->world->database, result1) &&
        !control) {
      dflt = promote_dflt(dflt, NOPERM);
      continue;
    }
    /*
     * Look for the object in the container
     */

    init_match(match_context, result1, target, NOTYPE);
    match_possession(match_context);
    result = match_result(match_context);
    result = match_possessed(match_context, player, result1, target, result,
                             check_enter);
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
      while (*buff1 && isspace(*buff1))
        buff1++;
      if (*buff1 == NUMBER_TOKEN)
        buff1++;
      *high_bound = atoi(buff1);
      if (*high_bound >= database->top)
        *high_bound = database->top - 1;
    } else {
      *high_bound = database->top - 1;
    }
    while (*buff2 && isspace(*buff2))
      buff2++;
    if (*buff2 == NUMBER_TOKEN)
      buff2++;
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
  char *str;

  /*
   * get name up to /
   */
  for (str = thing; *str && (*str != '/'); str++)
    ;

  /*
   * If no / in string, return failure
   */

  if (!*str) {
    *after = nullptr;
    *it = NOTHING;
    return 0;
  }
  *str++ = '\0';
  *after = str;

  /*
   * Look for the object
   */

  init_match(match_context, player, thing, NOTYPE);
  match_everything(match_context, 0);
  *it = match_result(match_context);

  /*
   * Return status of search
   */

  return (is_good_obj(match_context->evaluation->world->database, *it));
}
