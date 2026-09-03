/** @file
 * Typed player account state.
 */
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

/** Clears player account. @param[in,out] database Game database. @param[in]
 * player Player object. */

void player_account_clear(GameDatabase *database, DbRef player);

/** Returns a player's login alias, or an empty string when unset. */

const char *player_account_alias(GameDatabase *database, DbRef player);
/** Sets or clears a player's login alias. */

[[nodiscard]] bool player_account_alias_set(GameDatabase *database,
                                            DbRef player, const char *alias);

/** Executes player account password hash. @param[in,out] database Game
 * database. @param[in] player Player object. */

const char *player_account_password_hash(GameDatabase *database, DbRef player);
/** Sets player account password hash. @param[in,out] database Game database.
 * @param[in] player Player object. @param[in] hash Hash. */

[[nodiscard]] bool player_account_password_hash_set(GameDatabase *database,
                                                    DbRef player,
                                                    const char *hash);

/** Executes player account last login. @param[in] reference Reference. */

PlayerLastLoginResult player_account_last_login(PlayerAccountRef reference);
/** Sets player account last login. @param[in] change Change. */

[[nodiscard]] bool
player_account_last_login_set(const PlayerLastLoginChange *change);
/** Executes player account last site. @param[in,out] database Game database.
 * @param[in] player Player object. */

const char *player_account_last_site(GameDatabase *database, DbRef player);
/** Sets player account last site. @param[in,out] database Game database.
 * @param[in] player Player object. @param[in] site Site. */

[[nodiscard]] bool player_account_last_site_set(GameDatabase *database,
                                                DbRef player, const char *site);

/** Counts player account successful login. @param[in] database Game database.
 * @param[in] player Player object. */

int64_t player_account_successful_login_count(GameDatabase *database,
                                              DbRef player);
/** Counts player account failed login. @param[in] database Game database.
 * @param[in] player Player object. */

int64_t player_account_failed_login_count(GameDatabase *database, DbRef player);
/** Counts player account unreported failed login. @param[in] database Game
 * database. @param[in] player Player object. */

int64_t player_account_unreported_failed_login_count(GameDatabase *database,
                                                     DbRef player);
/** Sets player account login counts. @param[in] change Change. */

[[nodiscard]] bool
player_account_login_counts_set(const PlayerLoginCountsChange *change);
/** Executes player account login record. @param[in] change Change. */

[[nodiscard]] bool
player_account_login_record(const PlayerLoginRecordChange *change);
/** Counts player account login history. @param[in] request Request. */

size_t player_account_login_history_count(PlayerLoginHistoryRequest request);
/** Executes player account login history. @param[in] request Request. */

PlayerLoginHistoryResult
player_account_login_history(const PlayerLoginHistoryRequest *request);
/** Sets player account login history. @param[in] change Change. */

[[nodiscard]] bool
player_account_login_history_set(const PlayerLoginHistoryChange *change);

/** Counts player account last page. @param[in] database Game database.
 * @param[in] player Player object. */

size_t player_account_last_page_count(GameDatabase *database, DbRef player);
/** Executes player account last page recipient. @param[in] request Request. */

PlayerPageRecipientResult
player_account_last_page_recipient(const PlayerPageRecipientRequest *request);
/** Sets player account last page. @param[in,out] database Game database.
 * @param[in] player Player object. @param[in] recipients Recipients. @param[in]
 * count Number of elements. */

[[nodiscard]] bool player_account_last_page_set(GameDatabase *database,
                                                DbRef player,
                                                const DbRef *recipients,
                                                size_t count);

/** Executes player account format timestamp utc. @param[in] when When.
 * @param[out] buffer Caller-owned output storage. @param[in] buffer_size Size
 * of buffer in bytes. */

[[nodiscard]] bool player_account_format_timestamp_utc(time_t when,
                                                       char *buffer,
                                                       size_t buffer_size);
