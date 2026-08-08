/*
 * powers.c - power manipulation routines
 */

#include <ctype.h>

#include "btech/context.h" // IWYU pragma: keep
#include "mux/commands/command.h"
#include "mux/commands/command_keys.h"
#include "mux/objects/db.h"
#include "mux/objects/powers.h"
#include "mux/persistence/gamedb.h" // IWYU pragma: keep
#include "mux/server/configuration_context.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/stringutil.h"

static POWERENT gen_powers[2] = {
    {"idle", POWER_IDLE, 0},
    {nullptr, POWER_NONE, 0},
};
constexpr size_t GEN_POWER_COUNT = 1;

/**
 * Initialize power hash tables.
 */
void init_powertab(WorldIndexes *indexes) {
  POWERENT *fp;
  char nbuf[SBUF_SIZE];
  char *np;
  const char *bp;

  hash_table_initialize(&indexes->powers, 15 * HASH_FACTOR);
  for (size_t index = 0; index < GEN_POWER_COUNT; index++) {
    fp = &gen_powers[index];
    for (np = nbuf, bp = fp->powername; *bp; np++, bp++)
      *np = ascii_to_lower(*bp);
    *np = '\0';
    hash_table_add(nbuf, (int *)fp, &indexes->powers);
  }
}

/**
 * Display available powers.
 */
void display_powertab(EvaluationContext *evaluation, DbRef player) {
  char *buf, *bp;
  POWERENT *fp;

  bp = buf = alloc_lbuf("display_powertab");
  safe_str("Powers:", buf, &bp);
  for (size_t index = 0; index < GEN_POWER_COUNT; index++) {
    fp = &gen_powers[index];
    if ((fp->listperm & CA_WIZARD) &&
        !is_wizard(evaluation->world->database, player))
      continue;
    if ((fp->listperm & CA_GOD) && !is_god(evaluation->world->database, player))
      continue;
    safe_chr(' ', buf, &bp);
    safe_str(fp->powername, buf, &bp);
  }
  *bp = '\0';
  notify_checked(evaluation, player, player, buf, MSG_ME_ALL | MSG_F_DOWN);
  free_lbuf(buf);
}

POWERENT *find_power(WorldIndexes *indexes, DbRef thing, char *powername) {
  char *cp;

  (void)thing;

  /*
   * Make sure the power name is valid
   */

  for (cp = powername; *cp; cp++)
    *cp = ascii_to_lower(*cp);
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
  while (*power && isspace((unsigned char)*power))
    power++;
  if (*power == '!') {
    negate = true;
    power++;
  }
  while (*power && isspace((unsigned char)*power))
    power++;

  /*
   * Make sure a power name was specified
   */

  if (*power == '\0') {
    if (negate)
      notify_checked(evaluation, player, player,
                     "You must specify a power to clear.",
                     MSG_ME_ALL | MSG_F_DOWN);
    else
      notify_checked(evaluation, player, player,
                     "You must specify a power to set.",
                     MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  fp = find_power(indexes, target, power);
  if (fp == nullptr) {
    notify_checked(evaluation, player, player, "I don't understand that power.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  /*
   * Invoke the power handler, and print feedback
   */

  if (!is_wizard(evaluation->world->database, player) &&
      !is_god(evaluation->world->database, player)) {
    notify_checked(evaluation, player, player, "Permission denied.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }

  game_object_set_power(evaluation->world->database, target, fp->id, !negate);
  if (!(key & SET_QUIET))
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

  for (size_t index = 0; index < GEN_POWER_COUNT; index++) {
    fp = &gen_powers[index];
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
