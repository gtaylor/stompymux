/*
 * powers.c - power manipulation routines
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"

/**
 * set or clear indicated bit, no security checking
 */
static int ph_any(DbRef target, DbRef player, Power power, int fpowers,
                  int reset) {
  if (fpowers & POWER_EXT) {
    if (reset)
      s_powers2(target, obj_powers2(target) & ~power);
    else
      s_powers2(target, obj_powers2(target) | power);
  } else {
    if (reset)
      s_powers(target, obj_powers(target) & ~power);
    else
      s_powers(target, obj_powers(target) | power);
  }
  return 1;
}

/**
 * Only WIZARDS (or GOD) may set or clear the bit
 */
static int ph_wiz(DbRef target, DbRef player, Power power, int fpowers,
                  int reset) {
  if (!is_wizard(player) & !is_god(player))
    return 0;
  return (ph_any(target, player, power, fpowers, reset));
}

POWERENT gen_powers[] = {
    {(char *)"find_unfindable", POW_FIND_UNFIND, 0, 0, ph_wiz},
    {(char *)"idle", POW_IDLE, 0, 0, ph_wiz},
    {(char *)"long_fingers", POW_LONGFINGERS, 0, 0, ph_wiz},
    {(char *)"comm_all", POW_COMM_ALL, 0, 0, ph_wiz},
    {(char *)"see_hidden", POW_SEE_HIDDEN, 0, 0, ph_wiz},
    {(char *)"no_destroy", POW_NO_DESTROY, 0, 0, ph_wiz},
    {(char *)"pass_locks", POW_PASS_LOCKS, 0, 0, ph_wiz},
    /* BattletechMUX Powers */
    {(char *)"mech", POW_MECH, POWER_EXT, 0, ph_wiz},
    {(char *)"security", POW_SECURITY, POWER_EXT, 0, ph_wiz},
    {(char *)"mechrep", POW_MECHREP, POWER_EXT, 0, ph_wiz},
    {(char *)"map", POW_MAP, POWER_EXT, 0, ph_wiz},
    {(char *)"tech", POW_TECH, POWER_EXT, 0, ph_wiz},
    {(char *)"template", POW_TEMPLATE, POWER_EXT, 0, ph_wiz},
    {nullptr, 0, 0, 0, 0}};

/**
 * Initialize power hash tables.
 */
void init_powertab(void) {
  POWERENT *fp;
  char *nbuf, *np, *bp;

  hash_table_initialize(&mudstate.powers_htab, 15 * HASH_FACTOR);
  nbuf = alloc_sbuf("init_powertab");
  for (fp = gen_powers; fp->powername; fp++) {
    for (np = nbuf, bp = (char *)fp->powername; *bp; np++, bp++)
      *np = ToLower(*bp);
    *np = '\0';
    hash_table_add(nbuf, (int *)fp, &mudstate.powers_htab);
  }
  free_sbuf(nbuf);
}

/**
 * Display available powers.
 */
void display_powertab(DbRef player) {
  char *buf, *bp;
  POWERENT *fp;

  bp = buf = alloc_lbuf("display_powertab");
  safe_str((char *)"Powers:", buf, &bp);
  for (fp = gen_powers; fp->powername; fp++) {
    if ((fp->listperm & CA_WIZARD) && !is_wizard(player))
      continue;
    if ((fp->listperm & CA_GOD) && !is_god(player))
      continue;
    safe_chr(' ', buf, &bp);
    safe_str((char *)fp->powername, buf, &bp);
  }
  *bp = '\0';
  notify(player, buf);
  free_lbuf(buf);
}

POWERENT *find_power(DbRef thing, char *powername) {
  char *cp;

  /*
   * Make sure the power name is valid
   */

  for (cp = powername; *cp; cp++)
    *cp = ToLower(*cp);
  return (POWERENT *)hash_table_find(powername, &mudstate.powers_htab);
}

int decode_power(DbRef player, char *powername, POWERSET *pset) {
  POWERENT *pent;

  pset->word1 = 0;
  pset->word2 = 0;

  pent = (POWERENT *)hash_table_find(powername, &mudstate.powers_htab);
  if (!pent) {
    notify_printf(player, "%s: Power not found.", powername);
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
void power_set(DbRef target, DbRef player, char *power, int key) {
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
      notify(player, "You must specify a power to clear.");
    else
      notify(player, "You must specify a power to set.");
    return;
  }
  fp = find_power(target, power);
  if (fp == nullptr) {
    notify(player, "I don't understand that power.");
    return;
  }
  /*
   * Invoke the power handler, and print feedback
   */

  result = fp->handler(target, player, fp->powervalue, fp->powerpower, negate);
  if (!result)
    notify(player, "Permission denied.");
  else if (!(key & SET_QUIET) && !is_quiet(player))
    notify_printf(player, "%s - %s %s", Name(target), fp->powername,
                  negate ? "removed." : "granted.");
  return;
}

/**
 * Does object have power visible to player?
 */
int has_power(DbRef player, DbRef it, char *powername) {
  POWERENT *fp;
  Power fv;

  fp = find_power(it, powername);
  if (fp == nullptr)
    return 0;

  if (fp->powerpower & POWER_EXT)
    fv = obj_powers2(it);
  else
    fv = obj_powers(it);

  if (fv & fp->powervalue) {
    if ((fp->listperm & CA_WIZARD) && !is_wizard(player))
      return 0;
    if ((fp->listperm & CA_GOD) && !is_god(player))
      return 0;
    return 1;
  }
  return 0;
}

/**
 * Return an mbuf containing the type and powers on thing.
 */
char *power_description(DbRef player, DbRef target) {
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

  safe_mb_str((char *)"Powers:", buff, &bp);

  for (fp = gen_powers; fp->powername; fp++) {
    if (fp->powerpower & POWER_EXT)
      fv = obj_powers2(target);
    else
      fv = obj_powers(target);
    if (fv & fp->powervalue) {
      if ((fp->listperm & CA_WIZARD) && !is_wizard(player))
        continue;
      if ((fp->listperm & CA_GOD) && !is_god(player))
        continue;
      safe_mb_chr(' ', buff, &bp);
      safe_mb_str((char *)fp->powername, buff, &bp);
    }
  }

  /*
   * Terminate the string, and return the buffer to the caller
   */

  *bp = '\0';
  return buff;
}
