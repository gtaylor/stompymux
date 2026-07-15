/*
 * vattr.c -- Manages the user-defined attributes.
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/vattr.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"

static void fixcase(char *);
static char *store_string(char *);

/*
 * Allocate space for strings in lumps this big.
 */

#define STRINGBLOCK 1000

/*
 * Current block we're putting stuff in
 */

static char *stringblock = (char *)0;

/*
 * High water mark.
 */

static int stringblock_hwm = 0;

void vattr_init(void) {
  hash_table_initialize(&mudstate.vattr_name_htab, 65536);
}

VATTR *vattr_find(char *name) {
  register VATTR *vp;

  if (!ok_attr_name(name))
    return (nullptr);

  vp = (VATTR *)hash_table_find(name, &mudstate.vattr_name_htab);

  /*
   * vp is NULL or the right thing. It's right, either way.
   */
  return (vp);
}

VATTR *vattr_alloc(char *name, int flags) {
  int number;

  if (((number = mudstate.attr_next++) & 0x7f) == 0)
    number = mudstate.attr_next++;
  anum_extend(number);
  return (vattr_define(name, number, flags));
}

VATTR *vattr_define(char *name, int number, int flags) {
  VATTR *vp;

  /*
   * Be ruthless.
   */

  if (strlen(name) > VNAME_SIZE)
    name[VNAME_SIZE - 1] = '\0';

  fixcase(name);
  if (!ok_attr_name(name))
    return (nullptr);

  if ((vp = vattr_find(name)) != nullptr)
    return (vp);

  vp = malloc(sizeof(VATTR));

  vp->name = store_string(name);
  vp->flags = flags;
  vp->number = number;

  hash_table_add(vp->name, (int *)vp, &mudstate.vattr_name_htab);

  anum_extend(vp->number);
  anum_set(vp->number, (Attribute *)vp);
  return (vp);
}

void do_dbclean(DbRef player, DbRef cause, int key) {
  VATTR *vp;
  DbRef i;
  int notfree;
  int count = 0;

  for (vp = (VATTR *)hash_table_first_entry(&mudstate.vattr_name_htab);
       vp != nullptr;
       vp = (VATTR *)hash_table_next_entry(&mudstate.vattr_name_htab)) {
    notfree = 0;

    DO_WHOLE_DB(i) {
      if (attribute_get_raw(i, vp->number) != nullptr) {
        notfree = 1;
        break;
      }
    }

    if (!notfree) {
      anum_set(vp->number, nullptr);
      hash_table_delete(vp->name, &mudstate.vattr_name_htab);
      free((char *)vp);
      count++;
    }
  }
  notify_printf(player, "Database cleared of %d stale attribute entries.",
                count);
}

void vattr_delete(char *name) {
  VATTR *vp;
  int number;

  fixcase(name);
  if (!ok_attr_name(name))
    return;

  number = 0;

  vp = (VATTR *)hash_table_find(name, &mudstate.vattr_name_htab);

  if (vp) {
    number = vp->number;
    anum_set(number, nullptr);
    hash_table_delete(name, &mudstate.vattr_name_htab);
    free((char *)vp);
  }

  return;
}

VATTR *vattr_rename(char *name, char *newname) {
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

  vp = (VATTR *)hash_table_find(name, &mudstate.vattr_name_htab);

  if (vp)
    vp->name = store_string(newname);

  return (vp);
}

VATTR *vattr_first(void) {
  return (VATTR *)hash_table_first_entry(&mudstate.vattr_name_htab);
}

VATTR *vattr_next(VATTR *vp) {
  if (vp == nullptr)
    return (vattr_first());

  return ((VATTR *)hash_table_next_entry(&mudstate.vattr_name_htab));
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
static char *store_string(char *str) {
  int len;
  char *ret;

  len = strlen(str);

  /*
   * If we have no block, or there's not enough room left in the * * *
   * current one, get a new one.
   */

  if (!stringblock || (STRINGBLOCK - stringblock_hwm) < (len + 1)) {
    stringblock = malloc(STRINGBLOCK);
    if (!stringblock)
      return ((char *)0);
    stringblock_hwm = 0;
  }
  ret = stringblock + stringblock_hwm;
  StringCopy(ret, str);
  stringblock_hwm += (len + 1);
  return (ret);
}
