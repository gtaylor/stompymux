/* db.c - In-memory game-object and attribute database operations. */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include <assert.h>
#include <sys/file.h>
#include <sys/stat.h>

#include "mux/commands/command.h"
#include "mux/commands/macro.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/database/vattr.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "mux/world/match.h"
#include "mux/world/player_cache.h"
#include "mux/world/world_context.h"

#ifndef O_ACCMODE
#define O_ACCMODE (O_RDONLY | O_WRONLY | O_RDWR)
#endif

/*
 * Restart definitions
 */

void game_database_initialize(GameDatabase *database) {
  memset(database, 0, sizeof(*database));
  database->freelist = NOTHING;
}

void game_database_bind_services(GameDatabase *database,
                                 ServerConfiguration *configuration,
                                 WorldIndexes *indexes,
                                 DescriptorRegistry *descriptors,
                                 PlayerCache *players, VattrStore *vattrs,
                                 ServerLog *log) {
  database->configuration = configuration;
  database->indexes = indexes;
  database->descriptors = descriptors;
  database->players = players;
  database->vattrs = vattrs;
  database->log = log;
}

void game_database_destroy(GameDatabase *database) {
  if (database == nullptr)
    return;
  db_free(database);
  free(database->attributes_by_number);
  database->attributes_by_number = nullptr;
  database->attribute_capacity = 0;
}

typedef struct atrcount ATRCOUNT;
struct atrcount {
  GameDatabase *database;
  DbRef thing;
  int count;
};

/*
 * #define GNU_MALLOC_TEST 1
 */

#ifdef GNU_MALLOC_TEST
extern unsigned int malloc_sbrk_used; /* Amount of data space used now */
#endif

/*
 * Check routine forward declaration.
 */
extern int fwdlist_ck(EvaluationContext *, int, DbRef, DbRef, int, char *);

// Flags for character stats/skills attributes.
constexpr int PLSTAT_MODE = AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL;

/*
 * list of attributes
 */
Attribute attr_table[] = {
    {"Alias", A_ALIAS, AF_NOPROG | AF_NOCMD | AF_GOD, nullptr},
    {"Away", A_AWAY, AF_ODARK | AF_NOPROG, nullptr},
    {"Buildcoord", A_BUILDCOORD, AF_MDARK | AF_WIZARD, nullptr},
    {"Buildentrance", A_BUILDENTRANCE, AF_MDARK | AF_WIZARD, nullptr},
    {"Buildlinks", A_BUILDLINKS, AF_MDARK | AF_WIZARD, nullptr},
    {"Comment", A_COMMENT, AF_MDARK | AF_WIZARD, nullptr},
    {"Contactoptions", A_CONTACTOPT, AF_ODARK, nullptr},
    {"Desc", A_DESC, AF_NOPROG, nullptr},
    {"Destroyer", A_DESTROYER, AF_MDARK | AF_WIZARD | AF_NOPROG, nullptr},
    {"Ealias", A_EALIAS, AF_ODARK | AF_NOPROG, nullptr},
    {"Faction", A_FACTION, AF_MDARK | AF_WIZARD, nullptr},
    {"Filter", A_FILTER, AF_ODARK | AF_NOPROG, nullptr},
    {"Forwardlist", A_FORWARDLIST, AF_ODARK | AF_NOPROG, fwdlist_ck},
    {"Idesc", A_IDESC, AF_ODARK | AF_NOPROG, nullptr},
    {"Idle", A_IDLE, AF_ODARK | AF_NOPROG, nullptr},
    {"Infilter", A_INFILTER, AF_ODARK | AF_NOPROG, nullptr},
    {"Inprefix", A_INPREFIX, AF_ODARK | AF_NOPROG, nullptr},
    {"Job", A_JOB, AF_MDARK | AF_WIZARD, nullptr},
    {"Lalias", A_LALIAS, AF_ODARK | AF_NOPROG, nullptr},
    {"Last", A_LAST, AF_WIZARD | AF_NOCMD | AF_NOPROG, nullptr},
    {"Lastname", A_LASTNAME, AF_WIZARD | AF_NOPROG | AF_MDARK, nullptr},
    {"Luaparent", A_LUAPARENT, AF_WIZARD | AF_MDARK | AF_NOCMD | AF_NOPROG,
     nullptr},
    {"Lastpage", A_LASTPAGE,
     AF_INTERNAL | AF_NOCMD | AF_NOPROG | AF_GOD | AF_PRIVATE, nullptr},
    {"Lastsite", A_LASTSITE, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_GOD, nullptr},
    {"Listen", A_LISTEN, AF_ODARK, nullptr},

    {"Logindata", A_LOGINDATA, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL,
     nullptr},
    {"LRSheight", A_LRSHEIGHT, AF_ODARK, nullptr},
    {"Unused1", A_UNUSED1, AF_WIZARD | AF_MDARK, nullptr},
    {"Mapcolor", A_MAPCOLOR, AF_ODARK, nullptr},
    {"Mapvis", A_MAPVIS, AF_MDARK | AF_WIZARD, nullptr},
    {"Mechdesc", A_MECHDESC, AF_MDARK, nullptr},
    {"Mechname", A_MECHNAME, AF_MDARK, nullptr},
    {"Mechstatus", A_MECHSTATUS, AF_MDARK | AF_WIZARD, nullptr},
    {"Mechtype", A_MECHTYPE, AF_MDARK, nullptr},
    {"MechPrefID", A_MECHPREFID, AF_MDARK | AF_WIZARD, nullptr},
    {"Mechskills", A_MECHSKILLS, AF_MDARK, nullptr},
    {"Mwtemplate", A_MWTEMPLATE, AF_MDARK, nullptr},
    {"Name", A_NAME, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL, nullptr},
    {"PCequip", A_PCEQUIP, AF_MDARK, nullptr},
    {"Pilot", A_PILOTNUM, AF_MDARK, nullptr},
    {"Prefix", A_PREFIX, AF_ODARK | AF_NOPROG, nullptr},
    {"QueueMax", A_QUEUEMAX, AF_MDARK | AF_WIZARD | AF_NOPROG, nullptr},
    {"Ranknum", A_RANKNUM, AF_MDARK | AF_WIZARD, nullptr},
    {"Reject", A_REJECT, AF_ODARK | AF_NOPROG, nullptr},

    {"Semaphore", A_SEMAPHORE, AF_ODARK | AF_NOPROG | AF_WIZARD | AF_NOCMD,
     nullptr},
    {"Tacsize", A_TACSIZE, AF_ODARK, nullptr},
    {"Timeout", A_TIMEOUT, AF_MDARK | AF_NOPROG | AF_WIZARD, nullptr},
    {"Tz", A_TZ, AF_NOPROG, nullptr},
    {"VA", A_VA, AF_ODARK, nullptr},
    {"VB", A_VA + 1, AF_ODARK, nullptr},
    {"VC", A_VA + 2, AF_ODARK, nullptr},
    {"VD", A_VA + 3, AF_ODARK, nullptr},
    {"VE", A_VA + 4, AF_ODARK, nullptr},
    {"VF", A_VA + 5, AF_ODARK, nullptr},
    {"VG", A_VA + 6, AF_ODARK, nullptr},
    {"VH", A_VA + 7, AF_ODARK, nullptr},
    {"VI", A_VA + 8, AF_ODARK, nullptr},
    {"VJ", A_VA + 9, AF_ODARK, nullptr},
    {"VK", A_VA + 10, AF_ODARK, nullptr},
    {"VL", A_VA + 11, AF_ODARK, nullptr},
    {"VM", A_VA + 12, AF_ODARK, nullptr},
    {"VN", A_VA + 13, AF_ODARK, nullptr},
    {"VO", A_VA + 14, AF_ODARK, nullptr},
    {"VP", A_VA + 15, AF_ODARK, nullptr},
    {"VQ", A_VA + 16, AF_ODARK, nullptr},
    {"VR", A_VA + 17, AF_ODARK, nullptr},
    {"VS", A_VA + 18, AF_ODARK, nullptr},
    {"VT", A_VA + 19, AF_ODARK, nullptr},
    {"VU", A_VA + 20, AF_ODARK, nullptr},
    {"VV", A_VA + 21, AF_ODARK, nullptr},
    {"VW", A_VA + 22, AF_ODARK, nullptr},
    {"VX", A_VA + 23, AF_ODARK, nullptr},
    {"VY", A_VA + 24, AF_ODARK, nullptr},
    {"VZ", A_VA + 25, AF_ODARK, nullptr},
    {"Xtype", A_XTYPE, AF_MDARK | AF_WIZARD, nullptr},
    {"*Password", A_PASS, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL,
     nullptr},
    {"*Privileges", A_PRIVS, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL,
     nullptr},
    {"Techtime", A_TECHTIME, AF_MDARK | AF_WIZARD, nullptr},
    {"*EconParts", A_ECONPARTS, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL,
     nullptr},
    {"PLHEALTH", A_HEALTH, PLSTAT_MODE, nullptr},
    {"PLATTRS", A_ATTRS, PLSTAT_MODE, nullptr},
    {"PLADVS", A_ADVS, PLSTAT_MODE, nullptr},
    {"PLSKILLS", A_SKILLS, PLSTAT_MODE, nullptr},

    {nullptr, 0, 0, nullptr}};

/*
 * ---------------------------------------------------------------------------
 * * fwdlist_set, fwdlist_clr: Manage cached forwarding lists
 */
void fwdlist_set(GameDatabase *database, DbRef thing, FWDLIST *ifp) {
  FWDLIST *fp, *xfp;
  int i;

  /*
   * If fwdlist is null, clear
   */

  if (!ifp || (ifp->count <= 0)) {
    fwdlist_clr(database, thing);
    return;
  }
  /*
   * Copy input forwardlist to a correctly-sized buffer
   */

  fp = (FWDLIST *)malloc(sizeof(FWDLIST));

  for (i = 0; i < ifp->count; i++) {
    fp->data[i] = ifp->data[i];
  }
  fp->count = ifp->count;

  /*
   * Replace an existing forwardlist, or add a new one
   */

  xfp = fwdlist_get(database, thing);
  if (xfp) {
    free(xfp);
    numeric_hash_table_replace(thing, (int *)fp,
                               &database->indexes->forward_lists);
  } else {
    numeric_hash_table_add(thing, (int *)fp, &database->indexes->forward_lists);
  }
}

void fwdlist_clr(GameDatabase *database, DbRef thing) {
  FWDLIST *xfp;

  /*
   * If a forwardlist exists, delete it
   */

  xfp = fwdlist_get(database, thing);
  if (xfp) {
    free(xfp);
    numeric_hash_table_delete(thing, &database->indexes->forward_lists);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fwdlist_load: Load text into a forwardlist.
 */
int fwdlist_load(EvaluationContext *evaluation, FWDLIST *fp, DbRef player,
                 char *atext) {
  DbRef target;
  char *tp, *bp, *dp;
  int count, errors, fail;

  if (!atext)
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    atext = (char *)"";
#pragma clang diagnostic pop

  count = 0;
  errors = 0;
  bp = tp = alloc_lbuf("fwdlist_load.str");
  StringCopy(tp, atext);

  do {
    for (; *bp && isspace(*bp); bp++)
      ; /*
         * skip spaces
         */
    for (dp = bp; *bp && !isspace(*bp); bp++)
      ; /*
         * remember string
         */
    if (*bp)
      *bp++ = '\0'; /*
                     * terminate string
                     */
    if ((*dp++ == '#') && isdigit(*dp)) {
      target = clamped_atol(dp);
      fail = (!is_good_obj(evaluation->world->database, target) ||
              (!is_god(evaluation->world->database, player) &&
               !is_controls(evaluation, player, target)));
      if (fail) {
        notify_printf(evaluation, player,
                      "Cannot forward to #%ld: Permission denied.", target);
        errors++;
      } else {
        fp->data[count++] = (int)target;
      }
    }
  } while (*bp);

  free_lbuf(tp);
  fp->count = count;
  return errors;
}

/*
 * ---------------------------------------------------------------------------
 * * fwdlist_rewrite: Generate a text string from a FWDLIST buffer.
 */
int fwdlist_rewrite(GameDatabase *database, FWDLIST *fp, char *atext) {
  char *tp, *bp;
  int i, count;

  if (fp && fp->count) {
    count = fp->count;
    tp = alloc_sbuf("fwdlist_rewrite.errors");
    bp = atext;
    for (i = 0; i < fp->count; i++) {
      if (is_good_obj(database, fp->data[i])) {
        snprintf(tp, SBUF_SIZE, "#%d ", fp->data[i]);
        safe_str(tp, atext, &bp);
      } else {
        count--;
      }
    }
    *bp = '\0';
    free_sbuf(tp);
  } else {
    count = 0;
    if (atext)
      *atext = '\0';
  }
  return count;
}

/*
 * ---------------------------------------------------------------------------
 * * fwdlist_ck:  Check a list of dbref numbers to forward to for AUDIBLE
 */

int fwdlist_ck(EvaluationContext *evaluation, int key, DbRef player,
               DbRef thing, int anum, char *atext) {
  FWDLIST *fp;
  int count;

  count = 0;

  if (atext && *atext) {
    fp = (FWDLIST *)alloc_lbuf("fwdlist_ck.fp");
    fwdlist_load(evaluation, fp, player, atext);
  } else {
    fp = nullptr;
  }

  /*
   * Set the cached forwardlist
   */

  fwdlist_set(evaluation->world->database, thing, fp);
  count = atext ? fwdlist_rewrite(evaluation->world->database, fp, atext)
                : (fp ? fp->count : 0);
  if (fp)
    free_lbuf(fp);
  return ((count > 0) || !atext || !*atext);
}

FWDLIST *fwdlist_get(GameDatabase *database, DbRef thing) {
  FWDLIST *fp;

  fp = ((FWDLIST *)numeric_hash_table_find(thing,
                                           &database->indexes->forward_lists));

  return fp;
}

static char *set_string(char **ptr, char *new) {
  /*
   * if pointer not null unalloc it
   */

  if (*ptr)
    free(*ptr);

  /*
   * if new string is not null allocate space for it and copy it
   */

  if (!new)                  /*
                              * * || !*new
                              */
    return (*ptr = nullptr); /*
                              * Check with GAC about this
                              */
  *ptr = (char *)malloc(strlen(new) + 1);
  StringCopy(*ptr, new);
  return (*ptr);
}

/*
 * ---------------------------------------------------------------------------
 * * Name, s_Name: Get or set an object's name.
 */
INLINE char *game_object_name(GameDatabase *database, DbRef thing) {
  DbRef aowner;
  long aflags;
  char *buff;
  char buffer[MBUF_SIZE];

  if (database->configuration->cache_names) {
    if (thing > database->top || thing < 0) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
      return (char *)"#-1 INVALID DBREF";
#pragma clang diagnostic pop
    }
    if (!database->pure_names[thing]) {
      buff = attribute_get(database, thing, A_NAME, &aowner, &aflags);
      strip_ansi_r(buffer, buff, MBUF_SIZE);
      set_string(&database->pure_names[thing], buffer);
      free_lbuf(buff);
    }
  }

  attribute_get_string(database, database->name_buffer, thing, A_NAME, &aowner,
                       &aflags);
  return database->name_buffer;
}

INLINE char *game_object_pure_name(GameDatabase *database, DbRef thing) {
  DbRef aowner;
  long aflags;
  char *buff;

  if (database->configuration->cache_names) {
    if (thing > database->top || thing < 0) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
      return (char *)"#-1 INVALID DBREF";
#pragma clang diagnostic pop
    }
    if (!database->pure_names[thing]) {
      char new[LBUF_SIZE];

      buff = attribute_get(database, thing, A_NAME, &aowner, &aflags);
      set_string(&database->pure_names[thing],
                 strip_ansi_r(new, buff, strlen(buff)));
      free_lbuf(buff);
    }
    return database->pure_names[thing];
  }

  attribute_get_string(database, database->name_buffer, thing, A_NAME, &aowner,
                       &aflags);
  return strip_ansi_r(database->pure_name_buffer, database->name_buffer,
                      strlen(database->name_buffer));
}

INLINE void object_name_set(GameDatabase *database, DbRef thing, char *s) {
  char new[MBUF_SIZE];
  /* Truncate the name if we have to */

  strncpy(new, s, MBUF_SIZE - 1);
  if (s && (strlen(s) > MBUF_SIZE))
    s[MBUF_SIZE] = '\0';

  attribute_add_raw(database, thing, A_NAME, (char *)s);

  if (database->configuration->cache_names) {
    set_string(&database->pure_names[thing], strip_ansi_r(new, s, strlen(s)));
  }
}

void object_password_set(GameDatabase *database, DbRef thing, const char *s) {
  /* attribute_add_raw()'s buffer parameter isn't const-correct; s is only
     read (copied) here, never mutated. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  attribute_add_raw(database, thing, A_PASS, (char *)s);
#pragma clang diagnostic pop
}

/*
 * ---------------------------------------------------------------------------
 * * do_attrib: Manage user-named attributes.
 */

extern NameTable attraccess_nametab[];

void do_attribute(CommandInvocation *invocation) {
  CommandRuntime *runtime = invocation->context->runtime;
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *aname = invocation->first;
  char *value = invocation->second;
  int success, negate, f;
  char *buff, *sp, *p, *q;
  VATTR *va;
  Attribute *va2;

  /*
   * Look up the user-named attribute we want to play with
   */

  buff = alloc_sbuf("do_attribute");
  for (p = buff, q = aname; *q && ((p - buff) < (SBUF_SIZE - 1)); p++, q++)
    *p = ToUpper(*q);
  *p = '\0';

  va = (VATTR *)vattr_find(runtime->vattrs, buff);
  if (!va) {
    notify_printf(&invocation->context->evaluation, player,
                  "No such user-named attribute: %s", buff);
    free_sbuf(buff);
    return;
  }
  switch (key) {
  case ATTRIB_ACCESS:

    /*
     * Modify access to user-named attribute
     */

    for (sp = value; *sp; sp++)
      *sp = ToUpper(*sp);
    sp = strtok(value, " ");
    success = 0;
    while (sp != nullptr) {

      /*
       * Check for negation
       */

      negate = 0;
      if (*sp == '!') {
        negate = 1;
        sp++;
      }
      /*
       * Set or clear the appropriate bit
       */

      f = name_table_search(runtime->world->database,
                            runtime->world->configuration, player,
                            attraccess_nametab, sp);
      if (f > 0) {
        success = 1;
        if (negate)
          va->flags &= ~f;
        else
          va->flags |= f;
      } else {
        notify_printf(&invocation->context->evaluation, player,
                      "Unknown permission: %s.", sp);
      }

      /*
       * Get the next token
       */

      sp = strtok(nullptr, " ");
    }
    if (success && !is_quiet(runtime->world->database, player))
      notify_printf(&invocation->context->evaluation, player,
                    "Attribute access for %s changed to %s.", va->name, value);
    break;

  case ATTRIB_RENAME:

    /*
     * Make sure the new name doesn't already exist
     */

    va2 = attribute_by_name(invocation->context->world->database, value);
    if (va2) {
      notify(evaluation, player, "An attribute with that name already exists.");
      free_sbuf(buff);
      return;
    }
    if (vattr_rename(runtime->vattrs, va->name, value) == nullptr)
      notify(evaluation, player, "Attribute rename failed.");
    else
      notify(evaluation, player, "Attribute renamed.");
    break;

  case ATTRIB_DELETE:

    /*
     * Remove the attribute
     */

    vattr_delete(runtime->vattrs, buff);
    notify(evaluation, player, "Attribute deleted.");
    break;
  default:
    break;
  }
  free_sbuf(buff);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * init_attrtab: Initialize the attribute hash tables.
 */

void init_attrtab(GameDatabase *database) {
  Attribute *a;
  char *buff, *p;
  const char *q;

  hash_table_initialize(&database->indexes->attributes, 512);
  buff = alloc_sbuf("init_attrtab");
  for (a = attr_table; a->number; a++) {
    anum_extend(database, a->number);
    anum_set(database, a->number, a);
    for (p = buff, q = a->name; *q; p++, q++)
      *p = ToUpper(*q);
    *p = '\0';
    hash_table_add(buff, (int *)a, &database->indexes->attributes);
  }
  free_sbuf(buff);
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_by_name: Look up an attribute by name.
 */

Attribute *attribute_by_name(GameDatabase *database, const char *s) {
  char *buff, *p;
  const char *q;
  Attribute *a;
  VATTR *va;
  static Attribute tattr;

  if (!s || !*s) {
    return (nullptr);
  }

  /*
   * Convert the buffer name to lowercase
   */

  buff = alloc_sbuf("attribute_by_name");
  for (p = buff, q = s; *q && ((p - buff) < (SBUF_SIZE - 1)); p++, q++)
    *p = ToUpper(*q);
  *p = '\0';

  /*
   * Look for a predefined attribute
   */

  a = (Attribute *)hash_table_find(buff, &database->indexes->attributes);
  if (a != nullptr) {
    free_sbuf(buff);
    return a;
  }
  /*
   * Nope, look for a user attribute
   */

  va = (VATTR *)vattr_find(database->vattrs, buff);
  free_sbuf(buff);

  /*
   * If we got one, load tattr and return a pointer to it.
   */

  if (va != nullptr) {
    tattr.name = va->name;
    tattr.number = va->number;
    tattr.flags = va->flags;
    tattr.check = nullptr;
    return &tattr;
  }
  /*
   * All failed, return NULL
   */

  return nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * anum_extend: Grow the attr num lookup table.
 */

Attribute *game_database_anum_get(GameDatabase *database, int number) {
  return database->attributes_by_number[number];
}

void game_database_anum_set(GameDatabase *database, int number,
                            Attribute *attribute) {
  database->attributes_by_number[number] = attribute;
}

void anum_extend(GameDatabase *database, int newtop) {
  Attribute **anum_table2;
  int delta, i;

  delta = database->configuration->init_size;

  if (newtop <= database->attribute_capacity)
    return;
  if (newtop < database->attribute_capacity + delta)
    newtop = database->attribute_capacity + delta;
  if (database->attributes_by_number == nullptr) {
    database->attributes_by_number =
        malloc((size_t)(newtop + 1) * sizeof(Attribute *));
    for (i = 0; i <= newtop; i++)
      database->attributes_by_number[i] = nullptr;
  } else {
    anum_table2 = malloc((size_t)(newtop + 1) * sizeof(Attribute *));
    for (i = 0; i <= database->attribute_capacity; i++)
      anum_table2[i] = database->attributes_by_number[i];
    for (i = database->attribute_capacity + 1; i <= newtop; i++)
      anum_table2[i] = nullptr;
    free((char *)database->attributes_by_number);
    database->attributes_by_number = anum_table2;
  }
  database->attribute_capacity = newtop;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_by_number: Look up an attribute by number.
 */

Attribute *attribute_by_number(GameDatabase *database, int anum) {
  VATTR *va;
  static Attribute tattr;

  /*
   * Look for a predefined attribute
   */

  if (anum < A_USER_START)
    return anum_get(database, anum);

  if (anum >= database->attribute_capacity)
    return nullptr;

  /*
   * It's a user-defined attribute, we need to copy data
   */

  va = (VATTR *)anum_get(database, anum);
  if (va != nullptr) {
    tattr.name = va->name;
    tattr.number = va->number;
    tattr.flags = va->flags;
    tattr.check = nullptr;
    return &tattr;
  }
  /*
   * All failed, return NULL
   */

  return nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * mkattr: Lookup attribute by name, creating if needed.
 */

int mkattr(GameDatabase *database, char *buff) {
  Attribute *ap;
  VATTR *va;

  if (!(ap = attribute_by_name(database, buff))) {

    /*
     * Unknown attr, create a new one
     */

    va = vattr_alloc(database->vattrs, buff,
                     database->configuration->vattr_flags);
    if (!va || !(va->number))
      return -1;
    return va->number;
  }
  if (!(ap->number))
    return -1;
  return ap->number;
}

/*
 * routines to handle object attribute lists
 */

/*
 * ---------------------------------------------------------------------------
 * * attribute_encode: Encode an attribute string.
 */
static char *attribute_encode(GameDatabase *database, char *iattr, DbRef thing,
                              DbRef owner, long flags, int atr,
                              char *dest_buffer) {

  /*
   * If using the default owner and flags (almost all attributes will),
   * * * * * * * just store the string.
   */

  if (((owner == game_object_owner(database, thing)) || (owner == NOTHING)) &&
      !flags) {
    memset(dest_buffer, 0, LBUF_SIZE);
    strncpy(dest_buffer, iattr, LBUF_SIZE - 1);
    return dest_buffer;
  }

  /*
   * Encode owner and flags into the attribute text
   */

  if (owner == NOTHING)
    owner = game_object_owner(database, thing);
  memset(dest_buffer, 0, LBUF_SIZE);
  snprintf(dest_buffer, LBUF_SIZE - 1, "%c%ld:%ld:%s", ATR_INFO_CHAR, owner,
           flags, iattr);
  return dest_buffer;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_decode: Decode an attribute string.
 */

static void attribute_decode(GameDatabase *database, char *iattr, char *oattr,
                             DbRef thing, DbRef *owner, long *flags, int atr) {
  char *cp;
  int neg;
  int attrOwner;

  /*
   * See if the first char of the attribute is the special character
   */

  if (*iattr == ATR_INFO_CHAR) {

    /*
     * It is, crack the attr apart
     */

    cp = &iattr[1];

    /*
     * Get the attribute owner
     */

    attrOwner = 0;
    neg = 0;
    if (*cp == '-') {
      neg = 1;
      cp++;
    }
    while (isdigit(*cp)) {
      attrOwner = (attrOwner * 10) + (*cp++ - '0');
    }
    if (neg)
      attrOwner = 0 - attrOwner;

    *owner = attrOwner;
    /*
     * If delimiter is not ':', just return attribute
     */

    if (*cp++ != ':') {
      if (owner)
        *owner = game_object_owner(database, thing);
      if (flags)
        *flags = 0;
      if (oattr) {
        StringCopy(oattr, iattr);
      }
      return;
    }
    /*
     * Get the attribute flags
     */

    *flags = 0;
    while (isdigit(*cp)) {
      *flags = (*flags * 10) + (*cp++ - '0');
    }

    /*
     * If delimiter is not ':', just return attribute
     */

    if (*cp++ != ':') {
      if (owner)
        *owner = game_object_owner(database, thing);
      if (flags)
        *flags = 0;
      if (oattr) {
        StringCopy(oattr, iattr);
      }
    }
    /*
     * Get the attribute text
     */

    if (oattr)
      StringCopy(oattr, cp);
    if (attrOwner == NOTHING && owner)
      *owner = game_object_owner(database, thing);
  } else {

    /*
     * Not the special character, return normal info
     */

    if (owner)
      *owner = game_object_owner(database, thing);
    if (flags)
      *flags = 0;
    if (oattr)
      StringCopy(oattr, iattr);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_clear: clear an attribute in the list.
 */

void attribute_clear(GameDatabase *database, DbRef thing, int atr) {
  AttributeList *list;
  int hi, lo, mid;

  if (!database->objects[thing].at_count || !database->objects[thing].ahead)
    return;

  if (database->objects[thing].at_count < 0)
    abort();

  /*
   * Binary search for the attribute.
   */
  lo = 0;
  hi = database->objects[thing].at_count - 1;
  list = database->objects[thing].ahead;
  while (lo <= hi) {
    mid = ((hi - lo) >> 1) + lo;
    if (list[mid].number == atr) {
      free(list[mid].data);
      database->objects[thing].at_count -= 1;
      if (mid != database->objects[thing].at_count)
        bcopy((char *)(list + mid + 1), (char *)(list + mid),
              (size_t)(database->objects[thing].at_count - mid) *
                  sizeof(AttributeList));
      break;
    } else if (list[mid].number > atr) {
      hi = mid - 1;
    } else {
      lo = mid + 1;
    }
  }

  switch (atr) {
  case A_FORWARDLIST:
    game_object_set_flags2(database, thing,
                           game_object_flags2(database, thing) & ~HAS_FWDLIST);
    break;
  case A_LISTEN:
    game_object_set_flags2(database, thing,
                           game_object_flags2(database, thing) & ~HAS_LISTEN);
    break;
  case A_TIMEOUT:
    descriptor_reload(database, database->configuration, database->descriptors,
                      thing);
    break;
  case A_QUEUEMAX:
    pcache_reload(database->players, thing);
    break;
  default:
    break;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_add_raw, attribute_add: add attribute of type atr to list
 */

void attribute_add_raw(GameDatabase *database, DbRef thing, int atr,
                       char *buff) {
  AttributeList *list;
  char *text;
  int found = 0;
  int hi, lo, mid;

  if (!buff || !*buff) {
    attribute_clear(database, thing, atr);
    return;
  }
  if (strlen(buff) >= LBUF_SIZE) {
    buff[LBUF_SIZE - 1] = '\0';
  }
  if ((text = malloc(strlen(buff) + 1)) == nullptr) {
    return;
  }
  StringCopy(text, buff);

  if (!database->objects[thing].ahead) {
    if ((list = malloc(sizeof(AttributeList))) == nullptr) {
      free(text);
      return;
    }
    database->objects[thing].ahead = list;
    database->objects[thing].at_count = 1;
    list[0].number = atr;
    list[0].data = text;
    list[0].size = (int)strlen(text) + 1;
    found = 1;
  } else {

    /*
     * Binary search for the attribute
     */
    lo = 0;
    hi = database->objects[thing].at_count - 1;

    list = database->objects[thing].ahead;
    while (lo <= hi) {
      mid = ((hi - lo) >> 1) + lo;
      if (list[mid].number == atr) {
        free(list[mid].data);
        list[mid].data = text;
        list[mid].size = (int)strlen(text) + 1;
        found = 1;
        break;
      } else if (list[mid].number > atr) {
        hi = mid - 1;
      } else {
        lo = mid + 1;
      }
    }

    if (!found) {
      /*
       * If we got here, we didn't find it, so lo = hi + 1,
       * and the attribute should be inserted between them.
       */

      list = realloc(database->objects[thing].ahead,
                     (size_t)(database->objects[thing].at_count + 1) *
                         sizeof(AttributeList));

      if (!list) {
        free(text);
        return;
      }

      /*
       * Move the stuff upwards one slot
       */
      if (lo < database->objects[thing].at_count)
        bcopy((char *)(list + lo), (char *)(list + lo + 1),
              (size_t)(database->objects[thing].at_count - lo) *
                  sizeof(AttributeList));

      list[lo].data = text;
      list[lo].number = atr;
      list[lo].size = (int)strlen(text) + 1;
      database->objects[thing].at_count++;
      database->objects[thing].ahead = list;
    }
  }

  switch (atr) {
  case A_FORWARDLIST:
    game_object_set_flags2(database, thing,
                           game_object_flags2(database, thing) | HAS_FWDLIST);
    break;
  case A_LISTEN:
    game_object_set_flags2(database, thing,
                           game_object_flags2(database, thing) | HAS_LISTEN);
    break;
  case A_TIMEOUT:
    descriptor_reload(database, database->configuration, database->descriptors,
                      thing);
    break;
  case A_QUEUEMAX:
    pcache_reload(database->players, thing);
    break;
  default:
    break;
  }
}

void attribute_add(GameDatabase *database, DbRef thing, int atr, char *buff,
                   DbRef owner, long flags) {
  char *tbuff;
  char buffer[LBUF_SIZE];

  if (!buff || !*buff) {
    attribute_clear(database, thing, atr);
  } else {
    tbuff = attribute_encode(database, buff, thing, owner, flags, atr, buffer);
    attribute_add_raw(database, thing, atr, tbuff);
  }
}

void attribute_set_owner(GameDatabase *database, DbRef thing, int atr,
                         DbRef owner) {
  DbRef aowner;
  long aflags;
  char *buff;

  buff = attribute_get(database, thing, atr, &aowner, &aflags);
  attribute_add(database, thing, atr, buff, owner, aflags);
  free_lbuf(buff);
}

void attribute_set_flags(GameDatabase *database, DbRef thing, int atr,
                         DbRef flags) {
  DbRef aowner;
  long aflags;
  char *buff;

  buff = attribute_get(database, thing, atr, &aowner, &aflags);
  attribute_add(database, thing, atr, buff, aowner, flags);
  free_lbuf(buff);
}

/*
 * ---------------------------------------------------------------------------
 * * get_atr,attribute_get_raw, attribute_get_string, attribute_get: Get an
 * attribute from the database.
 */

int get_atr(GameDatabase *database, char *name) {
  Attribute *ap;

  if (!(ap = attribute_by_name(database, name)))
    return 0;
  if (!(ap->number))
    return -1;
  return ap->number;
}

char *attribute_get_raw(GameDatabase *database, DbRef thing, int atr) {
  int lo, mid, hi;
  AttributeList *list;

  if (thing < 0)
    return nullptr;

  /*
   * Binary search for the attribute
   */
  lo = 0;
  hi = database->objects[thing].at_count - 1;
  list = database->objects[thing].ahead;
  if (!list)
    return nullptr;

  while (lo <= hi) {
    mid = ((hi - lo) >> 1) + lo;
    if (list[mid].number == atr) {

      return list[mid].data;
    } else if (list[mid].number > atr) {
      hi = mid - 1;
    } else {
      lo = mid + 1;
    }
  }
  return nullptr;
}

char *attribute_get_string(GameDatabase *database, char *s, DbRef thing,
                           int atr, DbRef *owner, long *flags) {
  char *buff;

  buff = attribute_get_raw(database, thing, atr);
  if (!buff) {
    if (owner)
      *owner = game_object_owner(database, thing);
    if (flags)
      *flags = 0;
    *s = '\0';
  } else {
    attribute_decode(database, buff, s, thing, owner, flags, atr);
  }
  return s;
}

char *attribute_get(GameDatabase *database, DbRef thing, int atr, DbRef *owner,
                    long *flags) {
  char *buff;

  buff = alloc_lbuf("attribute_get");
  return attribute_get_string(database, buff, thing, atr, owner, flags);
}

int attribute_get_info(GameDatabase *database, DbRef thing, int atr,
                       DbRef *owner, long *flags) {
  char *buff;

  buff = attribute_get_raw(database, thing, atr);
  if (!buff) {
    *owner = game_object_owner(database, thing);
    *flags = 0;
    return 0;
  }
  attribute_decode(database, buff, nullptr, thing, owner, flags, atr);
  return 1;
}

char *attribute_parent_get_string(GameDatabase *database, char *s, DbRef thing,
                                  int atr, DbRef *owner, long *flags) {
  char *buff;
  DbRef parent;
  int lev;

  Attribute *ap;

  ITER_PARENTS(database, database->configuration, thing, parent, lev) {
    buff = attribute_get_raw(database, parent, atr);
    if (buff && *buff) {
      attribute_decode(database, buff, s, thing, owner, flags, atr);
      if ((lev == 0) || !(*flags & AF_PRIVATE))
        return s;
    }
    if ((lev == 0) &&
        is_good_obj(database, game_object_parent(database, parent))) {
      ap = attribute_by_number(database, atr);
      if (!ap || ap->flags & AF_PRIVATE)
        break;
    }
  }
  *owner = game_object_owner(database, thing);
  *flags = 0;
  *s = '\0';
  return s;
}

char *attribute_parent_get(GameDatabase *database, DbRef thing, int atr,
                           DbRef *owner, long *flags) {
  char *buff;

  buff = alloc_lbuf("attribute_parent_get");
  return attribute_parent_get_string(database, buff, thing, atr, owner, flags);
}

int attribute_parent_get_info(GameDatabase *database, DbRef thing, int atr,
                              DbRef *owner, long *flags) {
  char *buff;
  DbRef parent;
  int lev;
  Attribute *ap;

  ITER_PARENTS(database, database->configuration, thing, parent, lev) {
    buff = attribute_get_raw(database, parent, atr);
    if (buff && *buff) {
      attribute_decode(database, buff, nullptr, thing, owner, flags, atr);
      if ((lev == 0) || !(*flags & AF_PRIVATE))
        return 1;
    }
    if ((lev == 0) &&
        is_good_obj(database, game_object_parent(database, parent))) {
      ap = attribute_by_number(database, atr);
      if (!ap || ap->flags & AF_PRIVATE)
        break;
    }
  }
  *owner = game_object_owner(database, thing);
  *flags = 0;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_free: Return all attributes of an object.
 */

void attribute_free(GameDatabase *database, DbRef thing) {
  free(database->objects[thing].ahead);
  database->objects[thing].ahead = nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_copy: Copy all attributes from one object to another.  Takes the
 * * player argument to ensure that only attributes that COULD be set by
 * * the player are copied.
 */

void attribute_copy(EvaluationContext *evaluation, DbRef player, DbRef dest,
                    DbRef source) {
  int attr;
  long aflags;
  DbRef owner, aowner;
  char *as, *buf;
  Attribute *at;

  owner = game_object_owner(evaluation->world->database, dest);
  for (attr = attribute_list_first(evaluation->world->database, source, &as);
       attr; attr = attribute_list_next(&as)) {
    buf = attribute_get(evaluation->world->database, source, attr, &aowner,
                        &aflags);
    aowner = owner;
    at = attribute_by_number(evaluation->world->database, attr);
    if (attr && at) {
      if (write_attr(evaluation, owner, dest, at, aflags))
        /*
         * Only set attrs that owner has perm to set
         */
        attribute_add(evaluation->world->database, dest, attr, buf, aowner,
                      aflags);
    }
    free_lbuf(buf);
  }
  if (as)
    free(as);
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_change_owner: Change the ownership of the attributes of an object
 * to the
 * * current owner if they are not locked.
 */

void attribute_change_owner(GameDatabase *database, DbRef obj) {
  int attr;
  long aflags;
  DbRef owner, aowner;
  char *as, *buf;

  owner = game_object_owner(database, obj);
  for (attr = attribute_list_first(database, obj, &as); attr;
       attr = attribute_list_next(&as)) {
    buf = attribute_get(database, obj, attr, &aowner, &aflags);
    if (aowner != owner)
      attribute_add(database, obj, attr, buf, owner, aflags);
    free_lbuf(buf);
  }
  if (as)
    free(as);
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_list_next: Return next attribute in attribute list.
 */

int attribute_list_next(char **attrp) {
  ATRCOUNT *atr;

  if (!attrp || !*attrp) {
    return 0;
  } else {
    atr = (ATRCOUNT *)(void *)*attrp;
    if (atr->count > atr->database->objects[atr->thing].at_count) {
      free(atr);
      *attrp = nullptr;
      return 0;
    }
    atr->count++;
    return atr->database->objects[atr->thing].ahead[atr->count - 2].number;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_list_first: Returns the head of the attr list for object 'thing'
 */

int attribute_list_first(GameDatabase *database, DbRef thing, char **attrp) {
  ATRCOUNT *atr;

  if (database->objects[thing].at_count) {
    atr = malloc(sizeof(ATRCOUNT));
    atr->database = database;
    atr->thing = thing;
    atr->count = 2;
    *attrp = (char *)atr;
    if (!database->objects[thing].ahead[0].number) {
      free(atr);
      *attrp = nullptr;
      return 0;
    }
    return database->objects[thing].ahead[0].number;
  }
  *attrp = nullptr;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * db_grow: Extend the struct database.
 */

// So mistaken refs to #-1 won't die.
constexpr int SIZE_HACK = 1;

static void initialize_objects(GameDatabase *database, DbRef first,
                               DbRef last) {
  DbRef thing;

  for (thing = first; thing < last; thing++) {
    memset(game_database_object(database, thing), 0, sizeof(GameObject));
    game_object_set_owner(database, thing, GOD);
    game_object_set_flags(database, thing, (TYPE_GARBAGE | GOING));
    game_object_set_flags2(database, thing, 0);
    game_object_set_flags3(database, thing, 0);
    game_object_set_powers(database, thing, 0);
    game_object_set_powers2(database, thing, 0);
    game_object_set_location(database, thing, NOTHING);
    game_object_set_contents(database, thing, NOTHING);
    game_object_set_exits(database, thing, NOTHING);
    game_object_set_link(database, thing, NOTHING);
    game_object_set_next(database, thing, NOTHING);
    game_object_set_zone(database, thing, NOTHING);
    game_object_set_parent(database, thing, NOTHING);
    game_object_set_stack(database, thing, nullptr);
    game_database_object(database, thing)->ahead = nullptr;
    game_database_object(database, thing)->at_count = 0;
  }
}

void db_grow(GameDatabase *database, DbRef newtop) {
  int newsize, marksize, delta, i;
  DatabaseMarkBuffer *newmarkbuf;
  GameObject *newdb;
  NAME *newpurenames;

  char *cp;

  delta = database->configuration->init_size;

  /*
   * Determine what to do based on requested size, current top and  * *
   *
   * *  * *  * *  * * size.  Make sure we grow in reasonable-sized
   * chunks to * * prevent *  * *  * frequent reallocations of the
   * database->objects array.
   */

  /*
   * If requested size is smaller than the current database->objects size,
   * ignore it
   */

  if (newtop <= database->top) {
    return;
  }
  /*
   * If requested size is greater than the current database->objects size but
   * smaller
   * * * * * * * than the amount of space we have allocated, raise the
   * database->objects  * *  * size * * and * initialize the new area.
   */

  if (newtop <= database->size) {
    for (i = database->top; i < newtop; i++) {
      if (database->configuration->cache_names)
        database->pure_names[i] = nullptr;
    }
    initialize_objects(database, database->top, newtop);
    database->top = (int)newtop;
    return;
  }
  /*
   * Grow by a minimum of delta objects
   */

  if (newtop <= database->size + delta) {
    newsize = database->size + delta;
  } else {
    newsize = (int)newtop;
  }

  /*
   * Enforce minimumdatabase size
   */

  if (newsize < database->minimum_size)
    newsize = database->minimum_size + delta;
  ;

  /*
   * Grow the name tables
   */

  if (database->configuration->cache_names) {
    newpurenames = (NAME *)malloc((size_t)(newsize + SIZE_HACK) * sizeof(NAME));

    if (!newpurenames) {
      log_simple(
          database->log, LOG_ALWAYS, "ALC", "DB",
          tprintf("Could not allocate space for %d item name cache.", newsize));
      abort();
    }
    bzero((char *)newpurenames, (size_t)(newsize + SIZE_HACK) * sizeof(NAME));

    if (database->pure_names) {

      /*
       * An old name cache exists.  Copy it.
       */

      database->pure_names -= SIZE_HACK;
      bcopy((char *)database->pure_names, (char *)newpurenames,
            (size_t)(newtop + SIZE_HACK) * sizeof(NAME));
      cp = (char *)database->pure_names;
      free(cp);
    } else {

      /*
       * Creating a brand new struct database.  Fill in the
       * 'reserved' area in case it gets referenced.
       */

      database->pure_names = newpurenames;
      for (i = 0; i < SIZE_HACK; i++) {
        database->pure_names[i] = nullptr;
      }
    }
    database->pure_names = newpurenames + SIZE_HACK;
    newpurenames = nullptr;
  }
  /*
   * Grow the database->objects array
   */

  newdb =
      (GameObject *)malloc((size_t)(newsize + SIZE_HACK) * sizeof(GameObject));
  if (!newdb) {

    log_simple(database->log, LOG_ALWAYS, "ALC", "DB",
               tprintf("Could not allocate space for %d item struct database.",
                       newsize));
    abort();
  }
  if (database->objects) {

    /*
     * An old struct database exists.  Copy it to the new buffer
     */

    database->objects -= SIZE_HACK;
    bcopy((char *)database->objects, (char *)newdb,
          (size_t)(database->top + SIZE_HACK) * sizeof(GameObject));
    cp = (char *)database->objects;
    free(cp);
  } else {

    /*
     * Creating a brand new struct database.  Fill in the * * * *
     *
     * *  * * 'reserved' area in case it gets referenced.
     */

    database->objects = newdb;
    for (i = 0; i < SIZE_HACK; i++) {
      game_object_set_owner(database, i, GOD);
      game_object_set_flags(database, i, (TYPE_GARBAGE | GOING));
      game_object_set_powers(database, i, 0);
      game_object_set_powers2(database, i, 0);
      game_object_set_location(database, i, NOTHING);
      game_object_set_contents(database, i, NOTHING);
      game_object_set_exits(database, i, NOTHING);
      game_object_set_link(database, i, NOTHING);
      game_object_set_next(database, i, NOTHING);
      game_object_set_zone(database, i, NOTHING);
      game_object_set_parent(database, i, NOTHING);
      game_object_set_stack(database, i, nullptr);
      game_database_object(database, i)->ahead = nullptr;
      game_database_object(database, i)->at_count = 0;
    }
  }
  database->objects = newdb + SIZE_HACK;
  newdb = nullptr;

  for (i = database->top; i < newtop; i++) {
    if (database->configuration->cache_names) {
      database->pure_names[i] = nullptr;
    }
  }
  initialize_objects(database, database->top, newtop);
  database->top = (int)newtop;
  database->size = newsize;

  /*
   * Grow the database->objects mark buffer
   */

  marksize = (newsize + 7) >> 3;
  newmarkbuf = (DatabaseMarkBuffer *)malloc((size_t)marksize);
  bzero((char *)newmarkbuf, (size_t)marksize);
  if (database->markbits) {
    marksize = (int)((newtop + 7) >> 3);
    bcopy((char *)database->markbits, (char *)newmarkbuf, (size_t)marksize);
    cp = (char *)database->markbits;
    free(cp);
  }
  database->markbits = newmarkbuf;
}

void db_free(GameDatabase *database) {
  char *cp;

  if (database->objects != nullptr) {
    database->objects -= SIZE_HACK;
    cp = (char *)database->objects;
    free(cp);
    database->objects = nullptr;
  }
  if (database->pure_names != nullptr) {
    database->pure_names -= SIZE_HACK;
    free(database->pure_names);
    database->pure_names = nullptr;
  }
  free(database->markbits);
  database->markbits = nullptr;
  database->top = 0;
  database->size = 0;
  database->freelist = NOTHING;
}

void db_make_minimal(EvaluationContext *evaluation) {
  GameDatabase *database = evaluation->world->database;
  DbRef obj;

  db_free(database);
  db_grow(database, 1);
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  object_name_set(database, 0, (char *)"Limbo");
#pragma clang diagnostic pop
  game_object_set_flags(database, 0, TYPE_ROOM);
  game_object_set_powers(database, 0, 0);
  game_object_set_powers2(database, 0, 0);
  game_object_set_location(database, 0, NOTHING);
  game_object_set_exits(database, 0, NOTHING);
  game_object_set_link(database, 0, NOTHING);
  game_object_set_parent(database, 0, NOTHING);
  game_object_set_zone(database, 0, NOTHING);
  game_object_set_owner(database, 0, 1);
  database->objects[0].ahead = nullptr;
  database->objects[0].at_count = 0;
  /*
   * should be #1
   */
  load_player_names(evaluation->world);
  /* create_player()'s parameters aren't const-correct; these literals are
     only read here. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  obj = create_player(evaluation, (char *)"Wizard", (char *)"potrzebie",
                      NOTHING, 0);
#pragma clang diagnostic pop
  game_object_set_flags(database, obj,
                        game_object_flags(database, obj) | WIZARD);
  game_object_set_powers(database, obj, 0);
  game_object_set_powers2(database, obj, 0);

  /*
   * Manually link to Limbo, just in case
   */
  game_object_set_location(database, obj, 0);
  game_object_set_next(database, obj, NOTHING);
  game_object_set_contents(database, 0, obj);
  game_object_set_link(database, obj, 0);
}

DbRef parse_dbref(const char *s) {
  const char *p;
  long x;

  /*
   * Enforce completely numeric dbrefs
   */

  for (p = s; *p; p++) {
    if (!isdigit(*p))
      return NOTHING;
  }

  x = clamped_atol(s);
  return ((x >= 0) ? x : NOTHING);
}

/*
 * check_zone - checks back through a zone tree for control
 */
static int check_zone_at_depth(EvaluationContext *evaluation, DbRef player,
                               DbRef thing, int depth) {
  GameDatabase *database = evaluation->world->database;
  LuaLockInvocation lock;
  LuaLockResult result;
  if (!database->configuration->have_zones ||
      (game_object_zone(database, thing) == NOTHING) ||
      (depth == database->configuration->zone_nest_lim) ||
      is_player(database, thing)) {
    return 0;
  }

  /*
   * If the zone doesn't define an enter lock, DON'T allow control.
   */

  if (lock_test(evaluation, player, player, player,
                game_object_zone(database, thing), LUA_LOCK_ENTER,
                LUA_LOCK_OPERATION_ZONE_CONTROL, true, &lock, &result) &&
      result.defined) {
    return 1;
  }
  return check_zone_at_depth(evaluation, player,
                             game_object_zone(database, thing), depth + 1);
}

int check_zone(EvaluationContext *evaluation, DbRef player, DbRef thing) {
  return check_zone_at_depth(evaluation, player, thing, 1);
}

int check_zone_for_player(EvaluationContext *evaluation, DbRef player,
                          DbRef thing) {
  GameDatabase *database = evaluation->world->database;
  LuaLockInvocation lock;
  LuaLockResult result;
  if (!database->configuration->have_zones ||
      (game_object_zone(database, thing) == NOTHING) ||
      database->configuration->zone_nest_lim == 1 ||
      !is_player(database, thing)) {
    return 0;
  }

  if (lock_test(evaluation, player, player, player,
                game_object_zone(database, thing), LUA_LOCK_ENTER,
                LUA_LOCK_OPERATION_ZONE_CONTROL, true, &lock, &result) &&
      result.defined) {
    return 1;
  }
  return check_zone_at_depth(evaluation, player,
                             game_object_zone(database, thing), 2);
}

void toast_player(EvaluationContext *evaluation, DbRef player) {
  comsys_clear_player(evaluation, player);
  del_commac(evaluation->runtime->channels, player);
  do_clear_macro(&evaluation->command->match, evaluation->runtime->macros,
                 player, nullptr);
}
