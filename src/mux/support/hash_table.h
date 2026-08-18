/** @file
 * Structures and declarations needed for table hashing.
 */
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

/** Initializes hash table. @param[out] htab Htab. @param[in] size Storage size
 * in bytes. */

void hash_table_initialize(HashTable *htab, int size);
/** Destroys hash table. @param[in,out] htab Htab. */

void hash_table_destroy(HashTable *htab);

/** Resets hash table. @param[in,out] htab Htab. */

void hash_table_reset(HashTable *htab);

/** Executes hash value. @param[in] key Lookup key or command flags. @param[in]
 * mask Mask. */

int hash_value(char *key, int mask);
/** Returns hash mask. @param[in] table Table. */

int hash_mask_get(void *table);
/** Finds hash table find. @param[in] str String to process. @param[in] htab
 * Htab. */

void *hash_table_find(const char *str, HashTable *htab);
/** Finds hash table find const. @param[in] str String to process.
 * @param[in,out] htab Htab. */

const void *hash_table_find_const(const char *str, HashTable *htab);
/** Adds hash table. @param[in] str String to process. @param[in,out] hashdata
 * Hashdata. @param[in,out] htab Htab. */

int hash_table_add(const char *str, void *hashdata, HashTable *htab);
/** Adds const to hash table. @param[in] str String to process. @param[in]
 * hashdata Hashdata. @param[in,out] htab Htab. */

int hash_table_add_const(const char *str, const void *hashdata,
                         HashTable *htab);
/** Executes hash table delete. @param[in] str String to process. @param[in,out]
 * htab Htab. */

void hash_table_delete(const char *str, HashTable *htab);
/** Executes hash table flush. @param[in,out] htab Htab. @param[in] size Storage
 * size in bytes. */

void hash_table_flush(HashTable *htab, int size);
/** Executes hash table replace. @param[in,out] str String to process.
 * @param[in,out] hashdata Hashdata. @param[in,out] htab Htab. */

bool hash_table_replace(char *str, void *hashdata, HashTable *htab);
/** Executes hash table replace all. @param[in,out] old Old. @param[in,out] new
 * New. @param[in,out] htab Htab. */

void hash_table_replace_all(void *old, void *new, HashTable *htab);
/** Executes hash table next entry. @param[in,out] htab Htab. */

void *hash_table_next_entry(HashTable *htab);
/** Executes hash table first entry. @param[in,out] htab Htab. */

void *hash_table_first_entry(HashTable *htab);
/** Executes hash table first key. @param[in,out] htab Htab. */

char *hash_table_first_key(HashTable *htab);
/** Executes hash table next key. @param[in,out] htab Htab. */

char *hash_table_next_key(HashTable *htab);

/** Initializes numeric hash table. @param[out] htab Htab. @param[in] size
 * Storage size in bytes. */

void numeric_hash_table_initialize(HashTable *htab, int size);
/** Destroys numeric hash table. @param[in,out] htab Htab. */

void numeric_hash_table_destroy(HashTable *htab);
/** Resets numeric hash table. @param[in,out] htab Htab. */

void numeric_hash_table_reset(HashTable *htab);
/** Executes numeric hash table next entry. @param[in,out] htab Htab. */

void *numeric_hash_table_next_entry(HashTable *htab);
/** Executes numeric hash table first entry. @param[in,out] htab Htab. */

void *numeric_hash_table_first_entry(HashTable *htab);
/** Finds numeric hash table find. @param[in] val Val. @param[in] htab Htab. */

void *numeric_hash_table_find(long val, HashTable *htab);
/** Adds numeric hash table. @param[in] val Val. @param[in,out] hashdata
 * Hashdata. @param[in,out] htab Htab. */

int numeric_hash_table_add(long val, void *hashdata, HashTable *htab);
/** Executes numeric hash table delete. @param[in] val Val. @param[in,out] htab
 * Htab. */

void numeric_hash_table_delete(long val, HashTable *htab);
/** Executes numeric hash table flush. @param[in,out] htab Htab. @param[in] size
 * Storage size in bytes. */

void numeric_hash_table_flush(HashTable *htab, int size);
/** Executes numeric hash table replace. @param[in] val Val. @param[in,out]
 * hashdata Hashdata. @param[in,out] htab Htab. */

bool numeric_hash_table_replace(long val, void *hashdata, HashTable *htab);
