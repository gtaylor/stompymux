/* commac_persistence_sqlite.c -- commac, comsys, and macro persistence */

#include "config.h"

#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "commac.h"
#include "comsys.h"
#include "db.h"
#include "macro.h"
#include "mudconf.h"
#include "persistence/commac_persistence.h"
#include "persistence/gamedb.h"

/* The legacy file is read during this phase; SQLite is an atomic dump mirror. */
static const char commac_schema_sql[] =
    "CREATE TABLE commac_entries ("
    " who INTEGER PRIMARY KEY, curmac INTEGER NOT NULL,"
    " macro_slot_0 INTEGER NOT NULL, macro_slot_1 INTEGER NOT NULL,"
    " macro_slot_2 INTEGER NOT NULL, macro_slot_3 INTEGER NOT NULL,"
    " macro_slot_4 INTEGER NOT NULL"
    ");"
    "CREATE TABLE commac_aliases ("
    " who INTEGER NOT NULL, position INTEGER NOT NULL, alias TEXT NOT NULL,"
    " channel_name TEXT NOT NULL, PRIMARY KEY (who, position)"
    ") WITHOUT ROWID;"
    "CREATE TABLE comsys_channels ("
    " name TEXT PRIMARY KEY, type INTEGER NOT NULL, temp1 INTEGER NOT NULL,"
    " temp2 INTEGER NOT NULL, charge INTEGER NOT NULL,"
    " charge_who INTEGER NOT NULL, amount_col INTEGER NOT NULL,"
    " num_messages INTEGER NOT NULL, chan_obj INTEGER NOT NULL"
    ") WITHOUT ROWID;"
    "CREATE TABLE comsys_channel_users ("
    " channel_name TEXT NOT NULL, position INTEGER NOT NULL, who INTEGER NOT NULL,"
    " is_on INTEGER NOT NULL, title TEXT NOT NULL,"
    " PRIMARY KEY (channel_name, position)"
    ") WITHOUT ROWID;"
    "CREATE TABLE comsys_channel_messages ("
    " channel_name TEXT NOT NULL, position INTEGER NOT NULL,"
    " sent_at INTEGER NOT NULL, message TEXT NOT NULL,"
    " PRIMARY KEY (channel_name, position)"
    ") WITHOUT ROWID;"
    "CREATE TABLE macro_sets ("
    " set_index INTEGER PRIMARY KEY, owner INTEGER NOT NULL, status INTEGER NOT NULL,"
    " description TEXT NOT NULL"
    ");"
    "CREATE TABLE macro_entries ("
    " set_index INTEGER NOT NULL, position INTEGER NOT NULL, alias TEXT NOT NULL,"
    " expansion TEXT NOT NULL, PRIMARY KEY (set_index, position)"
    ") WITHOUT ROWID;";

static int commac_sqlite_exec(sqlite3 *sqlite, const char *sql) {
  char *error = NULL;
  int rc = sqlite3_exec(sqlite, sql, NULL, NULL, &error);

  sqlite3_free(error);
  return rc == SQLITE_OK ? 0 : -1;
}

static int commac_sqlite_step(sqlite3_stmt *statement) {
  if (sqlite3_step(statement) != SQLITE_DONE ||
      sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

static int commac_sqlite_bind_int(sqlite3_stmt *statement, int index,
                                  long value) {
  return sqlite3_bind_int64(statement, index, (sqlite3_int64)value) == SQLITE_OK
             ? 0
             : -1;
}

static int commac_sqlite_bind_text(sqlite3_stmt *statement, int index,
                                   const char *value) {
  return sqlite3_bind_text(statement, index, value ? value : "", -1,
                           SQLITE_TRANSIENT) == SQLITE_OK
             ? 0
             : -1;
}

/* Save all per-object aliases and selected macro-set slots. */
static int commac_store_entries(sqlite3 *sqlite) {
  sqlite3_stmt *entry = NULL;
  sqlite3_stmt *alias = NULL;
  struct commac *commac;
  int bucket;
  int index;
  int result = -1;

  purge_commac();
  if (sqlite3_prepare_v2(sqlite,
                         "INSERT INTO commac_entries "
                         "(who, curmac, macro_slot_0, macro_slot_1, "
                         "macro_slot_2, macro_slot_3, macro_slot_4) "
                         "VALUES (?, ?, ?, ?, ?, ?, ?);",
                         -1, &entry, NULL) == SQLITE_OK &&
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO commac_aliases "
                         "(who, position, alias, channel_name) VALUES (?, ?, ?, ?);",
                         -1, &alias, NULL) == SQLITE_OK) {
    result = 0;
    for (bucket = 0; result == 0 && bucket < NUM_COMMAC; bucket++) {
      for (commac = commac_table[bucket]; commac; commac = commac->next) {
        if (commac_sqlite_bind_int(entry, 1, commac->who) < 0 ||
            commac_sqlite_bind_int(entry, 2, commac->curmac) < 0 ||
            commac_sqlite_bind_int(entry, 3, commac->macros[0]) < 0 ||
            commac_sqlite_bind_int(entry, 4, commac->macros[1]) < 0 ||
            commac_sqlite_bind_int(entry, 5, commac->macros[2]) < 0 ||
            commac_sqlite_bind_int(entry, 6, commac->macros[3]) < 0 ||
            commac_sqlite_bind_int(entry, 7, commac->macros[4]) < 0 ||
            commac_sqlite_step(entry) < 0) {
          result = -1;
          break;
        }
        for (index = 0; result == 0 && index < commac->numchannels; index++) {
          if (commac_sqlite_bind_int(alias, 1, commac->who) < 0 ||
              commac_sqlite_bind_int(alias, 2, index) < 0 ||
              commac_sqlite_bind_text(alias, 3, commac->alias + index * 6) < 0 ||
              commac_sqlite_bind_text(alias, 4, commac->channels[index]) < 0 ||
              commac_sqlite_step(alias) < 0)
            result = -1;
        }
      }
    }
  }
  sqlite3_finalize(entry);
  sqlite3_finalize(alias);
  return result;
}

/* This context lets myfifo's callback insert channel history in FIFO order. */
static sqlite3_stmt *commac_message_statement;
static const char *commac_message_channel;
static int commac_message_position;
static int commac_message_result;

static void commac_store_message(void *data) {
  chmsg *message = data;

  if (commac_message_result < 0 ||
      commac_sqlite_bind_text(commac_message_statement, 1,
                              commac_message_channel) < 0 ||
      commac_sqlite_bind_int(commac_message_statement, 2,
                             commac_message_position++) < 0 ||
      commac_sqlite_bind_int(commac_message_statement, 3, message->time) < 0 ||
      commac_sqlite_bind_text(commac_message_statement, 4, message->msg) < 0 ||
      commac_sqlite_step(commac_message_statement) < 0)
    commac_message_result = -1;
}

/* Save configured channels, persisted player memberships, and history. */
static int commac_store_comsys(sqlite3 *sqlite) {
  sqlite3_stmt *channel = NULL;
  sqlite3_stmt *user_statement = NULL;
  sqlite3_stmt *message = NULL;
  struct channel *current;
  struct comuser *user;
  int index;
  int position;
  int result = -1;

  if (sqlite3_prepare_v2(sqlite,
                         "INSERT INTO comsys_channels "
                         "(name, type, temp1, temp2, charge, charge_who, amount_col, "
                         "num_messages, chan_obj) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);",
                         -1, &channel, NULL) == SQLITE_OK &&
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO comsys_channel_users "
                         "(channel_name, position, who, is_on, title) "
                         "VALUES (?, ?, ?, ?, ?);",
                         -1, &user_statement, NULL) == SQLITE_OK &&
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO comsys_channel_messages "
                         "(channel_name, position, sent_at, message) VALUES (?, ?, ?, ?);",
                         -1, &message, NULL) == SQLITE_OK) {
    result = 0;
    for (current = (struct channel *)hash_firstentry(&mudstate.channel_htab);
       result == 0 && current;
       current = (struct channel *)hash_nextentry(&mudstate.channel_htab)) {
    if (commac_sqlite_bind_text(channel, 1, current->name) < 0 ||
        commac_sqlite_bind_int(channel, 2, current->type) < 0 ||
        commac_sqlite_bind_int(channel, 3, current->temp1) < 0 ||
        commac_sqlite_bind_int(channel, 4, current->temp2) < 0 ||
        commac_sqlite_bind_int(channel, 5, current->charge) < 0 ||
        commac_sqlite_bind_int(channel, 6, current->charge_who) < 0 ||
        commac_sqlite_bind_int(channel, 7, current->amount_col) < 0 ||
        commac_sqlite_bind_int(channel, 8, current->num_messages) < 0 ||
        commac_sqlite_bind_int(channel, 9, current->chan_obj) < 0 ||
        commac_sqlite_step(channel) < 0) {
      result = -1;
      break;
    }
    position = 0;
    for (index = 0; result == 0 && index < current->num_users; index++) {
      user = current->users[index];
      if (!isPlayer(user->who) && !isRobot(user->who))
        continue;
      if (commac_sqlite_bind_text(user_statement, 1, current->name) < 0 ||
          commac_sqlite_bind_int(user_statement, 2, position++) < 0 ||
          commac_sqlite_bind_int(user_statement, 3, user->who) < 0 ||
          commac_sqlite_bind_int(user_statement, 4, user->on) < 0 ||
          commac_sqlite_bind_text(user_statement, 5, user->title) < 0 ||
          commac_sqlite_step(user_statement) < 0)
        result = -1;
    }
    commac_message_statement = message;
    commac_message_channel = current->name;
    commac_message_position = 0;
    commac_message_result = 0;
    myfifo_trav_r(&current->last_messages, commac_store_message);
    if (commac_message_result < 0)
      result = -1;
    }
  }
  sqlite3_finalize(channel);
  sqlite3_finalize(user_statement);
  sqlite3_finalize(message);
  return result;
}

/* Save macro-set numbering because commac slots refer to these indexes. */
static int commac_store_macros(sqlite3 *sqlite) {
  sqlite3_stmt *set = NULL;
  sqlite3_stmt *entry = NULL;
  struct macros *macro;
  int index;
  int macro_index;
  int result = -1;

  if (sqlite3_prepare_v2(sqlite,
                         "INSERT INTO macro_sets "
                         "(set_index, owner, status, description) VALUES (?, ?, ?, ?);",
                         -1, &set, NULL) == SQLITE_OK &&
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO macro_entries "
                         "(set_index, position, alias, expansion) VALUES (?, ?, ?, ?);",
                         -1, &entry, NULL) == SQLITE_OK) {
    result = 0;
    for (index = 0; result == 0 && index < nummacros; index++) {
    macro = macros[index];
    if (commac_sqlite_bind_int(set, 1, index) < 0 ||
        commac_sqlite_bind_int(set, 2, macro->player) < 0 ||
        commac_sqlite_bind_int(set, 3, macro->status) < 0 ||
        commac_sqlite_bind_text(set, 4, macro->desc) < 0 ||
        commac_sqlite_step(set) < 0) {
      result = -1;
      break;
    }
    for (macro_index = 0; result == 0 && macro_index < macro->nummacros;
         macro_index++) {
      if (commac_sqlite_bind_int(entry, 1, index) < 0 ||
          commac_sqlite_bind_int(entry, 2, macro_index) < 0 ||
          commac_sqlite_bind_text(entry, 3, macro->alias + macro_index * 5) <
              0 ||
          commac_sqlite_bind_text(entry, 4, macro->string[macro_index]) < 0 ||
          commac_sqlite_step(entry) < 0)
        result = -1;
    }
  }
  }
  sqlite3_finalize(set);
  sqlite3_finalize(entry);
  return result;
}

static int commac_persistence_store(sqlite3 *sqlite) {
  if (!mudconf.have_comsys && !mudconf.have_macros)
    return 0;
  return commac_sqlite_exec(sqlite, commac_schema_sql) < 0 ||
                 commac_store_entries(sqlite) < 0 || commac_store_comsys(sqlite) < 0 ||
                 commac_store_macros(sqlite) < 0
             ? -1
             : 0;
}

/* Read one integer column only when SQLite stored an integer value. */
static int commac_column_int(sqlite3_stmt *statement, int column, long *value) {
  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  *value = (long)sqlite3_column_int64(statement, column);
  return 0;
}

/* Read bounded text, rejecting malformed rows before allocating memory. */
static int commac_column_text(sqlite3_stmt *statement, int column,
                              const char **value, size_t maximum) {
  int length;

  if (sqlite3_column_type(statement, column) != SQLITE_TEXT)
    return -1;
  *value = (const char *)sqlite3_column_text(statement, column);
  length = sqlite3_column_bytes(statement, column);
  return *value && length >= 0 && (size_t)length <= maximum ? 0 : -1;
}

/* Add one persisted alias while retaining commac's sorted runtime layout. */
static int commac_load_alias(struct commac *commac, const char *alias,
                             const char *channel) {
  int capacity;

  if (strlen(alias) > 5)
    return -1;
  if (commac->numchannels == commac->maxchannels) {
    capacity = commac->maxchannels + 10;
    commac->alias = realloc(commac->alias, (size_t)capacity * 6);
    commac->channels = realloc(commac->channels,
                               sizeof(char *) * (size_t)capacity);
    if (!commac->alias || !commac->channels)
      return -1;
    commac->maxchannels = capacity;
  }
  StringCopy(commac->alias + commac->numchannels * 6, alias);
  commac->channels[commac->numchannels++] = strdup(channel);
  return 0;
}

/* Find an existing entry without creating one for a malformed alias row. */
static struct commac *commac_find_loaded(dbref who) {
  struct commac *commac;

  if (who < 0)
    return NULL;
  for (commac = commac_table[who % NUM_COMMAC]; commac;
       commac = commac->next) {
    if (commac->who == who)
      return commac;
  }
  return NULL;
}

/* Restore commac records and aliases before macro slots are validated. */
static int commac_load_entries(sqlite3 *sqlite) {
  sqlite3_stmt *entries = NULL;
  sqlite3_stmt *aliases = NULL;
  struct commac *commac;
  const char *alias;
  const char *channel;
  long value;
  long who;
  int slot;
  int result = -1;
  int step;

  if (sqlite3_prepare_v2(sqlite,
                         "SELECT who, curmac, macro_slot_0, macro_slot_1, "
                         "macro_slot_2, macro_slot_3, macro_slot_4 "
                         "FROM commac_entries ORDER BY who;",
                         -1, &entries, NULL) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(entries)) == SQLITE_ROW) {
      commac = create_new_commac();
      if (commac_column_int(entries, 0, &who) < 0 || who < 0 ||
          who >= mudstate.db_top ||
          commac_column_int(entries, 1, &value) < 0) {
        destroy_commac(commac);
        result = -1;
        break;
      }
      commac->who = who;
      commac->curmac = (int)value;
      for (slot = 0; slot < MAX_SLOTS && result == 0; slot++) {
        if (commac_column_int(entries, slot + 2, &value) < 0)
          result = -1;
        else
          commac->macros[slot] = (int)value;
      }
      if (result < 0) {
        destroy_commac(commac);
        break;
      }
      add_commac(commac);
    }
    if (result == 0 && step != SQLITE_DONE)
      result = -1;
  }
  sqlite3_finalize(entries);
  if (result == 0 && sqlite3_prepare_v2(
                         sqlite,
                         "SELECT who, alias, channel_name FROM commac_aliases "
                         "ORDER BY who, position;",
                         -1, &aliases, NULL) == SQLITE_OK) {
    while (result == 0 && (step = sqlite3_step(aliases)) == SQLITE_ROW) {
      if (commac_column_int(aliases, 0, &who) < 0 ||
          !(commac = commac_find_loaded(who)) ||
          commac_column_text(aliases, 1, &alias, 5) < 0 ||
          commac_column_text(aliases, 2, &channel, LBUF_SIZE - 1) < 0 ||
          commac_load_alias(commac, alias, channel) < 0)
        result = -1;
    }
    if (result == 0 && step != SQLITE_DONE)
      result = -1;
  } else if (result == 0)
    result = -1;
  sqlite3_finalize(aliases);
  if (result == 0) {
    for (who = 0; who < NUM_COMMAC; who++) {
      for (commac = commac_table[who]; commac; commac = commac->next)
        sort_com_aliases(commac);
    }
    purge_commac();
  }
  return result;
}

/* Restore channels first so memberships and history can reference them. */
static int commac_load_channels(sqlite3 *sqlite) {
  sqlite3_stmt *statement = NULL;
  struct channel *channel;
  const char *name;
  long value;
  int result = -1;
  int step;

  num_channels = 0;
  if (sqlite3_prepare_v2(sqlite,
                         "SELECT name, type, temp1, temp2, charge, charge_who, "
                         "amount_col, num_messages, chan_obj "
                         "FROM comsys_channels ORDER BY name;",
                         -1, &statement, NULL) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
      channel = calloc(1, sizeof(*channel));
      if (!channel || commac_column_text(statement, 0, &name, CHAN_NAME_LEN - 1) < 0 ||
          commac_column_int(statement, 1, &value) < 0) {
        free(channel);
        result = -1;
        break;
      }
      StringCopy(channel->name, name);
      channel->type = (int)value;
      if (commac_column_int(statement, 2, &value) < 0) result = -1; else channel->temp1 = (int)value;
      if (commac_column_int(statement, 3, &value) < 0) result = -1; else channel->temp2 = (int)value;
      if (commac_column_int(statement, 4, &value) < 0) result = -1; else channel->charge = (int)value;
      if (commac_column_int(statement, 5, &value) < 0) result = -1; else channel->charge_who = (int)value;
      if (commac_column_int(statement, 6, &value) < 0) result = -1; else channel->amount_col = (int)value;
      if (commac_column_int(statement, 7, &value) < 0) result = -1; else channel->num_messages = (int)value;
      if (commac_column_int(statement, 8, &value) < 0) result = -1; else channel->chan_obj = (int)value;
      if (result < 0) { free(channel); break; }
      hashadd(channel->name, (int *)channel, &mudstate.channel_htab);
      num_channels++;
    }
    if (result == 0 && step != SQLITE_DONE) result = -1;
  }
  sqlite3_finalize(statement);
  return result;
}

/* Restore the persisted player/robot channel memberships. */
static int commac_load_users(sqlite3 *sqlite) {
  sqlite3_stmt *statement = NULL;
  struct channel *channel;
  struct comuser *user;
  const char *name;
  const char *title;
  long who;
  long is_on;
  int result = -1;
  int step;

  if (sqlite3_prepare_v2(sqlite,
                         "SELECT channel_name, who, is_on, title "
                         "FROM comsys_channel_users ORDER BY channel_name, position;",
                         -1, &statement, NULL) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
      if (commac_column_text(statement, 0, &name, CHAN_NAME_LEN - 1) < 0 ||
          !(channel = select_channel((char *)name)) ||
          commac_column_int(statement, 1, &who) < 0 || who < 0 ||
          who >= mudstate.db_top ||
          commac_column_int(statement, 2, &is_on) < 0 ||
          commac_column_text(statement, 3, &title, LBUF_SIZE - 1) < 0) {
        result = -1;
        break;
      }
      user = calloc(1, sizeof(*user));
      if (!user) { result = -1; break; }
      if (channel->num_users == channel->max_users) {
        channel->max_users += 10;
        channel->users = realloc(channel->users,
                                 sizeof(*channel->users) * channel->max_users);
        if (!channel->users) { free(user); result = -1; break; }
      }
      user->who = who;
      user->on = (int)is_on;
      user->title = strdup(title);
      channel->users[channel->num_users++] = user;
      if (UNDEAD(who)) { user->on_next = channel->on_users; channel->on_users = user; }
    }
    if (result == 0 && step != SQLITE_DONE) result = -1;
  }
  sqlite3_finalize(statement);
  return result;
}

/* Restore bounded channel history in the oldest-to-newest queue order. */
static int commac_load_messages(sqlite3 *sqlite) {
  sqlite3_stmt *statement = NULL;
  struct channel *channel;
  chmsg *message;
  const char *name;
  const char *text;
  long sent_at;
  int result = -1;
  int step;

  if (sqlite3_prepare_v2(sqlite,
                         "SELECT channel_name, sent_at, message FROM "
                         "comsys_channel_messages ORDER BY channel_name, position;",
                         -1, &statement, NULL) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
      if (commac_column_text(statement, 0, &name, CHAN_NAME_LEN - 1) < 0 ||
          !(channel = select_channel((char *)name)) ||
          commac_column_int(statement, 1, &sent_at) < 0 ||
          commac_column_text(statement, 2, &text, LBUF_SIZE - 1) < 0) {
        result = -1;
        break;
      }
      message = malloc(sizeof(*message));
      if (!message) { result = -1; break; }
      message->time = (time_t)sent_at;
      message->msg = strdup(text);
      myfifo_push(&channel->last_messages, message);
    }
    if (result == 0 && step != SQLITE_DONE) result = -1;
  }
  sqlite3_finalize(statement);
  return result;
}

/* Restore macro indexes and definitions, then drop owners no longer in the DB. */
static int commac_load_macros(sqlite3 *sqlite) {
  sqlite3_stmt *sets = NULL;
  sqlite3_stmt *entries = NULL;
  struct macros *macro;
  const char *description;
  const char *alias;
  const char *expansion;
  long value;
  long owner;
  long status;
  int expected_set;
  int expected_entry;
  int result = -1;
  int step;

  nummacros = 0;
  maxmacros = 0;
  macros = NULL;
  if (sqlite3_prepare_v2(sqlite,
                         "SELECT set_index, owner, status, description FROM macro_sets "
                         "ORDER BY set_index;", -1, &sets, NULL) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(sets)) == SQLITE_ROW) {
      expected_set = nummacros;
      if (commac_column_int(sets, 0, &value) < 0 || value != expected_set ||
          commac_column_int(sets, 1, &owner) < 0 || owner < 0 ||
          owner >= mudstate.db_top ||
          commac_column_int(sets, 2, &status) < 0 ||
          commac_column_text(sets, 3, &description, LBUF_SIZE - 1) < 0) {
        result = -1;
        break;
      }
      macros = realloc(macros, sizeof(*macros) * (nummacros + 1));
      if (!macros) { result = -1; break; }
      macro = calloc(1, sizeof(*macro));
      if (!macro) { result = -1; break; }
      macros[nummacros++] = macro;
      macro->player = (int)owner;
      macro->status = (char)status;
      macro->desc = strdup(description);
    }
    if (result == 0 && step != SQLITE_DONE) result = -1;
  }
  sqlite3_finalize(sets);
  if (result == 0 && sqlite3_prepare_v2(
                         sqlite,
                         "SELECT set_index, position, alias, expansion FROM macro_entries "
                         "ORDER BY set_index, position;", -1, &entries, NULL) == SQLITE_OK) {
    expected_set = 0;
    expected_entry = 0;
    while (result == 0 && (step = sqlite3_step(entries)) == SQLITE_ROW) {
      if (commac_column_int(entries, 0, &value) < 0 || value < 0 ||
          value >= nummacros || (int)value < expected_set) { result = -1; break; }
      if ((int)value != expected_set) { expected_set = (int)value; expected_entry = 0; }
      macro = macros[value];
      if (commac_column_int(entries, 1, &value) < 0 || value != expected_entry++ ||
          commac_column_text(entries, 2, &alias, 4) < 0 ||
          commac_column_text(entries, 3, &expansion, LBUF_SIZE - 1) < 0) { result = -1; break; }
      macro->alias = realloc(macro->alias, (size_t)(macro->nummacros + 1) * 5);
      macro->string = realloc(macro->string,
                              sizeof(*macro->string) * (macro->nummacros + 1));
      if (!macro->alias || !macro->string) { result = -1; break; }
      StringCopy(macro->alias + macro->nummacros * 5, alias);
      macro->string[macro->nummacros++] = strdup(expansion);
      macro->maxmacros = macro->nummacros;
    }
    if (result == 0 && step != SQLITE_DONE) result = -1;
  } else if (result == 0) result = -1;
  sqlite3_finalize(entries);
  if (result == 0) {
    expected_set = 0;
    while (expected_set < nummacros) {
      if (!isPlayer(macros[expected_set]->player))
        clear_macro_set(expected_set);
      else
        expected_set++;
    }
  }
  return result;
}

/* SQLite is now authoritative for all commac, comsys, and macro state. */
static int commac_persistence_load(sqlite3 *sqlite) {
  if (!mudconf.have_comsys && !mudconf.have_macros)
    return 0;
  return commac_load_entries(sqlite) < 0 || commac_load_channels(sqlite) < 0 ||
                 commac_load_users(sqlite) < 0 || commac_load_messages(sqlite) < 0 ||
                 commac_load_macros(sqlite) < 0
             ? -1
             : 0;
}

int commac_persistence_register(void) {
  return persistence_register_sqlite_extension(
      "commac", commac_persistence_load, commac_persistence_store);
}
