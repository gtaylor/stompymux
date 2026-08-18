/** @file
 * Name-to-value table entry type.
 */
#pragma once

#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct NameTable {
  const char *name;
  int minlen;
  int perm;
  int flag;
} NameTable;

typedef struct NameTableInterpretRequest {
  EvaluationContext *evaluation;
  const ServerConfiguration *configuration;
  DbRef player;
  const NameTable *table;
  int flags;
  const char *prefix;
  const char *true_text;
  const char *false_text;
} NameTableInterpretRequest;

/** Executes name table search. @param[in,out] database Game database.
 * @param[in] configuration Server configuration. @param[in] player Player
 * object. @param[in] ntab Ntab. @param[in,out] flagname Flagname. */

int name_table_search(GameDatabase *database,
                      const ServerConfiguration *configuration, DbRef player,
                      const NameTable *ntab, char *flagname);
/** Finds name table find entry. @param[in,out] database Game database.
 * @param[in] configuration Server configuration. @param[in] player Player
 * object. @param[in,out] ntab Ntab. @param[in,out] flagname Flagname. */

NameTable *name_table_find_entry(GameDatabase *database,
                                 const ServerConfiguration *configuration,
                                 DbRef player, NameTable *ntab, char *flagname);
/** Finds name table find match. @param[in,out] table Table. @param[in] name
 * Name to use. */

NameTable *name_table_find_match(NameTable *table, const char *name);
/** Executes name table display. @param[in,out] evaluation Expression evaluation
 * context. @param[in] configuration Server configuration. @param[in] player
 * Player object. @param[in] ntab Ntab. @param[in] prefix Prefix. @param[in]
 * list_if_none List if none. */

void name_table_display(EvaluationContext *evaluation,
                        const ServerConfiguration *configuration, DbRef player,
                        const NameTable *ntab, const char *prefix,
                        int list_if_none);
/** Executes name table interpret. @param[in] request Request. */

void name_table_interpret(const NameTableInterpretRequest *request);
/** Sets name table list. @param[in,out] evaluation Expression evaluation
 * context. @param[in] configuration Server configuration. @param[in] player
 * Player object. @param[in] ntab Ntab. @param[in] flagword Flagword. @param[in]
 * prefix Prefix. @param[in] list_if_none List if none. */

void name_table_list_set(EvaluationContext *evaluation,
                         const ServerConfiguration *configuration, DbRef player,
                         const NameTable *ntab, int flagword,
                         const char *prefix, int list_if_none);
