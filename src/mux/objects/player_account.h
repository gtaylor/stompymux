/* player_account.h - Typed player account state. */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <time.h>

#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;
typedef struct PlayerAccountState PlayerAccountState;

typedef enum PlayerLoginOutcome {
  PLAYER_LOGIN_SUCCESS,
  PLAYER_LOGIN_FAILURE,
} PlayerLoginOutcome;

typedef struct PlayerLoginRecordView PlayerLoginRecordView;
struct PlayerLoginRecordView {
  time_t occurred_at;
  const char *host;
};

enum {
  PLAYER_SUCCESS_HISTORY_LIMIT = 4,
  PLAYER_FAILURE_HISTORY_LIMIT = 3,
};

void player_account_clear(GameDatabase *database, DbRef player);

const char *player_account_password_hash(GameDatabase *database, DbRef player);
bool player_account_password_hash_set(GameDatabase *database, DbRef player,
                                      const char *hash);

bool player_account_last_login(GameDatabase *database, DbRef player,
                               time_t *when);
bool player_account_last_login_set(GameDatabase *database, DbRef player,
                                   time_t when);
const char *player_account_last_site(GameDatabase *database, DbRef player);
bool player_account_last_site_set(GameDatabase *database, DbRef player,
                                  const char *site);

int64_t player_account_successful_login_count(GameDatabase *database,
                                              DbRef player);
int64_t player_account_failed_login_count(GameDatabase *database, DbRef player);
int64_t player_account_unreported_failed_login_count(GameDatabase *database,
                                                     DbRef player);
bool player_account_login_counts_set(GameDatabase *database, DbRef player,
                                     int64_t successful, int64_t failed,
                                     int64_t unreported_failed);
bool player_account_login_record(GameDatabase *database, DbRef player,
                                 PlayerLoginOutcome outcome, time_t occurred_at,
                                 const char *host);
size_t player_account_login_history_count(GameDatabase *database, DbRef player,
                                          PlayerLoginOutcome outcome);
bool player_account_login_history(GameDatabase *database, DbRef player,
                                  PlayerLoginOutcome outcome, size_t position,
                                  PlayerLoginRecordView *record);
bool player_account_login_history_set(GameDatabase *database, DbRef player,
                                      PlayerLoginOutcome outcome,
                                      size_t position, time_t occurred_at,
                                      const char *host);

size_t player_account_last_page_count(GameDatabase *database, DbRef player);
DbRef player_account_last_page_recipient(GameDatabase *database, DbRef player,
                                         size_t position);
bool player_account_last_page_set(GameDatabase *database, DbRef player,
                                  const DbRef *recipients, size_t count);

bool player_account_format_timestamp_utc(time_t when, char *buffer,
                                         size_t buffer_size);
