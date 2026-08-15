/*
 * htab.c - table hashing routines
 */

#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/red_black_tree.h"

struct StringDictEntry {
  char *key;
  union {
    void *mutable_data;
    const void *const_data;
  } data;
  bool is_const;
};

static bool nuke_hash_ent(const RedBlackTreeVisitCall *call);

static int hrbtab_compare(const RedBlackTreeCompareCall *call) {
  const void *left = call->lhs;
  const void *right = call->rhs;
  return strcasecmp(left, right);
}

void hash_table_initialize(HashTable *htab, int size [[maybe_unused]]) {
  memset(htab, 0, sizeof(HashTable));
  htab->tree = red_black_tree_init(hrbtab_compare, nullptr);
  htab->last = nullptr;
}

void hash_table_destroy(HashTable *htab) {
  if (htab == nullptr || htab->tree == nullptr)
    return;
  red_black_tree_walk(htab->tree, WALK_POSTORDER, nuke_hash_ent, nullptr);
  red_black_tree_destroy(htab->tree);
  free(htab->last);
  memset(htab, 0, sizeof(*htab));
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_reset: Reset hash table stats.
 */

void hash_table_reset(HashTable *htab) {
  htab->checks = 0;
  htab->scans = 0;
  htab->hits = 0;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_find: Look up an entry in a hash table and return a pointer to
 * its
 * * hash data.
 */

void *hash_table_find(const char *str, HashTable *htab) {
  struct StringDictEntry *ent;

  htab->checks++;
  ent = red_black_tree_find(htab->tree, str);
  if (ent) {
    if (ent->is_const)
      abort();
    return ent->data.mutable_data;
  }
  return (void *)ent;
}

const void *hash_table_find_const(const char *str, HashTable *htab) {
  struct StringDictEntry *ent;

  htab->checks++;
  ent = red_black_tree_find(htab->tree, str);
  if (ent == nullptr)
    return nullptr;
  return ent->is_const ? ent->data.const_data : ent->data.mutable_data;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_add: Add a new entry to a hash table.
 */

int hash_table_add(const char *str, void *hashdata, HashTable *htab) {
  struct StringDictEntry *ent;

  if (red_black_tree_exists(htab->tree, str))
    return (-1);

  ent = checked_storage_allocate(sizeof(struct StringDictEntry));
  ent->key = strdup(str);
  ent->data.mutable_data = hashdata;
  ent->is_const = false;

  red_black_tree_insert(htab->tree, ent->key, ent);
  return 0;
}

int hash_table_add_const(const char *str, const void *hashdata,
                         HashTable *htab) {
  struct StringDictEntry *ent;

  if (red_black_tree_exists(htab->tree, str))
    return -1;

  ent = checked_storage_allocate(sizeof(struct StringDictEntry));
  ent->key = strdup(str);
  ent->data.const_data = hashdata;
  ent->is_const = true;
  red_black_tree_insert(htab->tree, ent->key, ent);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_delete: Remove an entry from a hash table.
 */

void hash_table_delete(const char *str, HashTable *htab) {
  struct StringDictEntry *ent = nullptr;

  if (!red_black_tree_exists(htab->tree, str)) {
    return;
  }
  ent = red_black_tree_delete(htab->tree, str);

  if (ent) {
    if (ent->key)
      free(ent->key);
    free(ent);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_flush: free all the entries in a hashtable.
 */

static bool nuke_hash_ent(const RedBlackTreeVisitCall *call) {
  void *data = call->data;
  struct StringDictEntry *ent = (struct StringDictEntry *)data;
  free(ent->key);
  free(ent);
  return true;
}

void hash_table_flush(HashTable *htab, int size [[maybe_unused]]) {
  red_black_tree_walk(htab->tree, WALK_POSTORDER, nuke_hash_ent, nullptr);
  red_black_tree_destroy(htab->tree);
  htab->tree = red_black_tree_init(hrbtab_compare, nullptr);
  if (htab->last)
    free(htab->last);
  htab->last = nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_replace: replace the data part of a hash entry.
 */

bool hash_table_replace(char *str, void *hashdata, HashTable *htab) {
  struct StringDictEntry *ent;

  ent = red_black_tree_find(htab->tree, str);
  if (!ent)
    return false;
  if (ent->is_const)
    abort();

  ent->data.mutable_data = hashdata;
  ent->is_const = false;
  return true;
}

struct Hashreplstat {
  void *old;
  void *new;
};

static bool hashreplall_cb(const RedBlackTreeVisitCall *call) {
  void *data = call->data;
  void *arg = call->context;
  struct StringDictEntry *ent = (struct StringDictEntry *)data;
  struct Hashreplstat *repl = (struct Hashreplstat *)arg;

  if (ent->is_const)
    return true;
  if (ent->data.mutable_data == repl->old) {
    ent->data.mutable_data = repl->new;
    ent->is_const = false;
  }
  return true;
}

void hash_table_replace_all(void *old, void *new, HashTable *htab) {
  struct Hashreplstat repl = {old, new};

  red_black_tree_walk(htab->tree, WALK_INORDER, hashreplall_cb, &repl);
}

/*
 * Returns the key for the first hash entry in 'htab'.
 */

void *hash_table_first_entry(HashTable *htab) {
  struct StringDictEntry *ent;

  if (htab->last)
    free(htab->last);

  ent = red_black_tree_search(htab->tree, SEARCH_FIRST, nullptr);
  if (ent) {
    if (ent->is_const)
      abort();
    htab->last = strdup(ent->key);
    return ent->data.mutable_data;
  }
  htab->last = nullptr;

  return nullptr;
}

void *hash_table_next_entry(HashTable *htab) {
  struct StringDictEntry *ent;

  if (!htab->last) {
    return hash_table_first_entry(htab);
  }

  ent = red_black_tree_search(htab->tree, SEARCH_GT, htab->last);
  free(htab->last);

  if (ent) {
    if (ent->is_const)
      abort();
    htab->last = strdup(ent->key);
    return ent->data.mutable_data;
  }
  htab->last = nullptr;
  return nullptr;
}

char *hash_table_first_key(HashTable *htab) {
  struct StringDictEntry *ent;
  if (htab->last)
    free(htab->last);

  ent = red_black_tree_search(htab->tree, SEARCH_FIRST, nullptr);
  if (ent) {
    htab->last = strdup(ent->key);
    return ent->key;
  }
  htab->last = nullptr;

  return nullptr;
}

char *hash_table_next_key(HashTable *htab) {
  struct StringDictEntry *ent;

  if (!htab->last) {
    return hash_table_first_key(htab);
  }

  ent = red_black_tree_search(htab->tree, SEARCH_NEXT, htab->last);
  free(htab->last);

  if (ent) {
    htab->last = strdup(ent->key);
    return ent->key;
  }
  htab->last = nullptr;
  return nullptr;
}
