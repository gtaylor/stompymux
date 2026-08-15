/* player_account.h - Typed player account state. */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <time.h>

#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;
typedef struct PlayerAccountState PlayerAccountState;

typedef enum PlayerLoginOutcome : int {
  PLAYER_LOGIN_SUCCESS,
  PLAYER_LOGIN_FAILURE,
} PlayerLoginOutcome;

typedef struct PlayerLoginRecordView PlayerLoginRecordView;
struct PlayerLoginRecordView {
  time_t occurred_at;
  const char *host;
};

typedef struct PlayerAccountRef {
  GameDatabase *database;
  DbRef player;
} PlayerAccountRef;

typedef struct PlayerLastLoginResult {
  bool found;
  time_t occurred_at;
} PlayerLastLoginResult;

typedef struct PlayerLastLoginChange {
  PlayerAccountRef account;
  time_t occurred_at;
} PlayerLastLoginChange;

typedef struct PlayerLoginCountsChange {
  PlayerAccountRef account;
  int64_t successful;
  int64_t failed;
  int64_t unreported_failed;
} PlayerLoginCountsChange;

typedef struct PlayerLoginRecordChange {
  PlayerAccountRef account;
  PlayerLoginOutcome outcome;
  time_t occurred_at;
  const char *host;
} PlayerLoginRecordChange;

typedef struct PlayerLoginHistoryRequest {
  PlayerAccountRef account;
  PlayerLoginOutcome outcome;
  size_t position;
} PlayerLoginHistoryRequest;

typedef struct PlayerLoginHistoryResult {
  bool found;
  PlayerLoginRecordView record;
} PlayerLoginHistoryResult;

typedef struct PlayerLoginHistoryChange {
  PlayerLoginHistoryRequest target;
  time_t occurred_at;
  const char *host;
} PlayerLoginHistoryChange;

typedef struct PlayerPageRecipientRequest {
  PlayerAccountRef account;
  size_t position;
} PlayerPageRecipientRequest;

typedef struct PlayerPageRecipientResult {
  bool found;
  DbRef recipient;
} PlayerPageRecipientResult;

enum {
  PLAYER_SUCCESS_HISTORY_LIMIT = 4,
  PLAYER_FAILURE_HISTORY_LIMIT = 3,
};

void player_account_clear(GameDatabase *database, DbRef player);

const char *player_account_password_hash(GameDatabase *database, DbRef player);
bool player_account_password_hash_set(GameDatabase *database, DbRef player,
                                      const char *hash);

PlayerLastLoginResult player_account_last_login(PlayerAccountRef reference);
bool player_account_last_login_set(const PlayerLastLoginChange *change);
const char *player_account_last_site(GameDatabase *database, DbRef player);
bool player_account_last_site_set(GameDatabase *database, DbRef player,
                                  const char *site);

int64_t player_account_successful_login_count(GameDatabase *database,
                                              DbRef player);
int64_t player_account_failed_login_count(GameDatabase *database, DbRef player);
int64_t player_account_unreported_failed_login_count(GameDatabase *database,
                                                     DbRef player);
bool player_account_login_counts_set(const PlayerLoginCountsChange *change);
bool player_account_login_record(const PlayerLoginRecordChange *change);
size_t player_account_login_history_count(PlayerLoginHistoryRequest request);
PlayerLoginHistoryResult
player_account_login_history(const PlayerLoginHistoryRequest *request);
bool player_account_login_history_set(const PlayerLoginHistoryChange *change);

size_t player_account_last_page_count(GameDatabase *database, DbRef player);
PlayerPageRecipientResult
player_account_last_page_recipient(const PlayerPageRecipientRequest *request);
bool player_account_last_page_set(GameDatabase *database, DbRef player,
                                  const DbRef *recipients, size_t count);

bool player_account_format_timestamp_utc(time_t when, char *buffer,
                                         size_t buffer_size);
