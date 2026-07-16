/* match_helpers.c - Supplemental object matching helpers and command parsing.
 */

#include "mux/world/match.h"

#include "mux/server/platform.h"

#include "mux/commands/functions.h"
#include "mux/database/attrs.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
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

DbRef match_possessed(DbRef player, DbRef thing, char *target, DbRef dflt,
                      int check_enter) {
  DbRef result, result1;
  int control;
  char *buff, *start, *place, *s1, *d1, *temp;

  /*
   * First, check normally
   */

  if (is_good_obj(dflt))
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

    init_match(thing, buff, NOTYPE);
    if (player == thing) {
      match_neighbor();
      match_possession();
    } else {
      match_possession();
    }
    result1 = match_result();

    free_lbuf(buff);
    if (!is_good_obj(result1)) {
      dflt = promote_dflt(dflt, result1);
      continue;
    }
    /*
     * If we don't control it and it is either dark or opaque, *
     * * * * skip past.
     */

    control = is_controls(player, result1);
    if ((is_dark(result1) || is_opaque(result1)) && !control) {
      dflt = promote_dflt(dflt, NOTHING);
      continue;
    }
    /*
     * Validate object has the ENTER bit set, if requested
     */

    if ((check_enter) && !is_enter_ok(result1) && !control) {
      dflt = promote_dflt(dflt, NOPERM);
      continue;
    }
    /*
     * Look for the object in the container
     */

    init_match(result1, target, NOTYPE);
    match_possession();
    result = match_result();
    result = match_possessed(player, result1, target, result, check_enter);
    if (is_good_obj(result))
      return result;
    dflt = promote_dflt(dflt, result);
  }
  return dflt;
}

/**
 * break up <what>,<low>,<high> syntax
 */
void parse_range(char **name, DbRef *low_bound, DbRef *high_bound) {
  char *buff1, *buff2;

  buff1 = *name;
  if (buff1 && *buff1)
    *name = parse_to(&buff1, ',', EV_STRIP_TS);
  if (buff1 && *buff1) {
    buff2 = parse_to(&buff1, ',', EV_STRIP_TS);
    if (buff1 && *buff1) {
      while (*buff1 && isspace(*buff1))
        buff1++;
      if (*buff1 == NUMBER_TOKEN)
        buff1++;
      *high_bound = atoi(buff1);
      if (*high_bound >= mudstate.db_top)
        *high_bound = mudstate.db_top - 1;
    } else {
      *high_bound = mudstate.db_top - 1;
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
    *high_bound = mudstate.db_top - 1;
  }
}

int parse_thing_slash(DbRef player, char *thing, char **after, DbRef *it) {
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

  init_match(player, thing, NOTYPE);
  match_everything(MAT_EXIT_PARENTS);
  *it = match_result();

  /*
   * Return status of search
   */

  return (is_good_obj(*it));
}

extern NameTable lock_sw[];

int get_obj_and_lock(DbRef player, char *what, DbRef *it, Attribute **attr,
                     char *errmsg, char **bufc) {
  char *str, *tbuf;
  int anum;

  tbuf = alloc_lbuf("get_obj_and_lock");
  StringCopy(tbuf, what);
  if (parse_thing_slash(player, tbuf, &str, it)) {

    /*
     * <obj>/<lock> syntax, use the named lock
     */

    anum = name_table_search(player, lock_sw, str);
    if (anum < 0) {
      free_lbuf(tbuf);
      safe_str("#-1 LOCK NOT FOUND", errmsg, bufc);
      return 0;
    }
  } else {

    /*
     * Not <obj>/<lock>, do a normal get of the default lock
     */

    *it = match_thing(player, what);
    if (!is_good_obj(*it)) {
      free_lbuf(tbuf);
      safe_str("#-1 NOT FOUND", errmsg, bufc);
      return 0;
    }
    anum = A_LOCK;
  }

  /*
   * Get the attribute definition, fail if not found
   */

  free_lbuf(tbuf);
  *attr = attribute_by_number(anum);
  if (!(*attr)) {
    safe_str("#-1 LOCK NOT FOUND", errmsg, bufc);
    return 0;
  }
  return 1;
}
