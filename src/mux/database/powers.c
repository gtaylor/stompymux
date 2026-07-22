/*
 * powers.c - power manipulation routines
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/world/world_context.h"

/**
 * set or clear indicated bit, no security checking
 */
static int ph_any(EvaluationContext *evaluation, DbRef target, DbRef player,
                  Power power, int fpowers, int reset) {
  if (fpowers & POWER_EXT) {
    if (reset)
      game_object_set_powers2(
          evaluation->world->database, target,
          game_object_powers2(evaluation->world->database, target) & ~power);
    else
      game_object_set_powers2(
          evaluation->world->database, target,
          game_object_powers2(evaluation->world->database, target) | power);
  } else {
    if (reset)
      game_object_set_powers(
          evaluation->world->database, target,
          game_object_powers(evaluation->world->database, target) & ~power);
    else
      game_object_set_powers(
          evaluation->world->database, target,
          game_object_powers(evaluation->world->database, target) | power);
  }
  return 1;
}

/**
 * Only WIZARDS (or GOD) may set or clear the bit
 */
static int ph_wiz(EvaluationContext *evaluation, DbRef target, DbRef player,
                  Power power, int fpowers, int reset) {
  if (!is_wizard(evaluation->world->database, player) &&
      !is_god(evaluation->world->database, player))
    return 0;
  return (ph_any(evaluation, target, player, power, fpowers, reset));
}

POWERENT gen_powers[] = {{"idle", POW_IDLE, 0, 0, ph_wiz},
                         {"long_fingers", POW_LONGFINGERS, 0, 0, ph_wiz},
                         {"comm_all", POW_COMM_ALL, 0, 0, ph_wiz},
                         {"see_hidden", POW_SEE_HIDDEN, 0, 0, ph_wiz},
                         {"no_destroy", POW_NO_DESTROY, 0, 0, ph_wiz},
                         {"pass_locks", POW_PASS_LOCKS, 0, 0, ph_wiz},
                         /* BattletechMUX Powers */
                         {"mech", POW_MECH, POWER_EXT, 0, ph_wiz},
                         {"security", POW_SECURITY, POWER_EXT, 0, ph_wiz},
                         {"mechrep", POW_MECHREP, POWER_EXT, 0, ph_wiz},
                         {"map", POW_MAP, POWER_EXT, 0, ph_wiz},
                         {"tech", POW_TECH, POWER_EXT, 0, ph_wiz},
                         {"template", POW_TEMPLATE, POWER_EXT, 0, ph_wiz},
                         {nullptr, 0, 0, 0, 0}};

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

  /*
   * Make sure the power name is valid
   */

  for (cp = powername; *cp; cp++)
    *cp = ToLower(*cp);
  return (POWERENT *)hash_table_find(powername, &indexes->powers);
}

int decode_power(EvaluationContext *evaluation, WorldIndexes *indexes,
                 DbRef player, char *powername, POWERSET *pset) {
  POWERENT *pent;

  pset->word1 = 0;
  pset->word2 = 0;

  pent = (POWERENT *)hash_table_find(powername, &indexes->powers);
  if (!pent) {
    notify_printf(evaluation, player, "%s: Power not found.", powername);
    return 0;
  }
  if (pent->powerpower & POWER_EXT)
    pset->word2 = pent->powervalue;
  else
    pset->word1 = pent->powervalue;

  return 1;
}

/*
 * Set or clear a specified power on an object.
 */
void power_set(EvaluationContext *evaluation, WorldIndexes *indexes,
               DbRef target, DbRef player, char *power, int key) {
  POWERENT *fp;
  int negate, result;

  /*
   * Trim spaces, and handle the negation character
   */

  negate = 0;
  while (*power && isspace(*power))
    power++;
  if (*power == '!') {
    negate = 1;
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

  result = fp->handler(evaluation, target, player, fp->powervalue,
                       fp->powerpower, negate);
  if (!result)
    notify(evaluation, player, "Permission denied.");
  else if (!(key & SET_QUIET) && !is_quiet(evaluation->world->database, player))
    notify_printf(evaluation, player, "%s - %s %s",
                  game_object_name(evaluation->world->database, target),
                  fp->powername, negate ? "removed." : "granted.");
  return;
}

/**
 * Does object have power visible to player?
 */
int has_power(WorldContext *world, DbRef player, DbRef it, char *powername) {
  POWERENT *fp;
  Power fv;

  fp = find_power(world->indexes, it, powername);
  if (fp == nullptr)
    return 0;

  if (fp->powerpower & POWER_EXT)
    fv = game_object_powers2(world->database, it);
  else
    fv = game_object_powers(world->database, it);

  if (fv & fp->powervalue) {
    if ((fp->listperm & CA_WIZARD) && !is_wizard(world->database, player))
      return 0;
    if ((fp->listperm & CA_GOD) && !is_god(world->database, player))
      return 0;
    return 1;
  }
  return 0;
}

/**
 * Return an mbuf containing the type and powers on thing.
 */
char *power_description(GameDatabase *database, DbRef player, DbRef target) {
  char *buff, *bp;
  POWERENT *fp;
  Power fv;

  /*
   * Allocate the return buffer
   */

  bp = buff = alloc_mbuf("power_description");

  /*
   * Store the header strings and object type
   */

  safe_mb_str("Powers:", buff, &bp);

  for (fp = gen_powers; fp->powername; fp++) {
    if (fp->powerpower & POWER_EXT)
      fv = game_object_powers2(database, target);
    else
      fv = game_object_powers(database, target);
    if (fv & fp->powervalue) {
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
