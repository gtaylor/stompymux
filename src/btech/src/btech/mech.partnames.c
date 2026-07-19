/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "glue.h"
#include "mech.h"
#include "mech.partnames.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/support/hash_table.h"
#include "mux/world/world_context.h"
#include "p.template.h"

void list_hashstat(DbRef player, const char *tab_name, HashTable *htab);

/* Main idea:
   Keep 2 sorted tables, one of shortform -> index
   longform  -> index
   vlongform -> index
   Other
   index -> {short,long,vlong} form

   Index = ID + NUM_ITEMS * brand
 */

#define BRANDCOUNT 5
struct PartNameRegistry {
  PN *index_sorted[BRANDCOUNT + 1][NUM_ITEMS];
  PN **short_sorted;
  PN **long_sorted;
  PN **vlong_sorted;
  int object_count;
  HashTable short_hash;
  HashTable vlong_hash;
};

static void insert_sorted_brandname(PartNameRegistry *registry, int ind,
                                    PN *e) {
  int i, j;

#define UGLY_SORT(b, a)                                                        \
  for (i = 0; i < ind; i++)                                                    \
    if (strcmp(e->a, b[i]->a) < 0)                                             \
      break;                                                                   \
  for (j = ind; j > i; j--)                                                    \
    b[j] = b[j - 1];                                                           \
  b[i] = e
  UGLY_SORT(registry->short_sorted, shorty);
  UGLY_SORT(registry->long_sorted, longy);
  UGLY_SORT(registry->vlong_sorted, vlongy);
}

extern char *GetPartBrandName(int, int);

static int create_brandname(PartNameRegistry *registry,
                            const ServerConfiguration *configuration, int id,
                            int b) {
  char buf[LBUF_SIZE];
  char buf2[BTECH_TEXT_CAPACITY];
  char buf3[BTECH_TEXT_CAPACITY];
  char *c, *brn = NULL;
  PN *p;

  if (b)
    if (!(brn = GetPartBrandName(id, b)))
      return 0;
  Create(p, PN, 1);
/* \todo Remove this stupid #define and make the code readable */
#define SILLINESS(fun, val, fl)                                                \
  if (!(c = fun(configuration, id, b, (char[BTECH_TEXT_CAPACITY]){0}))) {      \
    free((void *)p->val);                                                      \
    free((void *)p);                                                           \
    return 0;                                                                  \
  }                                                                            \
  if (b) {                                                                     \
    strcpy(buf2, c);                                                           \
    if (fl)                                                                    \
      strcpy(buf3, my_shortform(brn, (char[BTECH_TEXT_CAPACITY]){0}));         \
    snprintf(buf, sizeof(buf), "%s.%s", fl ? buf3 : brn, buf2);                \
  } else                                                                       \
    strcpy(buf, c);                                                            \
  p->val = strdup(buf)
  SILLINESS(part_figure_out_name, vlongy, 0);
  SILLINESS(part_figure_out_sname, longy, 0);
  if (!(c = part_figure_out_shname(id, (char[BTECH_TEXT_CAPACITY]){0}))) {
    free((void *)p->shorty);
    free((void *)p);
    return 0;
  }
  if (b) {
    strcpy(buf2, c);
    strcpy(buf3, my_shortform(brn, (char[BTECH_TEXT_CAPACITY]){0}));
    snprintf(buf, sizeof(buf), "%s.%s", buf3, buf2);
  } else
    strcpy(buf, c);
  p->shorty = strdup(buf);
  p->index = PACKED_PART(id, b);
  registry->index_sorted[b][id] = p;
  return 1;
}

void initialize_partname_tables(BtechContext *context) {
  const ServerConfiguration *configuration = context->configuration;
  PartNameRegistry *registry = calloc(1, sizeof(*registry));
  long i;
  int j, c = 0, m, n;
  char tmpbuf[MBUF_SIZE];
  char *tmpc1, *tmpc2;

  if (registry == nullptr)
    exit(EXIT_FAILURE);
  context->part_names = registry;
  for (j = 0; j <= BRANDCOUNT; j++)
    for (i = 0; i < NUM_ITEMS; i++)
      c += create_brandname(registry, configuration, i, j);
  Create(registry->short_sorted, PN *, c);
  Create(registry->long_sorted, PN *, c);
  Create(registry->vlong_sorted, PN *, c);
  /* bubble-sort 'em and insert to array */
  i = 0;
  for (m = 0; m <= BRANDCOUNT; m++)
    for (n = 0; n < NUM_ITEMS; n++)
      if (registry->index_sorted[m][n])
        insert_sorted_brandname(registry, i++, registry->index_sorted[m][n]);
  hash_table_initialize(&registry->short_hash, 20 * HASH_FACTOR);
  hash_table_initialize(&registry->vlong_hash, 20 * HASH_FACTOR);
#define DASH(fromval, tohash)                                                  \
  for (tmpc1 = registry->short_sorted[i]->fromval, tmpc2 = tmpbuf; *tmpc1;     \
       tmpc1++, tmpc2++)                                                       \
    *tmpc2 = ToLower(*tmpc1);                                                  \
  *tmpc2 = 0;                                                                  \
  hash_table_add(tmpbuf, (int *)(i + 1), &registry->tohash);

  for (i = 0; i < c; i++) {
    DASH(shorty, short_hash);

    /*       DASH(longy, long_hash); */
    DASH(vlongy, vlong_hash);
  }
  registry->object_count = c;
}

#define SILLY_GET(fun, value)                                                  \
  char *fun(BtechContext *context, int i, int b) {                             \
    PartNameRegistry *registry = context->part_names;                          \
    if (i < 0 || i >= NUM_ITEMS || b < 0 || b > BRANDCOUNT)                    \
      return nullptr;                                                          \
    if (!(registry->index_sorted[b][i])) {                                     \
      if (b)                                                                   \
        return fun(context, i, 0);                                             \
      else                                                                     \
        return nullptr;                                                        \
    }                                                                          \
    return registry->index_sorted[b][i]->value;                                \
  }

SILLY_GET(get_parts_short_name, shorty);
SILLY_GET(get_parts_long_name, longy);
SILLY_GET(get_parts_vlong_name, vlongy);

#define wildcard_match quick_wild
extern int wildcard_match(const char *, const char *);

int find_matching_vlong_part(BtechContext *context, const char *wc, int *ind,
                             int *id, int *brand) {
  PartNameRegistry *registry = context->part_names;
  PN *p;
  const char *tmpc1;
  char *tmpc2;
  char tmpbuf[MBUF_SIZE];
  int *i;

  if (ind && *ind >= 0)
    return 0;
  for (tmpc1 = wc, tmpc2 = tmpbuf; *tmpc1; tmpc1++, tmpc2++) {
    *tmpc2 = ToLower(*tmpc1);
  }
  *tmpc2 = 0;
  if ((i = hash_table_find(tmpbuf, &registry->vlong_hash))) {
    if ((p = registry->short_sorted[((long)i) - 1])) {
      if (ind)
        *ind = ((long)i);
      UNPACK_PART(p->index, *id, *brand);
      return 1;
    }
  }
  return 0;
}

int find_matching_long_part(BtechContext *context, const char *wc, int *i,
                            int *id, int *brand) {
  PartNameRegistry *registry = context->part_names;
  PN *p;

  for ((*i)++; *i < registry->object_count; (*i)++)
    if (wildcard_match(wc, (p = registry->long_sorted[*i])->longy)) {
      UNPACK_PART(p->index, *id, *brand);
      return 1;
    }
  return 0;
}

int find_matching_short_part(BtechContext *context, const char *wc, int *ind,
                             int *id, int *brand) {
  PartNameRegistry *registry = context->part_names;
  PN *p;
  const char *tmpc1;
  char *tmpc2;
  char tmpbuf[MBUF_SIZE];
  int *i;

  if (*ind >= 0)
    return 0;
  for (tmpc1 = wc, tmpc2 = tmpbuf; *tmpc1; tmpc1++, tmpc2++) {
    *tmpc2 = ToLower(*tmpc1);
  }
  *tmpc2 = 0;
  if ((i = hash_table_find(tmpbuf, &registry->short_hash))) {
    if ((p = registry->short_sorted[((long)i) - 1])) {
      *ind = ((long)i);
      UNPACK_PART(p->index, *id, *brand);
      return 1;
    }
  }
  return 0;
}

void ListForms(DbRef player, void *data, char *buffer) {
  XCODE *debug = data;
  PartNameRegistry *registry = debug->context->part_names;
  int i;

  notify(btech_context_evaluation(debug->context), player, "Listing of forms:");
  for (i = 0; i < registry->object_count; i++)
    notify_printf(btech_context_evaluation(debug->context), player,
                  "%3d %-20s %-25s %s", i, registry->short_sorted[i]->shorty,
                  registry->short_sorted[i]->longy,
                  registry->short_sorted[i]->vlongy);
}

void fun_btpartmatch(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  /* fargs[0] = name to match on
   */

  /* Added check to see if anything was found, if not
   * send error message
   * Dany - 06/2005
   */

  int partindex = 0, id = 0, brand = 0;
  int part_count = 0;

  FUNCHECK(!WizR(context->world->database, player), "#-1 PERMISSION DENIED");
  FUNCHECK(strlen(fargs[0]) >= MBUF_SIZE, "#-1 PARTNAME TOO LONG");
  FUNCHECK(!fargs[0], "#-1 NEED PARTNAME");

  partindex = -1;
  while (find_matching_short_part(context->btech, fargs[0], &partindex, &id,
                                  &brand)) {
    safe_tprintf_str(buff, bufc, "%d ", PACKED_PART(id, brand));
    part_count++;
  }

  partindex = 0;
  while (find_matching_long_part(context->btech, fargs[0], &partindex, &id,
                                 &brand)) {
    safe_tprintf_str(buff, bufc, "%d ", PACKED_PART(id, brand));
    part_count++;
  }

  partindex = -1;
  while (find_matching_vlong_part(context->btech, fargs[0], &partindex, &id,
                                  &brand)) {
    safe_tprintf_str(buff, bufc, "%d ", PACKED_PART(id, brand));
    part_count++;
  }

  if (part_count == 0)
    safe_tprintf_str(buff, bufc, "#-1 INVALID PARTNAME");
}

/* Categories accepted by btpartslist(), based on the canonical part ID. */
typedef enum bt_part_category BT_PART_CATEGORY;
enum bt_part_category {
  BT_PART_CATEGORY_AMMO,
  BT_PART_CATEGORY_WEAPON,
  BT_PART_CATEGORY_BOMB,
  BT_PART_CATEGORY_SPECIAL,
  BT_PART_CATEGORY_CARGO,
  BT_PART_CATEGORY_INVALID
};

/* Convert a user-facing category name into the corresponding part category. */
static BT_PART_CATEGORY btpartslist_category(const char *category) {
  if (!strcasecmp(category, "ammo"))
    return BT_PART_CATEGORY_AMMO;
  if (!strcasecmp(category, "weapon") || !strcasecmp(category, "weapons") ||
      !strcasecmp(category, "weap"))
    return BT_PART_CATEGORY_WEAPON;
  if (!strcasecmp(category, "bomb") || !strcasecmp(category, "bombs"))
    return BT_PART_CATEGORY_BOMB;
  if (!strcasecmp(category, "special") || !strcasecmp(category, "specials") ||
      !strcasecmp(category, "part") || !strcasecmp(category, "parts"))
    return BT_PART_CATEGORY_SPECIAL;
  if (!strcasecmp(category, "cargo") || !strcasecmp(category, "carg"))
    return BT_PART_CATEGORY_CARGO;
  return BT_PART_CATEGORY_INVALID;
}

/* Return whether a canonical part ID belongs in the requested category. */
static int btpartslist_matches(BT_PART_CATEGORY category, int part) {
  switch (category) {
  case BT_PART_CATEGORY_AMMO:
    return IsAmmo(part);
  case BT_PART_CATEGORY_WEAPON:
    return IsWeapon(part);
  case BT_PART_CATEGORY_BOMB:
    return IsBomb(part);
  case BT_PART_CATEGORY_SPECIAL:
    return IsSpecial(part);
  case BT_PART_CATEGORY_CARGO:
    return IsCargo(part);
  default:
    return 0;
  }
}

/* List the canonical category names accepted by btpartslist(). */
void fun_btpartscategorylist(char *buff, char **bufc, DbRef player, DbRef cause,
                             char *fargs[], int nfargs, char *cargs[],
                             int ncargs, EvaluationContext *context) {
  FUNCHECK(!WizR(context->world->database, player), "#-1 PERMISSION DENIED");
  safe_str("ammo|weapon|bomb|special|cargo", buff, bufc);
}

/*
 * Return canonical long part names from one category, separated by pipes.
 * Requiring the category keeps this softcode result within one LBUF.
 */
void fun_btpartslist(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  BT_PART_CATEGORY category;
  PN *part_name;
  size_t used;
  size_t needed;
  int index;
  int part;
  int listed;
  PartNameRegistry *registry = context->btech->part_names;

  FUNCHECK(!WizR(context->world->database, player), "#-1 PERMISSION DENIED");
  FUNCHECK(nfargs != 1, "#-1 EXPECTS ONE CATEGORY ARGUMENT");

  category = btpartslist_category(fargs[0]);
  FUNCHECK(category == BT_PART_CATEGORY_INVALID,
           "#-1 CATEGORY MUST BE AMMO, WEAPON, BOMB, SPECIAL, OR CARGO");

  listed = 0;
  for (index = 0; index < registry->object_count; index++) {
    part_name = registry->long_sorted[index];
    part = part_name->index % NUM_ITEMS;
    if (!btpartslist_matches(category, part))
      continue;

    used = (size_t)(*bufc - buff);
    needed = strlen(part_name->longy) + (listed ? 1 : 0);
    if (used + needed >= LBUF_SIZE) {
      *bufc = buff;
      safe_str("#-1 LIST TOO LONG FOR THIS CATEGORY", buff, bufc);
      return;
    }
    if (listed)
      safe_str("|", buff, bufc);
    safe_str(part_name->longy, buff, bufc);
    listed++;
  }

  if (!listed)
    safe_str("#-1 NO PARTS IN CATEGORY", buff, bufc);
}

void fun_btpartname(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /* fargs[0] = partnumer to find name for
   * fargs[1] = 'short', 'long' or 'vlong'
   */

  int index;
  char *cptr;
  const char *infostr;

  FUNCHECK(!WizR(context->world->database, player), "#-1 PERMISSION DENIED");
  FUNCHECK(!fargs[0], "#-1 NEED PARTNAME");
  index = strtol(fargs[0], &cptr, 10);
  FUNCHECK(cptr == fargs[0], "#-1 INVALID PART NUMBER");

  infostr = partname_func(context->btech, index, fargs[1][0]);
  safe_tprintf_str(buff, bufc, "%s", infostr);
}

const char *partname_func(BtechContext *context, int index, int size) {
  PartNameRegistry *registry = context->part_names;
  int id, brand;
  PN *p;

  UNPACK_PART(index, id, brand);
  if (brand < 0 || brand > BRANDCOUNT || id < 0 || id >= NUM_ITEMS)
    return "#-1 INVALID PART NUMBER";

  p = registry->index_sorted[brand][id];
  if (!p)
    return "#-1 INVALID PART NUMBER";

  switch (size) {
  case 's':
  case 'S':
    return p->shorty;
  case 'l':
  case 'L':
    return p->longy;
  case 'v':
  case 'V':
    return p->vlongy;
  default:
    return "#-1 INVALID NAME TYPE";
  }
}

size_t part_name_count(const BtechContext *context) {
  return (size_t)context->part_names->object_count;
}

const PN *part_name_at(const BtechContext *context, size_t index) {
  if (index >= part_name_count(context))
    return nullptr;
  return context->part_names->short_sorted[index];
}

void destroy_partname_tables(BtechContext *context) {
  PartNameRegistry *registry = context->part_names;

  if (registry == nullptr)
    return;
  hash_table_destroy(&registry->short_hash);
  hash_table_destroy(&registry->vlong_hash);
  for (int brand = 0; brand <= BRANDCOUNT; brand++)
    for (int id = 0; id < NUM_ITEMS; id++) {
      PN *part_name = registry->index_sorted[brand][id];

      if (part_name == nullptr)
        continue;
      free(part_name->shorty);
      free(part_name->longy);
      free(part_name->vlongy);
      free(part_name);
    }
  free(registry->short_sorted);
  free(registry->long_sorted);
  free(registry->vlong_sorted);
  free(registry);
  context->part_names = nullptr;
}
