/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <stdio.h>
#include <string.h>
#include <strings.h>

#include "muxevent/muxevent_alloc.h"
#include "glue.h"
#include "mech.h"
#include "mech.partnames.h"
#include "rbtab.h"

void list_hashstat(dbref player, const char *tab_name, HASHTAB *htab);

/* Main idea:
   Keep 2 sorted tables, one of shortform -> index
   longform  -> index
   vlongform -> index
   Other
   index -> {short,long,vlong} form

   Index = ID + NUM_ITEMS * brand
 */

#define BRANDCOUNT 5
static PN *index_sorted[BRANDCOUNT + 1][NUM_ITEMS];

/* Sorted: short -> index, long -> index */
PN **short_sorted = NULL;
PN **long_sorted = NULL;
PN **vlong_sorted = NULL;
int object_count;

static void insert_sorted_brandname(int ind, PN *e) {
  int i, j;

#define UGLY_SORT(b, a)                                                        \
  for (i = 0; i < ind; i++)                                                    \
    if (strcmp(e->a, b[i]->a) < 0)                                             \
      break;                                                                   \
  for (j = ind; j > i; j--)                                                    \
    b[j] = b[j - 1];                                                           \
  b[i] = e
  UGLY_SORT(short_sorted, shorty);
  UGLY_SORT(long_sorted, longy);
  UGLY_SORT(vlong_sorted, vlongy);
}

extern char *part_figure_out_name(int i);
extern char *part_figure_out_sname(int i);
extern char *part_figure_out_shname(int i);
extern char *GetPartBrandName(int, int);
extern char *my_shortform(char *);

int temp_brand_flag;

static int create_brandname(int id, int b) {
  char buf[LBUF_SIZE];
  char buf2[MBUF_SIZE];
  char buf3[MBUF_SIZE];
  char *c, *brn = NULL;
  PN *p;

  if (b)
    if (!(brn = GetPartBrandName(id, b)))
      return 0;
  temp_brand_flag = b;
  Create(p, PN, 1);
/* \todo Remove this stupid #define and make the code readable */
#define SILLINESS(fun, val, fl)                                                \
  if (!(c = fun(id))) {                                                        \
    free((void *)p->val);                                                      \
    free((void *)p);                                                           \
    return 0;                                                                  \
  }                                                                            \
  if (b) {                                                                     \
    strcpy(buf2, c);                                                           \
    if (fl)                                                                    \
      strcpy(buf3, my_shortform(brn));                                         \
    snprintf(buf, sizeof(buf), "%s.%s", fl ? buf3 : brn, buf2);                \
  } else                                                                       \
    strcpy(buf, c);                                                            \
  p->val = strdup(buf)
  SILLINESS(part_figure_out_name, vlongy, 0);
  SILLINESS(part_figure_out_sname, longy, 0);
  SILLINESS(part_figure_out_shname, shorty, 1);
  p->index = PACKED_PART(id, b);
  index_sorted[b][id] = p;
  return 1;
}

static HASHTAB short_hash, vlong_hash;

void initialize_partname_tables() {
  long i;
  int j, c = 0, m, n;
  char tmpbuf[MBUF_SIZE];
  char *tmpc1, *tmpc2;

  bzero(index_sorted, sizeof(index_sorted));
  for (j = 0; j <= BRANDCOUNT; j++)
    for (i = 0; i < NUM_ITEMS; i++)
      c += create_brandname(i, j);
  Create(short_sorted, PN *, c);
  Create(long_sorted, PN *, c);
  Create(vlong_sorted, PN *, c);
  /* bubble-sort 'em and insert to array */
  i = 0;
  for (m = 0; m <= BRANDCOUNT; m++)
    for (n = 0; n < NUM_ITEMS; n++)
      if (index_sorted[m][n])
        insert_sorted_brandname(i++, index_sorted[m][n]);
  hashinit(&short_hash, 20 * HASH_FACTOR);
  hashinit(&vlong_hash, 20 * HASH_FACTOR);
#define DASH(fromval, tohash)                                                  \
  for (tmpc1 = short_sorted[i]->fromval, tmpc2 = tmpbuf; *tmpc1;               \
       tmpc1++, tmpc2++)                                                       \
    *tmpc2 = ToLower(*tmpc1);                                                  \
  *tmpc2 = 0;                                                                  \
  hashadd(tmpbuf, (int *)(i + 1), &tohash);

  for (i = 0; i < c; i++) {
    DASH(shorty, short_hash);

    /*       DASH(longy, long_hash); */
    DASH(vlongy, vlong_hash);
  }
  object_count = c;
}

#define SILLY_GET(fun, value)                                                  \
  char *fun(int i, int b) {                                                    \
    if (!(index_sorted[b][i])) {                                               \
      if (b)                                                                   \
        return fun(i, 0);                                                      \
      else {                                                                   \
        SendError(tprintf("No index for %d/%d", i, b));                        \
        return NULL;                                                           \
      }                                                                        \
    }                                                                          \
    return index_sorted[b][i]->value;                                          \
  }

SILLY_GET(get_parts_short_name, shorty);
SILLY_GET(get_parts_long_name, longy);
SILLY_GET(get_parts_vlong_name, vlongy);

#define wildcard_match quick_wild
extern int wildcard_match(char *, char *);

int find_matching_vlong_part(char *wc, int *ind, int *id, int *brand) {
  PN *p;
  char *tmpc1, *tmpc2;
  char tmpbuf[MBUF_SIZE];
  int *i;

  if (ind && *ind >= 0)
    return 0;
  for (tmpc1 = wc, tmpc2 = tmpbuf; *tmpc1; tmpc1++, tmpc2++) {
    *tmpc2 = ToLower(*tmpc1);
  }
  *tmpc2 = 0;
  if ((i = hashfind(tmpbuf, &vlong_hash))) {
    if ((p = short_sorted[((long)i) - 1])) {
      if (ind)
        *ind = ((long)i);
      UNPACK_PART(p->index, *id, *brand);
      return 1;
    }
  }
  return 0;
}

int find_matching_long_part(char *wc, int *i, int *id, int *brand) {
  PN *p;

  for ((*i)++; *i < object_count; (*i)++)
    if (wildcard_match(wc, (p = long_sorted[*i])->longy)) {
      UNPACK_PART(p->index, *id, *brand);
      return 1;
    }
  return 0;
}

int find_matching_short_part(char *wc, int *ind, int *id, int *brand) {
  PN *p;
  char *tmpc1, *tmpc2;
  char tmpbuf[MBUF_SIZE];
  int *i;

  if (*ind >= 0)
    return 0;
  for (tmpc1 = wc, tmpc2 = tmpbuf; *tmpc1; tmpc1++, tmpc2++) {
    *tmpc2 = ToLower(*tmpc1);
  }
  *tmpc2 = 0;
  if ((i = hashfind(tmpbuf, &short_hash))) {
    if ((p = short_sorted[((long)i) - 1])) {
      *ind = ((long)i);
      UNPACK_PART(p->index, *id, *brand);
      return 1;
    }
  }
  return 0;
}

void ListForms(dbref player, void *data, char *buffer) {
  int i;

  notify(player, "Listing of forms:");
  for (i = 0; i < object_count; i++)
    notify_printf(player, "%3d %-20s %-25s %s", i, short_sorted[i]->shorty,
                  short_sorted[i]->longy, short_sorted[i]->vlongy);
}

void fun_btpartmatch(char *buff, char **bufc, dbref player, dbref cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs) {
  /* fargs[0] = name to match on
   */

  /* Added check to see if anything was found, if not
   * send error message
   * Dany - 06/2005
   */

  int partindex = 0, id = 0, brand = 0;
  int part_count = 0;

  FUNCHECK(!WizR(player), "#-1 PERMISSION DENIED");
  FUNCHECK(strlen(fargs[0]) >= MBUF_SIZE, "#-1 PARTNAME TOO LONG");
  FUNCHECK(!fargs[0], "#-1 NEED PARTNAME");

  partindex = -1;
  while (find_matching_short_part(fargs[0], &partindex, &id, &brand)) {
    safe_tprintf_str(buff, bufc, "%d ", PACKED_PART(id, brand));
    part_count++;
  }

  partindex = 0;
  while (find_matching_long_part(fargs[0], &partindex, &id, &brand)) {
    safe_tprintf_str(buff, bufc, "%d ", PACKED_PART(id, brand));
    part_count++;
  }

  partindex = -1;
  while (find_matching_vlong_part(fargs[0], &partindex, &id, &brand)) {
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
void fun_btpartscategorylist(char *buff, char **bufc, dbref player,
                             dbref cause, char *fargs[], int nfargs,
                             char *cargs[], int ncargs) {
  FUNCHECK(!WizR(player), "#-1 PERMISSION DENIED");
  safe_str("ammo|weapon|bomb|special|cargo", buff, bufc);
}

/*
 * Return canonical long part names from one category, separated by pipes.
 * Requiring the category keeps this softcode result within one LBUF.
 */
void fun_btpartslist(char *buff, char **bufc, dbref player, dbref cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs) {
  BT_PART_CATEGORY category;
  PN *part_name;
  size_t used;
  size_t needed;
  int index;
  int part;
  int brand;
  int listed;

  FUNCHECK(!WizR(player), "#-1 PERMISSION DENIED");
  FUNCHECK(nfargs != 1, "#-1 EXPECTS ONE CATEGORY ARGUMENT");

  category = btpartslist_category(fargs[0]);
  FUNCHECK(category == BT_PART_CATEGORY_INVALID,
           "#-1 CATEGORY MUST BE AMMO, WEAPON, BOMB, SPECIAL, OR CARGO");

  listed = 0;
  for (index = 0; index < object_count; index++) {
    part_name = long_sorted[index];
    UNPACK_PART(part_name->index, part, brand);
    if (!btpartslist_matches(category, part))
      continue;

    used = (size_t)(*bufc - buff);
    needed = strlen(part_name->longy) + (listed ? 1 : 0);
    if (used + needed >= LBUF_SIZE) {
      *bufc = buff;
      safe_str("#-1 LIST TOO LONG FOR THIS CATEGORY",
               buff, bufc);
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

void fun_btpartname(char *buff, char **bufc, dbref player, dbref cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs) {
  /* fargs[0] = partnumer to find name for
   * fargs[1] = 'short', 'long' or 'vlong'
   */

  int index;
  char *cptr;
  char *infostr;

  FUNCHECK(!WizR(player), "#-1 PERMISSION DENIED");
  FUNCHECK(!fargs[0], "#-1 NEED PARTNAME");
  index = strtol(fargs[0], &cptr, 10);
  FUNCHECK(cptr == fargs[0], "#-1 INVALID PART NUMBER");

  infostr = partname_func(index, fargs[1][0]);
  safe_tprintf_str(buff, bufc, "%s", infostr);
}

char *partname_func(int index, int size) {

  static char buffer[MBUF_SIZE];
  int id, brand;
  PN *p;

  UNPACK_PART(index, id, brand);
  if (brand < 0 || brand > BRANDCOUNT || id < 0) {
    snprintf(buffer, MBUF_SIZE, "%s", "#-1 INVALID PART NUMBER");
    return buffer;
  }

  p = index_sorted[brand][id];
  if (!p) {
    snprintf(buffer, MBUF_SIZE, "%s", "#-1 INVALID PART NUMBER");
    return buffer;
  }

  switch (size) {
  case 's':
  case 'S':
    snprintf(buffer, MBUF_SIZE, "%s", p->shorty);
    break;
  case 'l':
  case 'L':
    snprintf(buffer, MBUF_SIZE, "%s", p->longy);
    break;
  case 'v':
  case 'V':
    snprintf(buffer, MBUF_SIZE, "%s", p->vlongy);
    break;
  default:
    snprintf(buffer, MBUF_SIZE, "%s", "#-1 INVALID NAME TYPE");
    break;
  }

  return buffer;
}
