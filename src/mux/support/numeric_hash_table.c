/* numeric_hash_table.c - Numeric-keyed hash-table operations. */

#include "mux/database/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"

static int nhrbtab_compare(int left, int right, void *arg) {
  return (right - left);
}

void numeric_hash_table_initialize(HashTable *htab, int size) {
  memset(htab, 0, sizeof(HashTable));
  htab->tree = red_black_tree_init((void *)nhrbtab_compare, nullptr);
  htab->last = nullptr;
}

void numeric_hash_table_reset(HashTable *htab) {
  htab->checks = 0;
  htab->scans = 0;
  htab->hits = 0;
};

/*
 * ---------------------------------------------------------------------------
 * * hash_table_find: Look up an entry in a hash table and return a pointer to
 * its
 * * hash data.
 */

void *numeric_hash_table_find(long val, HashTable *htab) {
  htab->checks++;
  return red_black_tree_find(htab->tree, (void *)val);
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_add: Add a new entry to a hash table.
 */

int numeric_hash_table_add(long val, void *hashdata, HashTable *htab) {
  if (red_black_tree_exists(htab->tree, (void *)val))
    return (-1);
  red_black_tree_insert(htab->tree, (void *)val, hashdata);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_delete: Remove an entry from a hash table.
 */

void numeric_hash_table_delete(long val, HashTable *htab) {
  red_black_tree_delete(htab->tree, (void *)val);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_flush: free all the entries in a hashtable.
 */

void numeric_hash_table_flush(HashTable *htab, int size) {
  red_black_tree_destroy(htab->tree);
  htab->tree = red_black_tree_init((void *)nhrbtab_compare, nullptr);
  htab->last = nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_replace: replace the data part of a hash entry.
 */

int numeric_hash_table_replace(long val, void *hashdata, HashTable *htab) {

  red_black_tree_insert(htab->tree, (void *)val, hashdata);
  return 1;
}
