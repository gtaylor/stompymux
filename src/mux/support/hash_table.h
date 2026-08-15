
/* hash_table.h - Structures and declarations needed for table hashing */

#pragma once

#include "mux/commands/command_runtime.h"
#include "mux/objects/db.h"
#include "mux/server/configuration_interpreter.h"
#include "mux/server/platform.h"
#include "mux/support/name_table.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"

struct HashTable {
  long long checks, scans, max_scan, hits, entries, deletes, nulls;
  RedBlackTree tree;
  void *last;
};
typedef struct HashTable HashTable;

void hash_table_initialize(HashTable * /*htab*/, int /*size*/);
void hash_table_destroy(HashTable * /*htab*/);

void hash_table_reset(HashTable * /*htab*/);

int hash_value(char *, int);
int hash_mask_get(void *);
void *hash_table_find(const char * /*str*/, HashTable * /*htab*/);
const void *hash_table_find_const(const char * /*str*/, HashTable * /*htab*/);
int hash_table_add(const char * /*str*/, void * /*hashdata*/,
                   HashTable * /*htab*/);
int hash_table_add_const(const char * /*str*/, const void * /*hashdata*/,
                         HashTable * /*htab*/);
void hash_table_delete(const char * /*str*/, HashTable * /*htab*/);
void hash_table_flush(HashTable * /*htab*/, int /*size*/);
int hash_table_replace(char * /*str*/, void * /*hashdata*/,
                       HashTable * /*htab*/);
void hash_table_replace_all(void * /*old*/, void * /*new*/,
                            HashTable * /*htab*/);
void *hash_table_next_entry(HashTable *htab);
void *hash_table_first_entry(HashTable *htab);
char *hash_table_first_key(HashTable *htab);
char *hash_table_next_key(HashTable *htab);

void numeric_hash_table_initialize(HashTable * /*htab*/, int /*size*/);
void numeric_hash_table_destroy(HashTable * /*htab*/);
void numeric_hash_table_reset(HashTable * /*htab*/);
void *numeric_hash_table_next_entry(HashTable *htab);
void *numeric_hash_table_first_entry(HashTable *htab);
void *numeric_hash_table_find(long /*val*/, HashTable * /*htab*/);
int numeric_hash_table_add(long /*val*/, void * /*hashdata*/,
                           HashTable * /*htab*/);
void numeric_hash_table_delete(long /*val*/, HashTable * /*htab*/);
void numeric_hash_table_flush(HashTable * /*htab*/, int /*size*/);
int numeric_hash_table_replace(long /*val*/, void * /*hashdata*/,
                               HashTable * /*htab*/);
