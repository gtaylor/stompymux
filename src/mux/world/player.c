
/*
 * player.c
 */

#include "mux/server/server_registries.h"
#include <crypto_pwhash.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <utils.h>

#include "mux/commands/command_handlers.h"
#include "mux/communication/comsys.h"
#include "mux/network/site_access.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/player_account.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/owned_text.h"
#include "mux/support/password.h"
#include "mux/support/stringutil.h"
#include "mux/support/validation.h"
#include "mux/support/wild.h"
#include "mux/world/object.h"
#include "mux/world/player.h"

/**
 * Record successful or failed login attempt.
 * If successful, report the number of failures since the last successful
 * login.
 */
void record_login(EvaluationContext *evaluation, DbRef player, bool successful,
                  time_t occurred_at, const char *host, const char *username) {
  char message_buffer[LBUF_SIZE];
  GameDatabase *database = evaluation->world->database;

  if (successful) {
    int64_t unreported =
        player_account_unreported_failed_login_count(database, player);
    if (unreported > 0) {
      PlayerLoginHistoryResult latest_failure =
          player_account_login_history(&(PlayerLoginHistoryRequest){
              .account = {.database = database, .player = player},
              .outcome = PLAYER_LOGIN_FAILURE});
      char timestamp[32];

      notify_checked(evaluation, player, player, "", MSG_ME_ALL | MSG_F_DOWN);
      notify_printf(
          evaluation, player,
          "**** %lld failed connect%s since your last successful connect. "
          "****",
          (long long)unreported, unreported == 1 ? "" : "s");
      if (latest_failure.found &&
          player_account_format_timestamp_utc(latest_failure.record.occurred_at,
                                              timestamp, sizeof(timestamp)))
        notify_printf(evaluation, player,
                      "Most recent attempt was from %s on %s.",
                      latest_failure.record.host, timestamp);
      notify_checked(evaluation, player, player, "", MSG_ME_ALL | MSG_F_DOWN);
    }
    player_account_login_record(&(PlayerLoginRecordChange){
        .account = {.database = database, .player = player},
        .outcome = PLAYER_LOGIN_SUCCESS,
        .occurred_at = occurred_at,
        .host = host});
    if (username && *username) {
      (void)snprintf(message_buffer, sizeof(message_buffer), "%s@%s", username,
                     host);
      player_account_last_site_set(database, player, message_buffer);
    } else {
      player_account_last_site_set(database, player, host);
    }
  } else {
    player_account_login_record(&(PlayerLoginRecordChange){
        .account = {.database = database, .player = player},
        .outcome = PLAYER_LOGIN_FAILURE,
        .occurred_at = occurred_at,
        .host = host});
  }
}

/**
 * Test a password to see if it is correct.
 */
bool check_pass(WorldContext *world, DbRef player, const char *password) {
  if (strlen(password) >
      (size_t)world->configuration->player_password_length_limit)
    return false;
  const char *target = player_account_password_hash(world->database, player);
  return (*target && password_verify(password, target)) != 0;
}

/**
 * Try to connect to an existing player.
 */
DbRef connect_player(const PlayerConnectionRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  WorldContext *world = request->world;
  DbRef player;
  time_t tt;
  tt = time(nullptr);
  if (tt == (time_t)-1)
    tt = 0;

  player = lookup_player(world, NOTHING, request->name, 0);
  if (player == NOTHING)
    return NOTHING;
  if (!check_pass(world, player, request->password)) {
    record_login(evaluation, player, false, tt, request->host,
                 request->username);
    return NOTHING;
  }
  tt = time(nullptr);
  if (tt == (time_t)-1)
    tt = 0;
  player_account_last_login_set(&(PlayerLastLoginChange){
      .account = {.database = world->database, .player = player},
      .occurred_at = tt});
  return player;
}

/**
 * Create a new player.
 */
DbRef create_player(const PlayerCreationRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  const char *name = request->name;
  const char *password = request->password;
  WorldContext *world = evaluation->world;
  DbRef player;
  char hashed_password[crypto_pwhash_STRBYTES];
  OwnedText pbuf;

  /*
   * Make sure the password is OK.  Name is checked in create_obj
   */

  if (!ok_new_player_name(world->configuration, name))
    return NOTHING;

  pbuf = trim_spaces(password);
  if (!ok_password(world->configuration, pbuf.text)) {
    owned_text_release(&pbuf);
    return NOTHING;
  }
  if (!password_hash(world->configuration, pbuf.text, hashed_password)) {
    owned_text_release(&pbuf);
    return NOTHING;
  }
  /*
   * If so, go create him
   */

  player = create_obj(evaluation, NOTHING, OBJECT_TYPE_PLAYER, name);
  if (player == NOTHING) {
    sodium_memzero(hashed_password, sizeof(hashed_password));
    owned_text_release(&pbuf);
    return NOTHING;
  }
  /*
   * initialize everything
   */
  if (*world->configuration->public_channel)
    comsys_add_alias(evaluation, player, "pub",
                     world->configuration->public_channel);

  object_password_set(world->database, player, hashed_password);
  game_object_set_link(world->database, player,
                       world->configuration->start_home != NOTHING
                           ? world->configuration->start_home
                           : world->configuration->start_room);
  sodium_memzero(hashed_password, sizeof(hashed_password));
  owned_text_release(&pbuf);
  return player;
}

/**
 * Display login history data.
 */
static void display_login_record(EvaluationContext *evaluation, DbRef player,
                                 const PlayerLoginRecordView *record) {
  char timestamp[32];
  if (record->host && *record->host &&
      player_account_format_timestamp_utc(record->occurred_at, timestamp,
                                          sizeof(timestamp)))
    notify_printf(evaluation, player, "     From: %s   On: %s", record->host,
                  timestamp);
}

void do_last(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *who = invocation->first;
  WorldContext *world = invocation->context->world;
  DbRef target;

  if (!who || !*who || !(string_compare(world->configuration, who, "me"))) {
    target = player;
  } else {
    target = lookup_player(world, player, who, 1);
  }

  if (target == NOTHING) {
    notify_checked(evaluation, player, player, "I couldn't find that player.",
                   MSG_ME_ALL | MSG_F_DOWN);
  } else {
    notify_printf(&invocation->context->evaluation, player,
                  "Total successful connects: %lld",
                  (long long)player_account_successful_login_count(
                      world->database, target));
    for (size_t index = 0;
         index < player_account_login_history_count((PlayerLoginHistoryRequest){
                     .account = {.database = world->database, .player = target},
                     .outcome = PLAYER_LOGIN_SUCCESS});
         index++) {
      PlayerLoginHistoryResult result =
          player_account_login_history(&(PlayerLoginHistoryRequest){
              .account = {.database = world->database, .player = target},
              .outcome = PLAYER_LOGIN_SUCCESS,
              .position = index});
      if (result.found)
        display_login_record(&invocation->context->evaluation, player,
                             &result.record);
    }
    notify_printf(
        &invocation->context->evaluation, player, "Total failed connects: %lld",
        (long long)player_account_failed_login_count(world->database, target));
    for (size_t index = 0;
         index < player_account_login_history_count((PlayerLoginHistoryRequest){
                     .account = {.database = world->database, .player = target},
                     .outcome = PLAYER_LOGIN_FAILURE});
         index++) {
      PlayerLoginHistoryResult result =
          player_account_login_history(&(PlayerLoginHistoryRequest){
              .account = {.database = world->database, .player = target},
              .outcome = PLAYER_LOGIN_FAILURE,
              .position = index});
      if (result.found)
        display_login_record(&invocation->context->evaluation, player,
                             &result.record);
    }
  }
}

/*
 * add_player_name, delete_player_name, lookup_player:
 * Manage playername->dbref mapping
 */
int add_player_name(WorldContext *world, DbRef player, const char *name) {
  int stat;
  DbRef *p;
  char *temp;
  char *tp;

  /*
   * Convert to all lowercase
   */

  tp = temp = alloc_lbuf("add_player_name");
  safe_str(name, temp, &tp);
  *tp = '\0';
  for (size_t index = 0; index < strlen(temp); index++) {
    char *character =
        checked_storage_at(temp, strlen(temp), sizeof(char), index);
    *character = ascii_to_lower(*character);
  }

  p = (long *)hash_table_find(temp, &world->indexes->players);
  if (p) {

    /*
     * Entry found in the hashtable.  If a player, succeed if the
     * * * numbers match (already correctly in the hash table),
     * fail * * if they don't.  Fail if the name is a disallowed
     * name * * (value AMBIGUOUS).
     */

    if (*p == AMBIGUOUS) {
      free_buf(temp);
      return 0;
    }
    if (is_good_obj(world->database, *p) &&
        (typeof_obj(world->database, *p) == OBJECT_TYPE_PLAYER)) {
      free_buf(temp);
      if (*p == player) {
        return 1;
      }
      return 0;
    }
    /*
     * It's an alias (or an incorrect entry).  Clobber it
     */
    free(p);
    p = checked_storage_allocate(sizeof(DbRef));

    *p = player;
    stat = hash_table_replace(temp, p, &world->indexes->players);
    free_buf(temp);
  } else {
    p = checked_storage_allocate(sizeof(DbRef));

    *p = player;
    stat = hash_table_add(temp, p, &world->indexes->players);
    free_buf(temp);
    stat = (stat < 0) ? 0 : 1;
  }
  return stat;
}

bool delete_player_name(WorldContext *world, DbRef player, const char *name) {
  DbRef *p;
  char *temp;
  char *tp;

  tp = temp = alloc_lbuf("delete_player_name");
  safe_str(name, temp, &tp);
  *tp = '\0';
  for (size_t index = 0; index < strlen(temp); index++) {
    char *character =
        checked_storage_at(temp, strlen(temp), sizeof(char), index);
    *character = ascii_to_lower(*character);
  }

  p = (long *)hash_table_find(temp, &world->indexes->players);
  if (!p || (*p == NOTHING) || ((player != NOTHING) && (*p != player))) {
    free_buf(temp);
    return false;
  }
  free(p);
  hash_table_delete(temp, &world->indexes->players);
  free_buf(temp);
  return true;
}

DbRef lookup_player(WorldContext *world, DbRef doer, const char *name,
                    int check_who) {
  DbRef *p;
  DbRef thing;
  char *temp;
  char *tp;

  if (!string_compare(world->configuration, name, "me"))
    return doer;

  if (*name == NUMBER_TOKEN) {
    const char *numeric_name = checked_string_suffix(name, 1);
    if (!is_number(numeric_name))
      return NOTHING;
    thing = clamped_atol(numeric_name);
    if (!is_good_obj(world->database, thing))
      return NOTHING;
    if (!((typeof_obj(world->database, thing) == OBJECT_TYPE_PLAYER) ||
          is_god(world->database, doer)))
      thing = NOTHING;
    return thing;
  }
  tp = temp = alloc_lbuf("lookup_player");
  safe_str(name, temp, &tp);
  *tp = '\0';
  for (size_t index = 0; index < strlen(temp); index++) {
    char *character =
        checked_storage_at(temp, strlen(temp), sizeof(char), index);
    *character = ascii_to_lower(*character);
  }
  p = (long *)hash_table_find(temp, &world->indexes->players);
  free_buf(temp);
  if (!p) {
    if (check_who) {
      thing =
          find_connected_name(world->database, world->descriptors, doer, name);
      if (is_dark(world->database, thing))
        thing = NOTHING;
    } else {
      thing = NOTHING;
    }
  } else if (!is_good_obj(world->database, *p)) {
    thing = NOTHING;
  } else {
    thing = *p;
  }

  return thing;
}

void load_player_names(WorldContext *world) {
  DbRef i;
  long aflags;
  char *alias;

  DO_WHOLE_DB(world->database, i) {
    if (typeof_obj(world->database, i) == OBJECT_TYPE_PLAYER) {
      add_player_name(world, i, game_object_pure_name(world->database, i));
    }
  }
  alias = alloc_lbuf("load_player_names");
  DO_WHOLE_DB(world->database, i) {
    if (typeof_obj(world->database, i) == OBJECT_TYPE_PLAYER) {
      alias = attribute_get_string(world->database, i, A_ALIAS, alias,
                                   LBUF_SIZE, &aflags);
      if (*alias)
        add_player_name(world, i, alias);
    }
  }
  free_buf(alias);
}

/**
 * badname_add, badname_check, badname_list: Add/look for/display bad names.
 */
void badname_add(WorldContext *world, char *bad_name) {
  BADNAME *bp;

  /*
   * Make a new node and link it in at the top
   */

  bp = (BADNAME *)checked_storage_allocate(sizeof(BADNAME));
  bp->name = checked_storage_allocate(strlen(bad_name) + 1);
  bp->next = world->access_control->bad_names;
  world->access_control->bad_names = bp;
  (void)string_copy_bounded(bp->name, strlen(bad_name) + 1, bad_name);
}

void badname_remove(WorldContext *world, char *bad_name) {
  BADNAME *bp;
  BADNAME *backp;

  /*
   * Look for an exact match on the bad name and remove if found
   */

  backp = nullptr;
  for (bp = world->access_control->bad_names; bp; backp = bp, bp = bp->next) {
    if (!string_compare(world->configuration, bad_name, bp->name)) {
      if (backp)
        backp->next = bp->next;
      else
        world->access_control->bad_names = bp->next;
      free(bp->name);
      free(bp);
      return;
    }
  }
}

bool badname_check(WorldContext *world, const char *bad_name) {
  BADNAME *bp;

  /*
   * Walk the badname list, doing wildcard matching.  If we get a hit *
   *
   * *  * *  * * then return false.  If no matches in the list, return
   * true.
   */

  for (bp = world->access_control->bad_names; bp; bp = bp->next) {
    if (quick_wild(bp->name, bad_name))
      return false;
  }
  return true;
}

void badname_list(EvaluationContext *evaluation, WorldContext *world,
                  DbRef player, const char *prefix) {
  BADNAME *bp;
  char *buff;
  char *bufp;

  /*
   * Construct an lbuf with all the names separated by spaces
   */

  buff = bufp = alloc_lbuf("badname_list");
  safe_str(prefix, buff, &bufp);
  for (bp = world->access_control->bad_names; bp; bp = bp->next) {
    safe_chr(' ', buff, &bufp);
    safe_str(bp->name, buff, &bufp);
  }
  *bufp = '\0';

  /*
   * Now display it
   */

  notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
  free_buf(buff);
}
