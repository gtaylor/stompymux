/*
 * powers.c - power manipulation routines
 */

#include <ctype.h>
#include <stddef.h>
#include <string.h>

#include "btech/context.h" // IWYU pragma: keep
#include "mux/commands/command.h"
#include "mux/commands/command_keys.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/persistence/gamedb.h" // IWYU pragma: keep
#include "mux/server/configuration_context.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/owned_text.h"
#include "mux/support/stringutil.h"

static const POWERENT POWER_ENTRIES[] = {
    {"idle", POWER_IDLE, 0},
    {nullptr, POWER_NONE, 0},
};
size_t object_power_entry_count(void) {
  return (sizeof(POWER_ENTRIES) / sizeof(*POWER_ENTRIES)) - 1;
}

const POWERENT *object_power_entry_at(size_t index) {
  return checked_storage_at_const(POWER_ENTRIES, object_power_entry_count(),
                                  sizeof(*POWER_ENTRIES), index);
}

/**
 * Initialize power hash tables.
 */
void init_powertab(WorldIndexes *indexes) {
  const POWERENT *fp;
  char nbuf[SBUF_SIZE];

  hash_table_initialize(&indexes->powers, 15 * HASH_FACTOR);
  for (size_t index = 0; index < object_power_entry_count(); index++) {
    fp = object_power_entry_at(index);
    size_t length = strlen(fp->powername);
    for (size_t character_index = 0; character_index < length;
         character_index++) {
      const char *input = checked_storage_at_const(
          fp->powername, length, sizeof(char), character_index);
      char *output =
          checked_storage_at(nbuf, sizeof(nbuf), sizeof(char), character_index);
      *output = ascii_to_lower(*input);
    }
    *(char *)checked_storage_at(nbuf, sizeof(nbuf), sizeof(char), length) =
        '\0';
    hash_table_add_const(nbuf, fp, &indexes->powers);
  }
}

/**
 * Display available powers.
 */
void display_powertab(EvaluationContext *evaluation, DbRef player) {
  char *buf;
  char *bp;
  const POWERENT *fp;

  bp = buf = alloc_lbuf("display_powertab");
  safe_str("Powers:", buf, &bp);
  for (size_t index = 0; index < object_power_entry_count(); index++) {
    fp = object_power_entry_at(index);
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
  free_buf(buf);
}

static const POWERENT *power_find_normalized(WorldIndexes *indexes,
                                             const char *powername) {
  char normalized[SBUF_SIZE];

  if (powername == nullptr)
    return nullptr;
  size_t length = strlen(powername);
  if (length >= sizeof(normalized))
    return nullptr;
  for (size_t index = 0; index < length; index++) {
    const char *input =
        checked_storage_at_const(powername, length, sizeof(char), index);
    char *output =
        checked_storage_at(normalized, sizeof(normalized), sizeof(char), index);
    *output = ascii_to_lower(*input);
  }
  *(char *)checked_storage_at(normalized, sizeof(normalized), sizeof(char),
                              length) = '\0';
  return hash_table_find_const(normalized, &indexes->powers);
}

const POWERENT *find_power(WorldIndexes *indexes, DbRef thing [[maybe_unused]],
                           const char *powername) {
  return power_find_normalized(indexes, powername);
}

bool decode_power(EvaluationContext *evaluation, WorldIndexes *indexes,
                  DbRef player, const char *powername, PowerId *id) {
  const POWERENT *pent;

  *id = POWER_NONE;

  pent = power_find_normalized(indexes, powername);
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
  const POWERENT *fp;
  bool negate;

  /*
   * Trim spaces, and handle the negation character
   */

  negate = false;
  size_t length = strlen(power);
  size_t offset = 0;
  while (offset < length &&
         (isspace)(*(const unsigned char *)checked_storage_at_const(
             power, length, sizeof(char), offset)))
    offset++;
  power = checked_mutable_string_suffix(power, offset);
  if (*power == '!') {
    negate = true;
    power = checked_mutable_string_suffix(power, 1);
  }
  length = strlen(power);
  offset = 0;
  while (offset < length &&
         (isspace)(*(const unsigned char *)checked_storage_at_const(
             power, length, sizeof(char), offset)))
    offset++;
  power = checked_mutable_string_suffix(power, offset);

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

  game_object_set_power(
      &(ObjectPowerChange){.target = {.database = evaluation->world->database,
                                      .object = target,
                                      .power = fp->id},
                           .value = (!negate) != 0});
  if (!(key & SET_QUIET))
    notify_printf(evaluation, player, "%s - %s %s",
                  game_object_name(evaluation->world->database, target),
                  fp->powername, negate ? "removed." : "granted.");
}

/**
 * Return an mbuf containing the type and powers on thing.
 */
OwnedText power_description(const PowerDescriptionRequest *request) {
  GameDatabase *database = request->database;
  DbRef player = request->viewer;
  DbRef target = request->target;
  char *buff;
  char *bp;
  const POWERENT *fp;

  /*
   * Allocate the return buffer
   */

  bp = buff = alloc_mbuf("power_description");

  /*
   * Store the header strings and object type
   */

  safe_mb_str("Powers:", buff, &bp);

  for (size_t index = 0; index < object_power_entry_count(); index++) {
    fp = object_power_entry_at(index);
    if (game_object_has_power(&(ObjectPowerRequest){
            .database = database, .object = target, .power = fp->id})) {
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
  return owned_text_take(buff);
}
