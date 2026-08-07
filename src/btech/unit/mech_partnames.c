#include "mux/objects/flags.h"
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

#include "btech/context.h"
#include "mech_internal.h"
#include "mech_partnames.h"
#include "mech_partnames_api.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "special_object.h"
#include "template_api.h"

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
  PartNameEntry *index_sorted[BRANDCOUNT + 1][NUM_ITEMS];
  PartNameEntry **short_sorted;
  PartNameEntry **long_sorted;
  PartNameEntry **vlong_sorted;
  int object_count;
  HashTable short_hash;
  HashTable vlong_hash;
};

typedef enum PartNameField {
  PART_NAME_SHORT,
  PART_NAME_LONG,
  PART_NAME_VERY_LONG,
} PartNameField;

static const char *part_name_field(const PartNameEntry *entry,
                                   PartNameField field) {
  switch (field) {
  case PART_NAME_SHORT:
    return entry->shorty;
  case PART_NAME_LONG:
    return entry->longy;
  case PART_NAME_VERY_LONG:
    return entry->vlongy;
  }
  return "";
}

static void insert_sorted_name(PartNameEntry **entries, int count,
                               PartNameEntry *entry, PartNameField field) {
  int insertion = 0;

  while (insertion < count &&
         strcmp(part_name_field(entry, field),
                part_name_field(entries[insertion], field)) >= 0)
    insertion++;
  for (int index = count; index > insertion; index--)
    entries[index] = entries[index - 1];
  entries[insertion] = entry;
}

static void insert_sorted_brandname(PartNameRegistry *registry, int count,
                                    PartNameEntry *entry) {
  insert_sorted_name(registry->short_sorted, count, entry, PART_NAME_SHORT);
  insert_sorted_name(registry->long_sorted, count, entry, PART_NAME_LONG);
  insert_sorted_name(registry->vlong_sorted, count, entry, PART_NAME_VERY_LONG);
}

extern const char *mech_part_brand_name(int, int);

static int create_brandname(PartNameRegistry *registry,
                            const ServerConfiguration *configuration, int id,
                            int b) {
  char buf[LBUF_SIZE];
  char buf2[BTECH_TEXT_CAPACITY];
  char buf3[BTECH_TEXT_CAPACITY];
  char *c;
  const char *brn = nullptr;
  PartNameEntry *p;

  if (b)
    if (!(brn = mech_part_brand_name(id, b)))
      return 0;
  Create(p, PartNameEntry, 1);
  c = part_figure_out_name(configuration, id, b,
                           (char[BTECH_TEXT_CAPACITY]){0});
  if (!c) {
    free(p);
    return 0;
  }
  if (b)
    snprintf(buf, sizeof(buf), "%s.%s", brn, c);
  else
    snprintf(buf, sizeof(buf), "%s", c);
  p->vlongy = strdup(buf);

  c = part_figure_out_sname(configuration, id, b,
                            (char[BTECH_TEXT_CAPACITY]){0});
  if (!c) {
    free(p->vlongy);
    free(p);
    return 0;
  }
  if (b)
    snprintf(buf, sizeof(buf), "%s.%s", brn, c);
  else
    snprintf(buf, sizeof(buf), "%s", c);
  p->longy = strdup(buf);
  if (!(c = part_figure_out_shname(id, (char[BTECH_TEXT_CAPACITY]){0}))) {
    free(p->longy);
    free(p->vlongy);
    free(p);
    return 0;
  }
  if (b) {
    strcpy(buf2, c);
    strcpy(buf3, my_shortform(brn, (char[BTECH_TEXT_CAPACITY]){0}));
    snprintf(buf, sizeof(buf), "%s.%s", buf3, buf2);
  } else
    strcpy(buf, c);
  p->shorty = strdup(buf);
  p->index = packed_part(id, b);
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
  Create(registry->short_sorted, PartNameEntry *, c);
  Create(registry->long_sorted, PartNameEntry *, c);
  Create(registry->vlong_sorted, PartNameEntry *, c);
  /* bubble-sort 'em and insert to array */
  i = 0;
  for (m = 0; m <= BRANDCOUNT; m++)
    for (n = 0; n < NUM_ITEMS; n++)
      if (registry->index_sorted[m][n])
        insert_sorted_brandname(registry, i++, registry->index_sorted[m][n]);
  hash_table_initialize(&registry->short_hash, 20 * HASH_FACTOR);
  hash_table_initialize(&registry->vlong_hash, 20 * HASH_FACTOR);
  for (i = 0; i < c; i++) {
    for (tmpc1 = registry->short_sorted[i]->shorty, tmpc2 = tmpbuf; *tmpc1;
         tmpc1++, tmpc2++)
      *tmpc2 = ascii_to_lower(*tmpc1);
    *tmpc2 = 0;
    hash_table_add(tmpbuf, (int *)(i + 1), &registry->short_hash);

    for (tmpc1 = registry->short_sorted[i]->vlongy, tmpc2 = tmpbuf; *tmpc1;
         tmpc1++, tmpc2++)
      *tmpc2 = ascii_to_lower(*tmpc1);
    *tmpc2 = 0;
    hash_table_add(tmpbuf, (int *)(i + 1), &registry->vlong_hash);
  }
  registry->object_count = c;
}

static char *get_part_name(BtechContext *context, int id, int brand,
                           PartNameField field) {
  PartNameRegistry *registry = context->part_names;

  if (id < 0 || id >= NUM_ITEMS || brand < 0 || brand > BRANDCOUNT)
    return nullptr;
  PartNameEntry *entry = registry->index_sorted[brand][id];
  if (!entry && brand)
    entry = registry->index_sorted[0][id];
  return entry ? (char *)part_name_field(entry, field) : nullptr;
}

char *get_parts_short_name(BtechContext *context, int id, int brand) {
  return get_part_name(context, id, brand, PART_NAME_SHORT);
}

char *get_parts_long_name(BtechContext *context, int id, int brand) {
  return get_part_name(context, id, brand, PART_NAME_LONG);
}

char *get_parts_vlong_name(BtechContext *context, int id, int brand) {
  return get_part_name(context, id, brand, PART_NAME_VERY_LONG);
}

#define wildcard_match quick_wild
extern int wildcard_match(const char *, const char *);

int find_matching_vlong_part(BtechContext *context, const char *wc, int *ind,
                             int *id, int *brand) {
  PartNameRegistry *registry = context->part_names;
  PartNameEntry *p;
  const char *tmpc1;
  char *tmpc2;
  char tmpbuf[MBUF_SIZE];
  int *i;

  if (ind && *ind >= 0)
    return 0;
  for (tmpc1 = wc, tmpc2 = tmpbuf; *tmpc1; tmpc1++, tmpc2++) {
    *tmpc2 = ascii_to_lower(*tmpc1);
  }
  *tmpc2 = 0;
  if ((i = hash_table_find(tmpbuf, &registry->vlong_hash))) {
    if ((p = registry->short_sorted[((long)i) - 1])) {
      if (ind)
        *ind = ((long)i);
      *id = packed_part_id(p->index);
      *brand = packed_part_brand(p->index);
      return 1;
    }
  }
  return 0;
}

int find_matching_long_part(BtechContext *context, const char *wc, int *i,
                            int *id, int *brand) {
  PartNameRegistry *registry = context->part_names;
  PartNameEntry *p;

  for ((*i)++; *i < registry->object_count; (*i)++)
    if (wildcard_match(wc, (p = registry->long_sorted[*i])->longy)) {
      *id = packed_part_id(p->index);
      *brand = packed_part_brand(p->index);
      return 1;
    }
  return 0;
}

int find_matching_short_part(BtechContext *context, const char *wc, int *ind,
                             int *id, int *brand) {
  PartNameRegistry *registry = context->part_names;
  PartNameEntry *p;
  const char *tmpc1;
  char *tmpc2;
  char tmpbuf[MBUF_SIZE];
  int *i;

  if (*ind >= 0)
    return 0;
  for (tmpc1 = wc, tmpc2 = tmpbuf; *tmpc1; tmpc1++, tmpc2++) {
    *tmpc2 = ascii_to_lower(*tmpc1);
  }
  *tmpc2 = 0;
  if ((i = hash_table_find(tmpbuf, &registry->short_hash))) {
    if ((p = registry->short_sorted[((long)i) - 1])) {
      *ind = ((long)i);
      *id = packed_part_id(p->index);
      *brand = packed_part_brand(p->index);
      return 1;
    }
  }
  return 0;
}

void ListForms(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  PartNameRegistry *registry = debug->context->part_names;
  int i;

  mecha_notify(btech_context_evaluation(debug->context), player,
               "Listing of forms:");
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

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (strlen(fargs[0]) >= MBUF_SIZE) {
    safe_tprintf_str(buff, bufc, "#-1 PARTNAME TOO LONG");
    return;
  }
  if (!fargs[0]) {
    safe_tprintf_str(buff, bufc, "#-1 NEED PARTNAME");
    return;
  }

  partindex = -1;
  while (find_matching_short_part(context->btech, fargs[0], &partindex, &id,
                                  &brand)) {
    safe_tprintf_str(buff, bufc, "%d ", packed_part(id, brand));
    part_count++;
  }

  partindex = 0;
  while (find_matching_long_part(context->btech, fargs[0], &partindex, &id,
                                 &brand)) {
    safe_tprintf_str(buff, bufc, "%d ", packed_part(id, brand));
    part_count++;
  }

  partindex = -1;
  while (find_matching_vlong_part(context->btech, fargs[0], &partindex, &id,
                                  &brand)) {
    safe_tprintf_str(buff, bufc, "%d ", packed_part(id, brand));
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
    return equipment_is_ammunition(part);
  case BT_PART_CATEGORY_WEAPON:
    return equipment_is_weapon(part);
  case BT_PART_CATEGORY_BOMB:
    return equipment_is_bomb(part);
  case BT_PART_CATEGORY_SPECIAL:
    return equipment_is_special(part);
  case BT_PART_CATEGORY_CARGO:
    return equipment_is_cargo(part);
  default:
    return 0;
  }
}

/* List the canonical category names accepted by btpartslist(). */
void fun_btpartscategorylist(char *buff, char **bufc, DbRef player, DbRef cause,
                             char *fargs[], int nfargs, char *cargs[],
                             int ncargs, EvaluationContext *context) {
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  safe_str("ammo|weapon|bomb|special|cargo", buff, bufc);
}

/*
 * Return canonical long part names from one category, separated by pipes.
 * Requiring the category keeps the result within one LBUF.
 */
void fun_btpartslist(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  BT_PART_CATEGORY category;
  PartNameEntry *part_name;
  size_t used;
  size_t needed;
  int index;
  int part;
  int listed;
  PartNameRegistry *registry = context->btech->part_names;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (nfargs != 1) {
    safe_tprintf_str(buff, bufc, "#-1 EXPECTS ONE CATEGORY ARGUMENT");
    return;
  }

  category = btpartslist_category(fargs[0]);
  if (category == BT_PART_CATEGORY_INVALID) {
    safe_tprintf_str(
        buff, bufc,
        "#-1 CATEGORY MUST BE AMMO, WEAPON, BOMB, SPECIAL, OR CARGO");
    return;
  }

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

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (!fargs[0]) {
    safe_tprintf_str(buff, bufc, "#-1 NEED PARTNAME");
    return;
  }
  index = strtol(fargs[0], &cptr, 10);
  if (cptr == fargs[0]) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID PART NUMBER");
    return;
  }

  infostr = partname_func(context->btech, index, fargs[1][0]);
  safe_tprintf_str(buff, bufc, "%s", infostr);
}

const char *partname_func(BtechContext *context, int index, int size) {
  PartNameRegistry *registry = context->part_names;
  int id, brand;
  PartNameEntry *p;

  id = packed_part_id(index);
  brand = packed_part_brand(index);
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

const PartNameEntry *part_name_at(const BtechContext *context, size_t index) {
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
      PartNameEntry *part_name = registry->index_sorted[brand][id];

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
