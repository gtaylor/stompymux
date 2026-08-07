/* player_account.c - Typed player account state. */

#include "mux/objects/player_account.h"

#include <limits.h>
#include <stdlib.h>
#include <string.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"

typedef struct PlayerLoginRecord PlayerLoginRecord;
struct PlayerLoginRecord {
  time_t occurred_at;
  char *host;
};

struct PlayerAccountState {
  char *password_hash;
  char *last_site;
  time_t last_login;
  bool has_last_login;
  int64_t successful_logins;
  int64_t failed_logins;
  int64_t unreported_failed_logins;
  PlayerLoginRecord successful_history[PLAYER_SUCCESS_HISTORY_LIMIT];
  PlayerLoginRecord failed_history[PLAYER_FAILURE_HISTORY_LIMIT];
  size_t successful_history_count;
  size_t failed_history_count;
  DbRef *last_page_recipients;
  size_t last_page_count;
};

static PlayerAccountState *player_account(GameDatabase *database,
                                          DbRef player) {
  if (!is_good_obj(database, player) ||
      typeof_obj(database, player) != OBJECT_TYPE_PLAYER)
    return nullptr;
  return game_database_object(database, player)->account;
}

static PlayerAccountState *player_account_require(GameDatabase *database,
                                                  DbRef player) {
  PlayerAccountState *account = player_account(database, player);

  if (account)
    return account;
  if (!is_good_obj(database, player) ||
      typeof_obj(database, player) != OBJECT_TYPE_PLAYER)
    return nullptr;
  account = calloc(1, sizeof(*account));
  if (account)
    game_database_object(database, player)->account = account;
  return account;
}

static bool account_replace_string(char **target, const char *value) {
  char *copy = nullptr;

  if (value && *value) {
    copy = strdup(value);
    if (!copy)
      return false;
  }
  free(*target);
  *target = copy;
  return true;
}

void player_account_clear(GameDatabase *database, DbRef player) {
  PlayerAccountState *account;

  if (!database || player < 0 || player >= database->top)
    return;
  account = game_database_object(database, player)->account;
  if (!account)
    return;
  free(account->password_hash);
  free(account->last_site);
  for (size_t index = 0; index < account->successful_history_count; index++)
    free(account->successful_history[index].host);
  for (size_t index = 0; index < account->failed_history_count; index++)
    free(account->failed_history[index].host);
  free(account->last_page_recipients);
  free(account);
  game_database_object(database, player)->account = nullptr;
}

const char *player_account_password_hash(GameDatabase *database, DbRef player) {
  PlayerAccountState *account = player_account(database, player);
  return account && account->password_hash ? account->password_hash : "";
}

bool player_account_password_hash_set(GameDatabase *database, DbRef player,
                                      const char *hash) {
  PlayerAccountState *account = player_account_require(database, player);
  return account && account_replace_string(&account->password_hash, hash);
}

bool player_account_last_login(GameDatabase *database, DbRef player,
                               time_t *when) {
  PlayerAccountState *account = player_account(database, player);
  if (!account || !account->has_last_login)
    return false;
  if (when)
    *when = account->last_login;
  return true;
}

bool player_account_last_login_set(GameDatabase *database, DbRef player,
                                   time_t when) {
  PlayerAccountState *account = player_account_require(database, player);
  if (!account)
    return false;
  account->last_login = when;
  account->has_last_login = true;
  return true;
}

const char *player_account_last_site(GameDatabase *database, DbRef player) {
  PlayerAccountState *account = player_account(database, player);
  return account && account->last_site ? account->last_site : "";
}

bool player_account_last_site_set(GameDatabase *database, DbRef player,
                                  const char *site) {
  PlayerAccountState *account = player_account_require(database, player);
  return account && account_replace_string(&account->last_site, site);
}

int64_t player_account_successful_login_count(GameDatabase *database,
                                              DbRef player) {
  PlayerAccountState *account = player_account(database, player);
  return account ? account->successful_logins : 0;
}

int64_t player_account_failed_login_count(GameDatabase *database,
                                          DbRef player) {
  PlayerAccountState *account = player_account(database, player);
  return account ? account->failed_logins : 0;
}

int64_t player_account_unreported_failed_login_count(GameDatabase *database,
                                                     DbRef player) {
  PlayerAccountState *account = player_account(database, player);
  return account ? account->unreported_failed_logins : 0;
}

bool player_account_login_counts_set(GameDatabase *database, DbRef player,
                                     int64_t successful, int64_t failed,
                                     int64_t unreported_failed) {
  PlayerAccountState *account;

  if (successful < 0 || failed < 0 || unreported_failed < 0 ||
      unreported_failed > failed)
    return false;
  account = player_account_require(database, player);
  if (!account)
    return false;
  account->successful_logins = successful;
  account->failed_logins = failed;
  account->unreported_failed_logins = unreported_failed;
  return true;
}

static PlayerLoginRecord *history(PlayerAccountState *account,
                                  PlayerLoginOutcome outcome,
                                  size_t **count_pointer, size_t *limit) {
  if (outcome == PLAYER_LOGIN_SUCCESS) {
    *count_pointer = &account->successful_history_count;
    *limit = PLAYER_SUCCESS_HISTORY_LIMIT;
    return account->successful_history;
  }
  *count_pointer = &account->failed_history_count;
  *limit = PLAYER_FAILURE_HISTORY_LIMIT;
  return account->failed_history;
}

bool player_account_login_record(GameDatabase *database, DbRef player,
                                 PlayerLoginOutcome outcome, time_t occurred_at,
                                 const char *host) {
  PlayerAccountState *account;
  PlayerLoginRecord *records;
  char *copy;
  size_t *count;
  size_t limit;

  if ((outcome != PLAYER_LOGIN_SUCCESS && outcome != PLAYER_LOGIN_FAILURE) ||
      !(account = player_account_require(database, player)) || !host ||
      !(copy = strdup(host)))
    return false;
  records = history(account, outcome, &count, &limit);
  if (*count == limit)
    free(records[limit - 1].host);
  else
    (*count)++;
  memmove(&records[1], &records[0], (*count - 1) * sizeof(*records));
  records[0] = (PlayerLoginRecord){.occurred_at = occurred_at, .host = copy};
  if (outcome == PLAYER_LOGIN_SUCCESS) {
    if (account->successful_logins < INT64_MAX)
      account->successful_logins++;
    account->unreported_failed_logins = 0;
  } else {
    if (account->failed_logins < INT64_MAX)
      account->failed_logins++;
    if (account->unreported_failed_logins < INT64_MAX)
      account->unreported_failed_logins++;
  }
  return true;
}

size_t player_account_login_history_count(GameDatabase *database, DbRef player,
                                          PlayerLoginOutcome outcome) {
  PlayerAccountState *account = player_account(database, player);
  if (!account ||
      (outcome != PLAYER_LOGIN_SUCCESS && outcome != PLAYER_LOGIN_FAILURE))
    return 0;
  return outcome == PLAYER_LOGIN_SUCCESS ? account->successful_history_count
                                         : account->failed_history_count;
}

bool player_account_login_history(GameDatabase *database, DbRef player,
                                  PlayerLoginOutcome outcome, size_t position,
                                  PlayerLoginRecordView *record) {
  PlayerAccountState *account = player_account(database, player);
  PlayerLoginRecord *records;
  size_t *count;
  size_t limit;

  if (!account || !record ||
      (outcome != PLAYER_LOGIN_SUCCESS && outcome != PLAYER_LOGIN_FAILURE))
    return false;
  records = history(account, outcome, &count, &limit);
  if (position >= *count)
    return false;
  *record =
      (PlayerLoginRecordView){.occurred_at = records[position].occurred_at,
                              .host = records[position].host};
  return true;
}

bool player_account_login_history_set(GameDatabase *database, DbRef player,
                                      PlayerLoginOutcome outcome,
                                      size_t position, time_t occurred_at,
                                      const char *host) {
  PlayerAccountState *account;
  PlayerLoginRecord *records;
  char *copy;
  size_t *count;
  size_t limit;

  if ((outcome != PLAYER_LOGIN_SUCCESS && outcome != PLAYER_LOGIN_FAILURE) ||
      !(account = player_account_require(database, player)) || !host ||
      !(copy = strdup(host)))
    return false;
  records = history(account, outcome, &count, &limit);
  if (position >= limit || position > *count) {
    free(copy);
    return false;
  }
  if (position < *count)
    free(records[position].host);
  else
    (*count)++;
  records[position] =
      (PlayerLoginRecord){.occurred_at = occurred_at, .host = copy};
  return true;
}

size_t player_account_last_page_count(GameDatabase *database, DbRef player) {
  PlayerAccountState *account = player_account(database, player);
  return account ? account->last_page_count : 0;
}

DbRef player_account_last_page_recipient(GameDatabase *database, DbRef player,
                                         size_t position) {
  PlayerAccountState *account = player_account(database, player);
  if (!account || position >= account->last_page_count)
    return NOTHING;
  return account->last_page_recipients[position];
}

bool player_account_last_page_set(GameDatabase *database, DbRef player,
                                  const DbRef *recipients, size_t count) {
  PlayerAccountState *account = player_account_require(database, player);
  DbRef *copy = nullptr;

  if (!account || (count > 0 && !recipients))
    return false;
  if (count > 0) {
    if (count > SIZE_MAX / sizeof(*copy) ||
        !(copy = malloc(count * sizeof(*copy))))
      return false;
    memcpy(copy, recipients, count * sizeof(*copy));
  }
  free(account->last_page_recipients);
  account->last_page_recipients = copy;
  account->last_page_count = count;
  return true;
}

bool player_account_format_timestamp_utc(time_t when, char *buffer,
                                         size_t buffer_size) {
  struct tm utc;
  return buffer && buffer_size > 0 && gmtime_r(&when, &utc) &&
         strftime(buffer, buffer_size, "%Y-%m-%dT%H:%M:%SZ", &utc) > 0;
}
