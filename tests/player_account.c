/* player_account.c -- Typed player-account state tests. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mux/objects/db.h"
#include "mux/objects/player_account.h"

bool is_good_obj(GameDatabase *database, DbRef object) {
  return object >= 0 && object < database->top &&
         game_object_type(database, object) != OBJECT_TYPE_GARBAGE;
}

static int check_history(GameDatabase *database) {
  PlayerLoginRecordView record;

  for (int index = 0; index < 6; index++)
    if (!player_account_login_record(database, 0, PLAYER_LOGIN_SUCCESS,
                                     1000 + index, "good.example"))
      return -1;
  for (int index = 0; index < 5; index++)
    if (!player_account_login_record(database, 0, PLAYER_LOGIN_FAILURE,
                                     2000 + index, "bad.example"))
      return -1;
  if (player_account_successful_login_count(database, 0) != 6 ||
      player_account_failed_login_count(database, 0) != 5 ||
      player_account_unreported_failed_login_count(database, 0) != 5 ||
      player_account_login_history_count(database, 0, PLAYER_LOGIN_SUCCESS) !=
          PLAYER_SUCCESS_HISTORY_LIMIT ||
      player_account_login_history_count(database, 0, PLAYER_LOGIN_FAILURE) !=
          PLAYER_FAILURE_HISTORY_LIMIT ||
      !player_account_login_history(database, 0, PLAYER_LOGIN_SUCCESS, 0,
                                    &record) ||
      record.occurred_at != 1005 || strcmp(record.host, "good.example") != 0 ||
      !player_account_login_record(database, 0, PLAYER_LOGIN_SUCCESS, 3000,
                                   "latest.example") ||
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

int main(void) {
  GameObject objects[2] = {0};
  GameDatabase database = {.objects = objects, .top = 2, .size = 2};
  DbRef recipients[] = {42, 7, 999};
  time_t last_login;

  game_object_set_type(&database, 0, OBJECT_TYPE_PLAYER);
  game_object_set_type(&database, 1, OBJECT_TYPE_THING);
  if (!player_account_password_hash_set(&database, 0, "hash") ||
      strcmp(player_account_password_hash(&database, 0), "hash") != 0 ||
      player_account_password_hash_set(&database, 1, "invalid") ||
      player_account_login_record(&database, 0, (PlayerLoginOutcome)99, 0,
                                  "invalid") ||
      !player_account_last_login_set(&database, 0, 123456789) ||
      !player_account_last_login(&database, 0, &last_login) ||
      last_login != 123456789 ||
      !player_account_last_site_set(&database, 0, "user@example") ||
      strcmp(player_account_last_site(&database, 0), "user@example") != 0 ||
      check_history(&database) < 0 ||
      !player_account_last_page_set(&database, 0, recipients, 3) ||
      player_account_last_page_count(&database, 0) != 3 ||
      player_account_last_page_recipient(&database, 0, 1) != 7 ||
      check_utc_format() < 0) {
    player_account_clear(&database, 0);
    return 1;
  }
  player_account_clear(&database, 0);
  if (objects[0].account || *player_account_password_hash(&database, 0))
    return 1;
  return 0;
}
