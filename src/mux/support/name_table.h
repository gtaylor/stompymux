/* name_table.h - Name-to-value table entry type. */

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

int name_table_search(GameDatabase *database,
                      const ServerConfiguration *configuration, DbRef player,
                      const NameTable *ntab, char *flagname);
NameTable *name_table_find_entry(GameDatabase *database,
                                 const ServerConfiguration *configuration,
                                 DbRef player, NameTable *ntab, char *flagname);
NameTable *name_table_find_match(NameTable *table, const char *name);
void name_table_display(EvaluationContext *evaluation,
                        const ServerConfiguration *configuration, DbRef player,
                        const NameTable *ntab, const char *prefix,
                        int list_if_none);
void name_table_interpret(const NameTableInterpretRequest *request);
void name_table_list_set(EvaluationContext *evaluation,
                         const ServerConfiguration *configuration, DbRef player,
                         const NameTable *ntab, int flagword,
                         const char *prefix, int list_if_none);
