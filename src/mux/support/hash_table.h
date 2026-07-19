
/* hash_table.h - Structures and declarations needed for table hashing */

#pragma once

#include "mux/database/db.h"
#include "mux/server/platform.h"
#include "mux/support/name_table.h"
#include "mux/support/red_black_tree.h"

typedef struct ConfigurationContext ConfigurationContext;

int cf_ntab_access(void *value, char *string, long extra, DbRef player,
                   char *command, ConfigurationContext *context);

struct HashTable {
  long long checks, scans, max_scan, hits, entries, deletes, nulls;
  RedBlackTree tree;
  void *last;
};
typedef struct HashTable HashTable;

void hash_table_initialize(HashTable *, int);
void hash_table_destroy(HashTable *);

void hash_table_reset(HashTable *);

int hash_value(char *, int);
int hash_mask_get(void *);
void *hash_table_find(const char *, HashTable *);
int hash_table_add(const char *, void *, HashTable *);
void hash_table_delete(const char *, HashTable *);
void hash_table_flush(HashTable *, int);
int hash_table_replace(char *, void *, HashTable *);
void hash_table_replace_all(void *, void *, HashTable *);
char *hash_table_info(const char *, HashTable *);
typedef struct ServerConfiguration ServerConfiguration;

int name_table_search(GameDatabase *, const ServerConfiguration *, DbRef,
                      const NameTable *, char *);
NameTable *name_table_find_entry(GameDatabase *, const ServerConfiguration *,
                                 DbRef, NameTable *, char *);
void name_table_display(EvaluationContext *, const ServerConfiguration *, DbRef,
                        NameTable *, const char *, int);
void name_table_interpret(EvaluationContext *, const ServerConfiguration *,
                          DbRef, NameTable *, int, const char *, const char *,
                          const char *);
void name_table_list_set(EvaluationContext *, const ServerConfiguration *,
                         DbRef, NameTable *, int, const char *, int);
void *hash_table_next_entry(HashTable *htab);
void *hash_table_first_entry(HashTable *htab);
char *hash_table_first_key(HashTable *htab);
char *hash_table_next_key(HashTable *htab);

void numeric_hash_table_initialize(HashTable *, int);
void numeric_hash_table_destroy(HashTable *);
void numeric_hash_table_reset(HashTable *);
void *numeric_hash_table_next_entry(HashTable *htab);
void *numeric_hash_table_first_entry(HashTable *htab);
char *numeric_hash_table_info(const char *, HashTable *);
void *numeric_hash_table_find(long, HashTable *);
int numeric_hash_table_add(long, void *, HashTable *);
void numeric_hash_table_delete(long, HashTable *);
void numeric_hash_table_flush(HashTable *, int);
int numeric_hash_table_replace(long, void *, HashTable *);
extern NameTable powers_nametab[];
