/* player_account.c -- Typed player-account state tests. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mux/objects/db.h"
#include "mux/objects/player_account.h"

bool is_good_obj(GameDatabase *database, DbRef object);

bool is_good_obj(GameDatabase *database, DbRef object) {
  return object >= 0 && object < database->top &&
         game_object_type(database, object) != OBJECT_TYPE_GARBAGE;
}

static int check_history(GameDatabase *database) {
  PlayerAccountRef account = {.database = database, .player = 0};

  for (int index = 0; index < 6; index++) {
    if (!player_account_login_record(
            &(PlayerLoginRecordChange){.account = account,
                                       .outcome = PLAYER_LOGIN_SUCCESS,
                                       .occurred_at = 1000 + index,
                                       .host = "good.example"}))
      return -1;
  }
  for (int index = 0; index < 5; index++) {
    if (!player_account_login_record(
            &(PlayerLoginRecordChange){.account = account,
                                       .outcome = PLAYER_LOGIN_FAILURE,
                                       .occurred_at = 2000 + index,
                                       .host = "bad.example"}))
      return -1;
  }
  PlayerLoginHistoryResult record =
      player_account_login_history(&(PlayerLoginHistoryRequest){
          .account = account, .outcome = PLAYER_LOGIN_SUCCESS});
  if (player_account_successful_login_count(database, 0) != 6 ||
      player_account_failed_login_count(database, 0) != 5 ||
      player_account_unreported_failed_login_count(database, 0) != 5 ||
      player_account_login_history_count((PlayerLoginHistoryRequest){
          .account = account, .outcome = PLAYER_LOGIN_SUCCESS}) !=
          PLAYER_SUCCESS_HISTORY_LIMIT ||
      player_account_login_history_count((PlayerLoginHistoryRequest){
          .account = account, .outcome = PLAYER_LOGIN_FAILURE}) !=
          PLAYER_FAILURE_HISTORY_LIMIT ||
      !record.found || record.record.occurred_at != 1005 ||
      strcmp(record.record.host, "good.example") != 0 ||
      !player_account_login_record(
          &(PlayerLoginRecordChange){.account = account,
                                     .outcome = PLAYER_LOGIN_SUCCESS,
                                     .occurred_at = 3000,
                                     .host = "latest.example"}) ||
      player_account_unreported_failed_login_count(database, 0) != 0)
    return -1;
  return 0;
}

static int check_utc_format(void) {
  char first[32];
  char second[32];
  const time_t timestamp = 0;

  if (setenv("TZ", "America/Los_Angeles", 1) < 0)
    return -1;
  tzset();
  if (!player_account_format_timestamp_utc(timestamp, first, sizeof(first)))
    return -1;
  if (setenv("TZ", "Asia/Tokyo", 1) < 0)
    return -1;
  tzset();
  if (!player_account_format_timestamp_utc(timestamp, second, sizeof(second)))
    return -1;
  return strcmp(first, "1970-01-01T00:00:00Z") == 0 &&
                 strcmp(first, second) == 0
             ? 0
             : -1;
}

static int check_invalid_player_rejections(GameDatabase *database,
                                           DbRef player) {
  const PlayerAccountRef account = {.database = database, .player = player};
  const DbRef recipient = 42;

  return !player_account_last_login_set(
             &(PlayerLastLoginChange){.account = account, .occurred_at = 1}) &&
                 !player_account_alias_set(database, player, "invalid") &&
                 !player_account_last_site_set(database, player, "invalid") &&
                 !player_account_login_counts_set(
                     &(PlayerLoginCountsChange){.account = account,
                                                .successful = 1,
                                                .failed = 1,
                                                .unreported_failed = 1}) &&
                 !player_account_login_record(
                     &(PlayerLoginRecordChange){.account = account,
                                                .outcome = PLAYER_LOGIN_SUCCESS,
                                                .occurred_at = 1,
                                                .host = "invalid"}) &&
                 !player_account_login_history_set(&(PlayerLoginHistoryChange){
                     .target = {.account = account,
                                .outcome = PLAYER_LOGIN_SUCCESS},
                     .occurred_at = 1,
                     .host = "invalid"}) &&
                 !player_account_last_page_set(database, player, &recipient,
                                               1) &&
                 !player_account_last_login(account).found &&
                 *player_account_last_site(database, player) == '\0' &&
                 player_account_successful_login_count(database, player) == 0 &&
                 player_account_login_history_count((PlayerLoginHistoryRequest){
                     .account = account, .outcome = PLAYER_LOGIN_SUCCESS}) ==
                     0 &&
                 player_account_last_page_count(database, player) == 0
             ? 0
             : -1;
}

int main(void) {
  GameObject objects[3] = {0};
  GameDatabase database = {.object_storage = objects, .top = 2, .size = 2};
  DbRef recipients[] = {42, 7, 999};
  char alias[] = "WizardAlias";

  game_object_set_type(&database, 0, OBJECT_TYPE_PLAYER);
  game_object_set_type(&database, 1, OBJECT_TYPE_THING);
  if (!player_account_password_hash_set(&database, 0, "hash") ||
      strcmp(player_account_password_hash(&database, 0), "hash") != 0 ||
      !player_account_alias_set(&database, 0, alias) ||
      strcmp(player_account_alias(&database, 0), "WizardAlias") != 0 ||
      player_account_password_hash_set(&database, 1, "invalid") ||
      check_invalid_player_rejections(&database, 1) < 0 ||
      check_invalid_player_rejections(&database, 2) < 0 ||
      player_account_login_record(&(PlayerLoginRecordChange){
          .account = {.database = &database, .player = 0},
          .outcome = (PlayerLoginOutcome)99,
          .host = "invalid"}) ||
      !player_account_last_login_set(&(PlayerLastLoginChange){
          .account = {.database = &database, .player = 0},
          .occurred_at = 123456789}) ||
      !player_account_last_site_set(&database, 0, "user@example") ||
      strcmp(player_account_last_site(&database, 0), "user@example") != 0 ||
      check_history(&database) < 0 ||
      !player_account_last_page_set(&database, 0, recipients, 3) ||
      player_account_last_page_count(&database, 0) != 3 ||
      player_account_last_page_recipient(
          &(PlayerPageRecipientRequest){
              .account = {.database = &database, .player = 0}, .position = 1})
              .recipient != 7 ||
      check_utc_format() < 0) {
    player_account_clear(&database, 0);
    return 1;
  }
  alias[0] = 'X';
  if (strcmp(player_account_alias(&database, 0), "WizardAlias") != 0 ||
      !player_account_alias_set(&database, 0, nullptr) ||
      *player_account_alias(&database, 0) ||
      !player_account_alias_set(&database, 0, "FinalAlias"))
    return 1;
  PlayerLastLoginResult last_login = player_account_last_login(
      (PlayerAccountRef){.database = &database, .player = 0});
  if (!last_login.found || last_login.occurred_at != 123456789)
    return 1;
  player_account_clear(&database, 0);
  if (game_database_object(&database, 0)->account ||
      *player_account_password_hash(&database, 0) ||
      *player_account_alias(&database, 0))
    return 1;
  return 0;
}
