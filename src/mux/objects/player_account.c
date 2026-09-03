/* player_account.c - Typed player account state. */

#include "mux/objects/player_account.h"

#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

typedef struct PlayerLoginRecord PlayerLoginRecord;
struct PlayerLoginRecord {
  time_t occurred_at;
  char *host;
};

struct PlayerAccountState {
  char *alias;
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

static PlayerLoginRecord *login_record(PlayerLoginRecord *records, size_t limit,
                                       size_t index) {
  return checked_storage_at(records, limit, sizeof(*records), index);
}

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
  account = checked_storage_try_allocate_array(1, sizeof(*account));
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
  free(account->alias);
  free(account->password_hash);
  free(account->last_site);
  for (size_t index = 0; index < account->successful_history_count; index++)
    free(login_record(account->successful_history, PLAYER_SUCCESS_HISTORY_LIMIT,
                      index)
             ->host);
  for (size_t index = 0; index < account->failed_history_count; index++)
    free(login_record(account->failed_history, PLAYER_FAILURE_HISTORY_LIMIT,
                      index)
             ->host);
  free(account->last_page_recipients);
  free(account);
  game_database_object(database, player)->account = nullptr;
}

const char *player_account_alias(GameDatabase *database, DbRef player) {
  PlayerAccountState *account = player_account(database, player);
  return account && account->alias ? account->alias : "";
}

bool player_account_alias_set(GameDatabase *database, DbRef player,
                              const char *alias) {
  PlayerAccountState *account = player_account_require(database, player);
  return (account && account_replace_string(&account->alias, alias)) != 0;
}

const char *player_account_password_hash(GameDatabase *database, DbRef player) {
  PlayerAccountState *account = player_account(database, player);
  return account && account->password_hash ? account->password_hash : "";
}

bool player_account_password_hash_set(GameDatabase *database, DbRef player,
                                      const char *hash) {
  PlayerAccountState *account = player_account_require(database, player);
  return (account && account_replace_string(&account->password_hash, hash)) !=
         0;
}

PlayerLastLoginResult player_account_last_login(PlayerAccountRef reference) {
  PlayerAccountState *account =
      player_account(reference.database, reference.player);
  if (!account || !account->has_last_login)
    return (PlayerLastLoginResult){};
  return (PlayerLastLoginResult){.found = true,
                                 .occurred_at = account->last_login};
}

bool player_account_last_login_set(const PlayerLastLoginChange *change) {
  PlayerAccountState *account =
      player_account_require(change->account.database, change->account.player);
  if (!account)
    return false;
  account->last_login = change->occurred_at;
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
  return (account && account_replace_string(&account->last_site, site)) != 0;
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

bool player_account_login_counts_set(const PlayerLoginCountsChange *change) {
  PlayerAccountState *account;

  if (change->successful < 0 || change->failed < 0 ||
      change->unreported_failed < 0 ||
      change->unreported_failed > change->failed)
    return false;
  account =
      player_account_require(change->account.database, change->account.player);
  if (!account)
    return false;
  account->successful_logins = change->successful;
  account->failed_logins = change->failed;
  account->unreported_failed_logins = change->unreported_failed;
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

bool player_account_login_record(const PlayerLoginRecordChange *change) {
  PlayerLoginOutcome outcome = change->outcome;
  PlayerAccountState *account;
  PlayerLoginRecord *records;
  char *copy;
  size_t *count;
  size_t limit;

  if (outcome != PLAYER_LOGIN_SUCCESS && outcome != PLAYER_LOGIN_FAILURE)
    return false;
  account =
      player_account_require(change->account.database, change->account.player);
  if (!account || !change->host)
    return false;
  copy = strdup(change->host);
  if (!copy)
    return false;
  records = history(account, outcome, &count, &limit);
  if (*count == limit)
    free(login_record(records, limit, limit - 1)->host);
  else
    (*count)++;
  if (*count > 1)
    memmove(login_record(records, limit, 1), login_record(records, limit, 0),
            (*count - 1) * sizeof(*records));
  *login_record(records, limit, 0) =
      (PlayerLoginRecord){.occurred_at = change->occurred_at, .host = copy};
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

size_t player_account_login_history_count(PlayerLoginHistoryRequest request) {
  PlayerAccountState *account =
      player_account(request.account.database, request.account.player);
  if (!account || (request.outcome != PLAYER_LOGIN_SUCCESS &&
                   request.outcome != PLAYER_LOGIN_FAILURE))
    return 0;
  return request.outcome == PLAYER_LOGIN_SUCCESS
             ? account->successful_history_count
             : account->failed_history_count;
}

PlayerLoginHistoryResult
player_account_login_history(const PlayerLoginHistoryRequest *request) {
  PlayerAccountState *account =
      player_account(request->account.database, request->account.player);
  PlayerLoginRecord *records;
  size_t *count;
  size_t limit;

  if (!account || (request->outcome != PLAYER_LOGIN_SUCCESS &&
                   request->outcome != PLAYER_LOGIN_FAILURE))
    return (PlayerLoginHistoryResult){};
  records = history(account, request->outcome, &count, &limit);
  if (request->position >= *count)
    return (PlayerLoginHistoryResult){};
  const PlayerLoginRecord *stored =
      login_record(records, limit, request->position);
  return (PlayerLoginHistoryResult){
      .found = true,
      .record = {.occurred_at = stored->occurred_at, .host = stored->host}};
}

bool player_account_login_history_set(const PlayerLoginHistoryChange *change) {
  PlayerLoginOutcome outcome = change->target.outcome;
  size_t position = change->target.position;
  PlayerAccountState *account;
  PlayerLoginRecord *records;
  char *copy;
  size_t *count;
  size_t limit;

  if (outcome != PLAYER_LOGIN_SUCCESS && outcome != PLAYER_LOGIN_FAILURE)
    return false;
  account = player_account_require(change->target.account.database,
                                   change->target.account.player);
  if (!account || !change->host)
    return false;
  copy = strdup(change->host);
  if (!copy)
    return false;
  records = history(account, outcome, &count, &limit);
  if (position >= limit || position > *count) {
    free(copy);
    return false;
  }
  if (position < *count)
    free(login_record(records, limit, position)->host);
  else
    (*count)++;
  *login_record(records, limit, position) =
      (PlayerLoginRecord){.occurred_at = change->occurred_at, .host = copy};
  return true;
}

size_t player_account_last_page_count(GameDatabase *database, DbRef player) {
  PlayerAccountState *account = player_account(database, player);
  return account ? account->last_page_count : 0;
}

PlayerPageRecipientResult
player_account_last_page_recipient(const PlayerPageRecipientRequest *request) {
  PlayerAccountState *account =
      player_account(request->account.database, request->account.player);
  if (!account || request->position >= account->last_page_count)
    return (PlayerPageRecipientResult){};
  return (PlayerPageRecipientResult){
      .found = true,
      .recipient = *(const DbRef *)checked_storage_at_const(
          account->last_page_recipients, account->last_page_count,
          sizeof(DbRef), request->position)};
}

bool player_account_last_page_set(GameDatabase *database, DbRef player,
                                  const DbRef *recipients, size_t count) {
  PlayerAccountState *account = player_account_require(database, player);
  DbRef *copy = nullptr;

  if (!account || (count > 0 && !recipients))
    return false;
  if (count > 0) {
    copy = checked_storage_try_allocate_array(count, sizeof(*copy));
    if (!copy)
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
  return (buffer && buffer_size > 0 && gmtime_r(&when, &utc) &&
          strftime(buffer, buffer_size, "%Y-%m-%dT%H:%M:%SZ", &utc) > 0) != 0;
}
