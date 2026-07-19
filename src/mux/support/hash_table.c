/*
 * htab.c - table hashing routines
 */

#include "mux/server/platform.h"

#include "mux/database/db.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"

#include "mux/server/server_config.h"

struct string_dict_entry {
  char *key;
  void *data;
};

static int nuke_hash_ent(void *key, void *data, int depth, void *arg);

static int hrbtab_compare(char *left, char *right, void *arg) {
  return strcasecmp(left, right);
}

void hash_table_initialize(HashTable *htab, int size) {
  memset(htab, 0, sizeof(HashTable));
  htab->tree = red_black_tree_init(
      (int (*)(void *, void *, void *))(GenericFnPtr)hrbtab_compare, nullptr);
  htab->last = nullptr;
}

void hash_table_destroy(HashTable *htab) {
  if (htab == nullptr || htab->tree == nullptr)
    return;
  red_black_tree_walk(htab->tree, WALK_POSTORDER, nuke_hash_ent, nullptr);
  red_black_tree_destroy(htab->tree);
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
  struct string_dict_entry *ent;

  htab->checks++;
  /* red_black_tree's key parameter isn't const-correct; str is only used
     as a lookup key here, never mutated. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  ent = red_black_tree_find(htab->tree, (void *)str);
#pragma clang diagnostic pop
  if (ent) {
    return ent->data;
  } else
    return (void *)ent;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_add: Add a new entry to a hash table.
 */

int hash_table_add(const char *str, void *hashdata, HashTable *htab) {
  struct string_dict_entry *ent;

  /* red_black_tree's key parameter isn't const-correct; str is only used
     as a lookup key here, never mutated. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  if (red_black_tree_exists(htab->tree, (void *)str))
    return (-1);
#pragma clang diagnostic pop

  ent = malloc(sizeof(struct string_dict_entry));
  ent->key = strdup(str);
  ent->data = hashdata;

  red_black_tree_insert(htab->tree, ent->key, ent);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_delete: Remove an entry from a hash table.
 */

void hash_table_delete(const char *str, HashTable *htab) {
  struct string_dict_entry *ent = nullptr;

  /* red_black_tree's key parameter isn't const-correct; str is only used
     as a lookup key here, never mutated. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  if (!red_black_tree_exists(htab->tree, (void *)str)) {
    return;
  }
  ent = red_black_tree_delete(htab->tree, (void *)str);
#pragma clang diagnostic pop

  if (ent) {
    if (ent->key)
      free(ent->key);
    free(ent);
  }

  return;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_flush: free all the entries in a hashtable.
 */

static int nuke_hash_ent(void *key, void *data, int depth, void *arg) {
  struct string_dict_entry *ent = (struct string_dict_entry *)data;
  free(ent->key);
  free(ent);
  return 1;
}

void hash_table_flush(HashTable *htab, int size) {
  red_black_tree_walk(htab->tree, WALK_POSTORDER, nuke_hash_ent, nullptr);
  red_black_tree_destroy(htab->tree);
  htab->tree = red_black_tree_init(
      (int (*)(void *, void *, void *))(GenericFnPtr)hrbtab_compare, nullptr);
  if (htab->last)
    free(htab->last);
  htab->last = nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_replace: replace the data part of a hash entry.
 */

int hash_table_replace(char *str, void *hashdata, HashTable *htab) {
  struct string_dict_entry *ent;

  ent = red_black_tree_find(htab->tree, str);
  if (!ent)
    return 0;

  ent->data = hashdata;
  return 1;
}

struct hashreplstat {
  void *old;
  void *new;
};

static int hashreplall_cb(void *key, void *data, int depth, void *arg) {
  struct string_dict_entry *ent = (struct string_dict_entry *)data;
  struct hashreplstat *repl = (struct hashreplstat *)arg;

  if (ent->data == repl->old) {
    ent->data = repl->new;
  }
  return 1;
}

void hash_table_replace_all(void *old, void *new, HashTable *htab) {
  struct hashreplstat repl = {old, new};

  red_black_tree_walk(htab->tree, WALK_INORDER, hashreplall_cb, &repl);
}

/*
 * ---------------------------------------------------------------------------
 * * hash_table_info: return an mbuf with hashing stats
 */

char *hash_table_info(const char *tab_name, HashTable *htab) {
  char *buff;

  buff = alloc_mbuf("hash_table_info");
  snprintf(buff, MBUF_SIZE, "%-15s %8d", tab_name,
           red_black_tree_size(htab->tree));
  return buff;
}

/*
 * Returns the key for the first hash entry in 'htab'.
 */

void *hash_table_first_entry(HashTable *htab) {
  struct string_dict_entry *ent;

  if (htab->last)
    free(htab->last);

  ent = red_black_tree_search(htab->tree, SEARCH_FIRST, nullptr);
  if (ent) {
    htab->last = strdup(ent->key);
    return ent->data;
  }
  htab->last = nullptr;

  return nullptr;
}

void *hash_table_next_entry(HashTable *htab) {
  struct string_dict_entry *ent;

  if (!htab->last) {
    return hash_table_first_entry(htab);
  }

  ent = red_black_tree_search(htab->tree, SEARCH_GT, htab->last);
  free(htab->last);

  if (ent) {
    htab->last = strdup(ent->key);
    return ent->data;
  } else {
    htab->last = nullptr;
    return nullptr;
  }
}

char *hash_table_first_key(HashTable *htab) {
  struct string_dict_entry *ent;
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
  struct string_dict_entry *ent;

  if (!htab->last) {
    return hash_table_first_key(htab);
  }

  ent = red_black_tree_search(htab->tree, SEARCH_NEXT, htab->last);
  free(htab->last);

  if (ent) {
    htab->last = strdup(ent->key);
    return ent->key;
  } else {
    htab->last = nullptr;
    return nullptr;
  }
}
