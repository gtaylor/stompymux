/*
 * htab.c - table hashing routines
 */

#include <ctype.h>
#include <string.h>

#include "mux/commands/command.h"
#include "mux/objects/flags.h"
#include "mux/server/configuration.h"
#include "mux/server/configuration_interpreter.h"
#include "mux/server/game.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/name_table.h"
#include "mux/support/stringutil.h"

constexpr size_t NAME_TABLE_MAX_ENTRIES = 1024;

static bool name_table_is_sentinel(const void *element) {
  return ((const NameTable *)element)->name == nullptr;
}

static size_t name_table_count(const NameTable *table) {
  return checked_storage_sentinel_count(
      table, sizeof(*table), NAME_TABLE_MAX_ENTRIES, name_table_is_sentinel);
}

static NameTable *name_table_entry_at(NameTable *table, size_t count,
                                      size_t index) {
  return checked_storage_at(table, count, sizeof(*table), index);
}

static const NameTable *name_table_entry_at_const(const NameTable *table,
                                                  size_t count, size_t index) {
  return checked_storage_at_const(table, count, sizeof(*table), index);
}

/*
 * ---------------------------------------------------------------------------
 * * name_table_search: Search a name table for a match and return the flag
 * value.
 */
int name_table_search(GameDatabase *database,
                      const ServerConfiguration *configuration, DbRef player,
                      const NameTable *ntab, char *flagname) {
  size_t count = name_table_count(ntab);
  for (size_t index = 0; index < count; index++) {
    const NameTable *nt = name_table_entry_at_const(ntab, count, index);
    if (minmatch(flagname, nt->name, nt->minlen)) {
      if (check_access(database, configuration, player, nt->perm)) {
        return nt->flag;
      } else
        return -2;
    }
  }
  return -1;
}

/*
 * ---------------------------------------------------------------------------
 * * name_table_find_entry: Search a name table for a match and return a pointer
 * to it.
 */

NameTable *name_table_find_entry(GameDatabase *database,
                                 const ServerConfiguration *configuration,
                                 DbRef player, NameTable *ntab,
                                 char *flagname) {
  size_t count = name_table_count(ntab);
  for (size_t index = 0; index < count; index++) {
    NameTable *nt = name_table_entry_at(ntab, count, index);
    if (minmatch(flagname, nt->name, nt->minlen)) {
      if (check_access(database, configuration, player, nt->perm)) {
        return nt;
      }
    }
  }
  return nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * name_table_display: Print out the names of the entries in a name table.
 */

void name_table_display(EvaluationContext *evaluation,
                        const ServerConfiguration *configuration, DbRef player,
                        NameTable *ntab, const char *prefix, int list_if_none) {
  char *buf, *bp;
  int got_one;

  buf = alloc_lbuf("name_table_display");
  bp = buf;
  got_one = 0;
  safe_str(prefix, buf, &bp);
  size_t count = name_table_count(ntab);
  for (size_t index = 0; index < count; index++) {
    NameTable *nt = name_table_entry_at(ntab, count, index);
    if (is_god(evaluation->world->database, player) ||
        check_access(evaluation->world->database, configuration, player,
                     nt->perm)) {
      safe_chr(' ', buf, &bp);
      safe_str(nt->name, buf, &bp);
      got_one = 1;
    }
  }
  *bp = '\0';
  if (got_one || list_if_none)
    notify_checked(evaluation, player, player, buf, MSG_ME_ALL | MSG_F_DOWN);
  free_lbuf(buf);
}

/*
 * ---------------------------------------------------------------------------
 * * name_table_interpret: Print values for flags defined in name table.
 */

void name_table_interpret(const NameTableInterpretRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  const ServerConfiguration *configuration = request->configuration;
  DbRef player = request->player;
  char *buf, *bp;

  buf = alloc_lbuf("name_table_interpret");
  bp = buf;
  safe_str(request->prefix, buf, &bp);
  size_t count = name_table_count(request->table);
  for (size_t index = 0; index < count; index++) {
    NameTable *nt = name_table_entry_at(request->table, count, index);
    if (is_god(evaluation->world->database, player) ||
        check_access(evaluation->world->database, configuration, player,
                     nt->perm)) {
      safe_chr(' ', buf, &bp);
      safe_str(nt->name, buf, &bp);
      safe_str("...", buf, &bp);
      const char *text;
      if ((request->flags & nt->flag) != 0)
        text = request->true_text;
      else
        text = request->false_text;
      safe_str(text, buf, &bp);
      if (index + 1 < count)
        safe_chr(';', buf, &bp);
    }
  }
  *bp = '\0';
  notify_checked(evaluation, player, player, buf, MSG_ME_ALL | MSG_F_DOWN);
  free_lbuf(buf);
}

/*
 * ---------------------------------------------------------------------------
 * * name_table_list_set: Print values for flags defined in name table.
 */

void name_table_list_set(EvaluationContext *evaluation,
                         const ServerConfiguration *configuration, DbRef player,
                         NameTable *ntab, int flagword, const char *prefix,
                         int list_if_none) {
  char *buf, *bp;
  int got_one;

  buf = bp = alloc_lbuf("name_table_list_set");
  safe_str(prefix, buf, &bp);
  size_t count = name_table_count(ntab);
  got_one = 0;
  for (size_t index = 0; index < count; index++) {
    NameTable *nt = name_table_entry_at(ntab, count, index);
    if (((flagword & nt->flag) != 0) &&
        (is_god(evaluation->world->database, player) ||
         check_access(evaluation->world->database, configuration, player,
                      nt->perm))) {
      safe_chr(' ', buf, &bp);
      safe_str(nt->name, buf, &bp);
      got_one = 1;
    }
  }
  *bp = '\0';
  if (got_one || list_if_none)
    notify_checked(evaluation, player, player, buf, MSG_ME_ALL | MSG_F_DOWN);
  free_lbuf(buf);
}

int cf_ntab_access(const ConfigurationCall *call) {
  char *str = call->text;
  ConfigurationContext *context = call->context;
  char *ap;
  size_t length = strlen(str);
  size_t offset = 0;

  while (offset < length &&
         !(isspace)(*(const unsigned char *)checked_storage_at_const(
             str, length, sizeof(char), offset)))
    offset++;
  if (offset < length) {
    *(char *)checked_storage_at(str, length + 1, sizeof(char), offset) = '\0';
    offset++;
  }
  while (offset < length &&
         (isspace)(*(const unsigned char *)checked_storage_at_const(
             str, length, sizeof(char), offset)))
    offset++;
  ap = checked_storage_at(str, length + 1, sizeof(char), offset);
  NameTable *table = call->value;
  size_t count = name_table_count(table);
  for (size_t index = 0; index < count; index++) {
    NameTable *np = name_table_entry_at(table, count, index);
    if (minmatch(str, np->name, np->minlen)) {
      ConfigurationCall modify_call = *call;
      modify_call.value = &np->perm;
      modify_call.text = ap;
      return configuration_modify_bits(&modify_call);
    }
  }
  configuration_log_not_found(context, call->player, call->command, "Entry",
                              str);
  return -1;
}
