/*
 * powers.c - power manipulation routines
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/objects/db.h"
#include "mux/objects/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/world/world_context.h"

POWERENT gen_powers[] = {{"idle", POWER_IDLE, 0},
                         {"long_fingers", POWER_LONG_FINGERS, 0},
                         {"comm_all", POWER_COMM_ALL, 0},
                         {"see_hidden", POWER_SEE_HIDDEN, 0},
                         {"no_destroy", POWER_NO_DESTROY, 0},
                         {"pass_locks", POWER_PASS_LOCKS, 0},
                         {"mech", POWER_MECH, 0},
                         {"security", POWER_SECURITY, 0},
                         {"mechrep", POWER_MECHREP, 0},
                         {"map", POWER_MAP, 0},
                         {"tech", POWER_TECH, 0},
                         {"template", POWER_TEMPLATE, 0},
                         {nullptr, POWER_NONE, 0}};

/**
 * Initialize power hash tables.
 */
void init_powertab(WorldIndexes *indexes) {
  POWERENT *fp;
  char *nbuf, *np;
  const char *bp;

  hash_table_initialize(&indexes->powers, 15 * HASH_FACTOR);
  nbuf = alloc_sbuf("init_powertab");
  for (fp = gen_powers; fp->powername; fp++) {
    for (np = nbuf, bp = fp->powername; *bp; np++, bp++)
      *np = ToLower(*bp);
    *np = '\0';
    hash_table_add(nbuf, (int *)fp, &indexes->powers);
  }
  free_sbuf(nbuf);
}

/**
 * Display available powers.
 */
void display_powertab(EvaluationContext *evaluation, DbRef player) {
  char *buf, *bp;
  POWERENT *fp;

  bp = buf = alloc_lbuf("display_powertab");
  safe_str("Powers:", buf, &bp);
  for (fp = gen_powers; fp->powername; fp++) {
    if ((fp->listperm & CA_WIZARD) &&
        !is_wizard(evaluation->world->database, player))
      continue;
    if ((fp->listperm & CA_GOD) && !is_god(evaluation->world->database, player))
      continue;
    safe_chr(' ', buf, &bp);
    safe_str(fp->powername, buf, &bp);
  }
  *bp = '\0';
  notify(evaluation, player, buf);
  free_lbuf(buf);
}

POWERENT *find_power(WorldIndexes *indexes, DbRef thing, char *powername) {
  char *cp;

  (void)thing;

  /*
   * Make sure the power name is valid
   */

  for (cp = powername; *cp; cp++)
    *cp = ToLower(*cp);
  return (POWERENT *)hash_table_find(powername, &indexes->powers);
}

bool decode_power(EvaluationContext *evaluation, WorldIndexes *indexes,
                  DbRef player, char *powername, PowerId *id) {
  POWERENT *pent;

  *id = POWER_NONE;

  pent = (POWERENT *)hash_table_find(powername, &indexes->powers);
  if (!pent) {
    notify_printf(evaluation, player, "%s: Power not found.", powername);
    return false;
  }
  *id = pent->id;

  return true;
}

/*
 * Set or clear a specified power on an object.
 */
void power_set(EvaluationContext *evaluation, WorldIndexes *indexes,
               DbRef target, DbRef player, char *power, int key) {
  POWERENT *fp;
  bool negate;

  /*
   * Trim spaces, and handle the negation character
   */

  negate = false;
  while (*power && isspace(*power))
    power++;
  if (*power == '!') {
    negate = true;
    power++;
  }
  while (*power && isspace(*power))
    power++;

  /*
   * Make sure a power name was specified
   */

  if (*power == '\0') {
    if (negate)
      notify(evaluation, player, "You must specify a power to clear.");
    else
      notify(evaluation, player, "You must specify a power to set.");
    return;
  }
  fp = find_power(indexes, target, power);
  if (fp == nullptr) {
    notify(evaluation, player, "I don't understand that power.");
    return;
  }
  /*
   * Invoke the power handler, and print feedback
   */

  if (!is_wizard(evaluation->world->database, player) &&
      !is_god(evaluation->world->database, player)) {
    notify(evaluation, player, "Permission denied.");
    return;
  }

  game_object_set_power(evaluation->world->database, target, fp->id, !negate);
  if (!(key & SET_QUIET) && !is_quiet(evaluation->world->database, player))
    notify_printf(evaluation, player, "%s - %s %s",
                  game_object_name(evaluation->world->database, target),
                  fp->powername, negate ? "removed." : "granted.");
  return;
}

/**
 * Does object have power visible to player?
 */
bool has_power(WorldContext *world, DbRef player, DbRef it, char *powername) {
  POWERENT *fp;

  fp = find_power(world->indexes, it, powername);
  if (fp == nullptr)
    return false;

  if (game_object_has_power(world->database, it, fp->id)) {
    if ((fp->listperm & CA_WIZARD) && !is_wizard(world->database, player))
      return false;
    if ((fp->listperm & CA_GOD) && !is_god(world->database, player))
      return false;
    return true;
  }
  return false;
}

/**
 * Return an mbuf containing the type and powers on thing.
 */
char *power_description(GameDatabase *database, DbRef player, DbRef target) {
  char *buff, *bp;
  POWERENT *fp;

  /*
   * Allocate the return buffer
   */

  bp = buff = alloc_mbuf("power_description");

  /*
   * Store the header strings and object type
   */

  safe_mb_str("Powers:", buff, &bp);

  for (fp = gen_powers; fp->powername; fp++) {
    if (game_object_has_power(database, target, fp->id)) {
      if ((fp->listperm & CA_WIZARD) && !is_wizard(database, player))
        continue;
      if ((fp->listperm & CA_GOD) && !is_god(database, player))
        continue;
      safe_mb_chr(' ', buff, &bp);
      safe_mb_str(fp->powername, buff, &bp);
    }
  }

  /*
   * Terminate the string, and return the buffer to the caller
   */

  *bp = '\0';
  return buff;
}
