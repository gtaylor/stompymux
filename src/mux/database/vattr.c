/*
 * vattr.c -- Manages the user-defined attributes.
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/vattr.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"

static void fixcase(char *);
typedef struct VattrStringBlock VattrStringBlock;
struct VattrStringBlock {
  VattrStringBlock *next;
  int used;
  char data[1000];
};

struct VattrStore {
  GameDatabase *database;
  HashTable names;
  VattrStringBlock *blocks;
  int next_number;
};

static char *store_string(VattrStore *store, char *string);

/*
 * Allocate space for strings in lumps this big.
 */

constexpr int STRINGBLOCK = 1000;

/*
 * Current block we're putting stuff in
 */

VattrStore *vattr_store_create(GameDatabase *database) {
  VattrStore *store = calloc(1, sizeof(*store));

  if (store == nullptr)
    return nullptr;
  store->database = database;
  hash_table_initialize(&store->names, 65536);
  store->next_number = A_USER_START;
  return store;
}

void vattr_store_destroy(VattrStore *store) {
  if (store == nullptr)
    return;
  for (VATTR *attribute = hash_table_first_entry(&store->names);
       attribute != nullptr; attribute = hash_table_next_entry(&store->names))
    free(attribute);
  hash_table_destroy(&store->names);
  while (store->blocks != nullptr) {
    VattrStringBlock *next = store->blocks->next;

    free(store->blocks);
    store->blocks = next;
  }
  free(store);
}

int vattr_store_next_number(const VattrStore *store) {
  return store->next_number;
}

void vattr_store_set_next_number(VattrStore *store, int number) {
  store->next_number = number;
}

VATTR *vattr_find(VattrStore *store, char *name) {
  register VATTR *vp;

  if (!ok_attr_name(name))
    return (nullptr);

  vp = (VATTR *)hash_table_find(name, &store->names);

  /*
   * vp is NULL or the right thing. It's right, either way.
   */
  return (vp);
}

VATTR *vattr_alloc(VattrStore *store, char *name, int flags) {
  int number;

  if (((number = store->next_number++) & 0x7f) == 0)
    number = store->next_number++;
  anum_extend(store->database, number);
  return (vattr_define(store, name, number, flags));
}

VATTR *vattr_define(VattrStore *store, char *name, int number, int flags) {
  VATTR *vp;

  /*
   * Be ruthless.
   */

  if (strlen(name) > VNAME_SIZE)
    name[VNAME_SIZE - 1] = '\0';

  fixcase(name);
  if (!ok_attr_name(name))
    return (nullptr);

  if ((vp = vattr_find(store, name)) != nullptr)
    return (vp);

  vp = malloc(sizeof(VATTR));

  vp->name = store_string(store, name);
  vp->flags = flags;
  vp->number = number;

  hash_table_add(vp->name, (int *)vp, &store->names);

  anum_extend(store->database, vp->number);
  anum_set(store->database, vp->number, (Attribute *)vp);
  return (vp);
}

void do_dbclean(CommandInvocation *invocation) {
  VattrStore *store = invocation->context->runtime->vattrs;
  VATTR *vp;
  DbRef i;
  int notfree;
  int count = 0;

  for (vp = (VATTR *)hash_table_first_entry(&store->names); vp != nullptr;
       vp = (VATTR *)hash_table_next_entry(&store->names)) {
    notfree = 0;

    DO_WHOLE_DB(invocation->context->world->database, i) {
      if (attribute_get_raw(store->database, i, vp->number) != nullptr) {
        notfree = 1;
        break;
      }
    }

    if (!notfree) {
      anum_set(store->database, vp->number, nullptr);
      hash_table_delete(vp->name, &store->names);
      free((char *)vp);
      count++;
    }
  }
  notify_printf(&invocation->context->evaluation, invocation->player,
                "Database cleared of %d stale attribute entries.", count);
}

void vattr_delete(VattrStore *store, char *name) {
  VATTR *vp;
  int number;

  fixcase(name);
  if (!ok_attr_name(name))
    return;

  number = 0;

  vp = (VATTR *)hash_table_find(name, &store->names);

  if (vp) {
    number = vp->number;
    anum_set(store->database, number, nullptr);
    hash_table_delete(name, &store->names);
    free((char *)vp);
  }

  return;
}

VATTR *vattr_rename(VattrStore *store, char *name, char *newname) {
  VATTR *vp;

  fixcase(name);
  if (!ok_attr_name(name))
    return (nullptr);

  /*
   * Be ruthless.
   */

  if (strlen(newname) > VNAME_SIZE)
    newname[VNAME_SIZE - 1] = '\0';

  fixcase(newname);
  if (!ok_attr_name(newname))
    return (nullptr);

  vp = (VATTR *)hash_table_find(name, &store->names);

  if (vp)
    vp->name = store_string(store, newname);

  return (vp);
}

VATTR *vattr_first(VattrStore *store) {
  return (VATTR *)hash_table_first_entry(&store->names);
}

VATTR *vattr_next(VattrStore *store, VATTR *vp) {
  if (vp == nullptr)
    return (vattr_first(store));

  return ((VATTR *)hash_table_next_entry(&store->names));
}

static void fixcase(char *name) {
  char *cp = name;

  while (*cp) {
    *cp = ToUpper(*cp);
    cp++;
  }

  return;
}

/**
 * Some goop for efficiently storing strings we expect to
 * keep forever. There is no freeing mechanism.
 */
static char *store_string(VattrStore *store, char *str) {
  int len;
  char *ret;

  len = (int)strlen(str);

  /*
   * If we have no block, or there's not enough room left in the * * *
   * current one, get a new one.
   */

  if (store->blocks == nullptr ||
      (STRINGBLOCK - store->blocks->used) < (len + 1)) {
    VattrStringBlock *block = calloc(1, sizeof(*block));

    if (block == nullptr)
      return ((char *)0);
    block->next = store->blocks;
    store->blocks = block;
  }
  ret = store->blocks->data + store->blocks->used;
  StringCopy(ret, str);
  store->blocks->used += (len + 1);
  return (ret);
}
