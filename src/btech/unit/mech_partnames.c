#include "context_internal.h"
#include "equipment_types.h"
#include "mux/objects/flags.h"
/* Implements BattleTech unit mechanics for unit partnames. */

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btech/context.h"
#include "checked_conversion.h"
#include "failures_api.h"
#include "mech_partnames.h"
#include "mech_partnames_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"
#include "mux/support/stringutil.h"
#include "mux/support/wild.h"
#include "registry_api.h"
#include "script_functions_api.h"
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

static PartNameEntry **part_index_slot(PartNameRegistry *registry, int brand,
                                       int id) {
  if (brand < 0 || brand > BRANDCOUNT || id < 0 || id >= NUM_ITEMS)
    abort();
  size_t index = (size_t)brand * NUM_ITEMS + (size_t)id;
  return (PartNameEntry **)checked_storage_at(
      registry->index_sorted, ((size_t)BRANDCOUNT + 1U) * (size_t)NUM_ITEMS,
      sizeof(PartNameEntry *), index);
}

static PartNameEntry *part_index_entry(PartNameRegistry *registry, int brand,
                                       int id) {
  return *part_index_slot(registry, brand, id);
}

static PartNameEntry **sorted_slot(PartNameEntry **entries, size_t count,
                                   size_t index) {
  return (PartNameEntry **)checked_storage_at((void *)entries, count,
                                              sizeof(*entries), index);
}

static PartNameEntry *sorted_entry(PartNameEntry **entries, size_t count,
                                   size_t index) {
  return *sorted_slot(entries, count, index);
}

static void lowercase_name(char output[static MBUF_SIZE], const char *name) {
  size_t length = strlen(name);
  for (size_t index = 0; index < length; index++) {
    const char *source =
        checked_storage_at_const(name, length + 1, sizeof(*name), index);
    char *destination =
        checked_storage_at(output, MBUF_SIZE, sizeof(*output), index);
    *destination = ascii_to_lower(*source);
  }
  char *terminator =
      checked_storage_at(output, MBUF_SIZE, sizeof(*output), length);
  *terminator = '\0';
}

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

static void insert_sorted_name(PartNameEntry **entries, size_t capacity,
                               int count, PartNameEntry *entry,
                               PartNameField field) {
  int insertion = 0;

  while (
      insertion < count &&
      strcmp(part_name_field(entry, field),
             part_name_field(sorted_entry(entries, capacity, (size_t)insertion),
                             field)) >= 0)
    insertion++;
  for (int index = count; index > insertion; index--)
    *sorted_slot(entries, capacity, (size_t)index) =
        sorted_entry(entries, capacity, (size_t)(index - 1));
  *sorted_slot(entries, capacity, (size_t)insertion) = entry;
}

static void insert_sorted_brandname(PartNameRegistry *registry, int count,
                                    PartNameEntry *entry) {
  size_t capacity = (size_t)registry->object_count;
  insert_sorted_name(registry->short_sorted, capacity, count, entry,
                     PART_NAME_SHORT);
  insert_sorted_name(registry->long_sorted, capacity, count, entry,
                     PART_NAME_LONG);
  insert_sorted_name(registry->vlong_sorted, capacity, count, entry,
                     PART_NAME_VERY_LONG);
}

static int create_brandname(PartNameRegistry *registry,
                            const ServerConfiguration *configuration, int id,
                            int b) {
  char buf[LBUF_SIZE];
  char buf2[BTECH_TEXT_CAPACITY];
  char buf3[BTECH_TEXT_CAPACITY];
  char *c;
  const char *brn = nullptr;
  PartNameEntry *p;

  if (b) {
    PartBrandRequest request = {
        .equipment_type = id,
        .quality_level = b,
    };
    brn = mech_part_brand_name(&request);
  }
  if (!brn)
    return 0;
  p = checked_storage_allocate(sizeof(*p));
  c = part_name_format(
      &(PartNameRequest){.configuration = configuration,
                         .part = id,
                         .brand = b,
                         .buffer = (char[BTECH_TEXT_CAPACITY]){0}});
  if (!c) {
    free(p);
    return 0;
  }
  if (b)
    (void)snprintf(buf, sizeof(buf), "%s.%s", brn, c);
  else
    (void)snprintf(buf, sizeof(buf), "%s", c);
  p->vlongy = strdup(buf);

  c = part_name_format(
      &(PartNameRequest){.configuration = configuration,
                         .part = id,
                         .brand = b,
                         .short_name = true,
                         .buffer = (char[BTECH_TEXT_CAPACITY]){0}});
  if (!c) {
    free(p->vlongy);
    free(p);
    return 0;
  }
  if (b)
    (void)snprintf(buf, sizeof(buf), "%s.%s", brn, c);
  else
    (void)snprintf(buf, sizeof(buf), "%s", c);
  p->longy = strdup(buf);
  c = part_figure_out_shname(id, (char[BTECH_TEXT_CAPACITY]){0});
  if (!c) {
    free(p->longy);
    free(p->vlongy);
    free(p);
    return 0;
  }
  if (b) {
    strlcpy(buf2, c, sizeof(buf2));
    strlcpy(buf3, my_shortform(brn, (char[BTECH_TEXT_CAPACITY]){0}),
            sizeof(buf3));
    (void)snprintf(buf, sizeof(buf), "%s.%s", buf3, buf2);
  } else
    strlcpy(buf, c, sizeof(buf));
  p->shorty = strdup(buf);
  p->index = packed_part(id, b);
  *part_index_slot(registry, b, id) = p;
  return 1;
}

void initialize_partname_tables(BtechContext *context) {
  const ServerConfiguration *configuration = context->configuration;
  PartNameRegistry *registry = calloc(1, sizeof(*registry));
  int i;
  int j, c = 0, m, n;
  char tmpbuf[MBUF_SIZE];

  if (registry == nullptr)
    exit(EXIT_FAILURE);
  context->part_names = registry;
  for (j = 0; j <= BRANDCOUNT; j++)
    for (i = 0; i < NUM_ITEMS; i++)
      c += create_brandname(registry, configuration, i, j);
  registry->short_sorted = (PartNameEntry **)checked_storage_allocate(
      sizeof(*registry->short_sorted) * (size_t)c);
  registry->long_sorted = (PartNameEntry **)checked_storage_allocate(
      sizeof(*registry->long_sorted) * (size_t)c);
  registry->vlong_sorted = (PartNameEntry **)checked_storage_allocate(
      sizeof(*registry->vlong_sorted) * (size_t)c);
  registry->object_count = c;
  /* bubble-sort 'em and insert to array */
  i = 0;
  for (m = 0; m <= BRANDCOUNT; m++)
    for (n = 0; n < NUM_ITEMS; n++)
      if (part_index_entry(registry, m, n))
        insert_sorted_brandname(registry, i++,
                                part_index_entry(registry, m, n));
  hash_table_initialize(&registry->short_hash, 20 * HASH_FACTOR);
  hash_table_initialize(&registry->vlong_hash, 20 * HASH_FACTOR);
  for (i = 0; i < c; i++) {
    PartNameEntry *entry =
        sorted_entry(registry->short_sorted, (size_t)c, (size_t)i);
    lowercase_name(tmpbuf, entry->shorty);
    hash_table_add(tmpbuf, (void *)(intptr_t)(i + 1), &registry->short_hash);

    lowercase_name(tmpbuf, entry->vlongy);
    hash_table_add(tmpbuf, (void *)(intptr_t)(i + 1), &registry->vlong_hash);
  }
}

typedef struct PartNameLookup {
  BtechContext *context;
  PartReference part;
  PartNameField field;
} PartNameLookup;

static const char *get_part_name(const PartNameLookup *lookup) {
  PartNameRegistry *registry = lookup->context->part_names;
  const int ID = lookup->part.id;
  const int BRAND = lookup->part.brand;

  if (ID < 0 || ID >= NUM_ITEMS || BRAND < 0 || BRAND > BRANDCOUNT)
    return nullptr;
  PartNameEntry *entry = part_index_entry(registry, BRAND, ID);
  if (!entry && BRAND)
    entry = part_index_entry(registry, 0, ID);
  return entry ? part_name_field(entry, lookup->field) : nullptr;
}

const char *get_parts_short_name(BtechContext *context, int id, int brand) {
  return get_part_name(&(PartNameLookup){.context = context,
                                         .part = {.id = id, .brand = brand},
                                         .field = PART_NAME_SHORT});
}

const char *get_parts_long_name(BtechContext *context, int id, int brand) {
  return get_part_name(&(PartNameLookup){.context = context,
                                         .part = {.id = id, .brand = brand},
                                         .field = PART_NAME_LONG});
}

const char *get_parts_vlong_name(BtechContext *context, int id, int brand) {
  return get_part_name(&(PartNameLookup){.context = context,
                                         .part = {.id = id, .brand = brand},
                                         .field = PART_NAME_VERY_LONG});
}

static PartMatchResult part_match_exact(const PartMatchRequest *request,
                                        HashTable *table,
                                        PartNameEntry **entries) {
  PartNameRegistry *registry = request->context->part_names;
  PartNameEntry *p;
  char tmpbuf[MBUF_SIZE];
  void *match;

  if (request->cursor >= 0)
    return (PartMatchResult){0};
  lowercase_name(tmpbuf, request->pattern);
  match = hash_table_find(tmpbuf, table);
  if (match) {
    intptr_t match_index = (intptr_t)match;
    p = sorted_entry(entries, (size_t)registry->object_count,
                     (size_t)(match_index - 1));
    if (p)
      return (PartMatchResult){.found = true,
                               .cursor = clamp_intptr_to_int(match_index),
                               .part = {.id = packed_part_id(p->index),
                                        .brand = packed_part_brand(p->index)}};
  }
  return (PartMatchResult){0};
}

static PartMatchResult part_match_long(const PartMatchRequest *request) {
  PartNameRegistry *registry = request->context->part_names;
  PartNameEntry *p;

  for (int cursor = request->cursor + 1; cursor < registry->object_count;
       cursor++) {
    p = sorted_entry(registry->long_sorted, (size_t)registry->object_count,
                     (size_t)cursor);
    if (quick_wild(request->pattern, p->longy))
      return (PartMatchResult){.found = true,
                               .cursor = cursor,
                               .part = {.id = packed_part_id(p->index),
                                        .brand = packed_part_brand(p->index)}};
  }
  return (PartMatchResult){0};
}

PartMatchResult part_match_next(const PartMatchRequest *request) {
  PartNameRegistry *registry = request->context->part_names;
  switch (request->kind) {
  case PART_MATCH_SHORT:
    return part_match_exact(request, &registry->short_hash,
                            registry->short_sorted);
  case PART_MATCH_LONG:
    return part_match_long(request);
  case PART_MATCH_VERY_LONG:
    return part_match_exact(request, &registry->vlong_hash,
                            registry->short_sorted);
  }
  return (PartMatchResult){0};
}

PartMatchResult part_name_lookup(const PartNameLookupRequest *request) {
  PartMatchResult match = part_match_next(&(PartMatchRequest){
      .context = request->context,
      .pattern = request->name,
      .kind = PART_MATCH_LONG,
      .cursor = -1,
  });

  if (match.found)
    return match;
  return part_match_next(&(PartMatchRequest){
      .context = request->context,
      .pattern = request->name,
      .kind = PART_MATCH_VERY_LONG,
      .cursor = -1,
  });
}

void list_forms(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  PartNameRegistry *registry = debug->context->part_names;
  int i;

  mecha_notify(btech_context_evaluation(debug->context), player,
               "Listing of forms:");
  for (i = 0; i < registry->object_count; i++) {
    PartNameEntry *entry = sorted_entry(
        registry->short_sorted, (size_t)registry->object_count, (size_t)i);
    notify_printf(btech_context_evaluation(debug->context), player,
                  "%3d %-20s %-25s %s", i, entry->shorty, entry->longy,
                  entry->vlongy);
  }
}

BtechScriptResult fun_btpartmatch(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* fargs[0] = name to match on
   */

  /* Added check to see if anything was found, if not
   * send error message
   * Dany - 06/2005
   */

  int part_count = 0;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  if (!fargs[0]) {
    safe_tprintf_str(buff, bufc, "#-1 NEED PARTNAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  if (strlen(fargs[0]) >= MBUF_SIZE) {
    safe_tprintf_str(buff, bufc, "#-1 PARTNAME TOO LONG");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }

  PartMatchRequest request = {
      .context = context->btech,
      .pattern = fargs[0],
      .kind = PART_MATCH_SHORT,
      .cursor = -1,
  };
  for (;;) {
    PartMatchResult match = part_match_next(&request);
    if (!match.found)
      break;
    request.cursor = match.cursor;
    safe_tprintf_str(buff, bufc, "%d ",
                     packed_part(match.part.id, match.part.brand));
    part_count++;
  }

  request.kind = PART_MATCH_LONG;
  request.cursor = 0;
  for (;;) {
    PartMatchResult match = part_match_next(&request);
    if (!match.found)
      break;
    request.cursor = match.cursor;
    safe_tprintf_str(buff, bufc, "%d ",
                     packed_part(match.part.id, match.part.brand));
    part_count++;
  }

  request.kind = PART_MATCH_VERY_LONG;
  request.cursor = -1;
  for (;;) {
    PartMatchResult match = part_match_next(&request);
    if (!match.found)
      break;
    request.cursor = match.cursor;
    safe_tprintf_str(buff, bufc, "%d ",
                     packed_part(match.part.id, match.part.brand));
    part_count++;
  }

  if (part_count == 0)
    safe_tprintf_str(buff, bufc, "#-1 INVALID PARTNAME");

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}

/* Categories accepted by btpartslist(), based on the canonical part ID. */
typedef enum BtPartCategory {
  BT_PART_CATEGORY_AMMO,
  BT_PART_CATEGORY_WEAPON,
  BT_PART_CATEGORY_BOMB,
  BT_PART_CATEGORY_SPECIAL,
  BT_PART_CATEGORY_CARGO,
  BT_PART_CATEGORY_INVALID
} BtPartCategory;

/* Convert a user-facing category name into the corresponding part category. */
static BtPartCategory btpartslist_category(const char *category) {
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
typedef struct PartCategoryRequest {
  BtPartCategory category;
  int part;
} PartCategoryRequest;

static bool btpartslist_matches(const PartCategoryRequest *request) {
  switch (request->category) {
  case BT_PART_CATEGORY_AMMO:
    return equipment_is_ammunition(request->part);
  case BT_PART_CATEGORY_WEAPON:
    return equipment_is_weapon(request->part);
  case BT_PART_CATEGORY_BOMB:
    return equipment_is_bomb(request->part);
  case BT_PART_CATEGORY_SPECIAL:
    return equipment_is_special(request->part);
  case BT_PART_CATEGORY_CARGO:
    return equipment_is_cargo(request->part);
  case BT_PART_CATEGORY_INVALID:
    return 0;
  default:
    return 0;
  }
}

/* List the canonical category names accepted by btpartslist(). */
BtechScriptResult fun_btpartscategorylist(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  safe_str("ammo|weapon|bomb|special|cargo", buff, bufc);

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}

/*
 * Return canonical long part names from one category, separated by pipes.
 * Requiring the category keeps the result within one LBUF.
 */
BtechScriptResult fun_btpartslist(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  BtPartCategory category;
  PartNameEntry *part_name;
  size_t used;
  size_t needed;
  int index;
  int part;
  int listed;
  PartNameRegistry *registry = context->btech->part_names;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  if (NFARGS != 1) {
    safe_tprintf_str(buff, bufc, "#-1 EXPECTS ONE CATEGORY ARGUMENT");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }

  category = btpartslist_category(fargs[0]);
  if (category == BT_PART_CATEGORY_INVALID) {
    safe_tprintf_str(
        buff, bufc,
        "#-1 CATEGORY MUST BE AMMO, WEAPON, BOMB, SPECIAL, OR CARGO");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }

  listed = 0;
  for (index = 0; index < registry->object_count; index++) {
    part_name = sorted_entry(registry->long_sorted,
                             (size_t)registry->object_count, (size_t)index);
    part = part_name->index % NUM_ITEMS;
    if (!btpartslist_matches(
            &(PartCategoryRequest){.category = category, .part = part}))
      continue;

    used = (size_t)(*bufc - buff);
    needed = strlen(part_name->longy) + (listed ? 1 : 0);
    if (used + needed >= LBUF_SIZE) {
      *bufc = buff;
      safe_str("#-1 LIST TOO LONG FOR THIS CATEGORY", buff, bufc);
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    if (listed)
      safe_str("|", buff, bufc);
    safe_str(part_name->longy, buff, bufc);
    listed++;
  }

  if (!listed)
    safe_str("#-1 NO PARTS IN CATEGORY", buff, bufc);

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}

BtechScriptResult fun_btpartname(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* fargs[0] = partnumer to find name for
   * fargs[1] = 'short', 'long' or 'vlong'
   */

  int index;
  char *cptr;
  const char *infostr;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!fargs[0]) {
    safe_tprintf_str(buff, bufc, "#-1 NEED PARTNAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  long parsed_index = strtol(fargs[0], &cptr, 10);
  index = clamp_intptr_to_int(parsed_index);
  if (cptr == fargs[0]) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID PART NUMBER");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }

  char *const *name_type_slot = (char *const *)checked_storage_at_const(
      (const void *)fargs, (size_t)NFARGS, sizeof(*fargs), 1);
  const char NAME_TYPE = **name_type_slot;
  PartNameDescriptionFormat format;
  switch (NAME_TYPE) {
  case 's':
  case 'S':
    format = PART_NAME_DESCRIPTION_SHORT;
    break;
  case 'l':
  case 'L':
    format = PART_NAME_DESCRIPTION_LONG;
    break;
  case 'v':
  case 'V':
    format = PART_NAME_DESCRIPTION_VERY_LONG;
    break;
  default:
    safe_tprintf_str(buff, bufc, "#-1 INVALID NAME TYPE");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  infostr = partname_func(&(PartNameDescriptionRequest){
      .context = context->btech, .packed_part = index, .format = format});
  safe_tprintf_str(buff, bufc, "%s", infostr);

  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}

const char *partname_func(const PartNameDescriptionRequest *request) {
  BtechContext *context = request->context;
  const int INDEX = request->packed_part;
  PartNameRegistry *registry = context->part_names;
  int id, brand;
  PartNameEntry *p;

  id = packed_part_id(INDEX);
  brand = packed_part_brand(INDEX);
  if (brand < 0 || brand > BRANDCOUNT || id < 0 || id >= NUM_ITEMS)
    return "#-1 INVALID PART NUMBER";

  p = part_index_entry(registry, brand, id);
  if (!p)
    return "#-1 INVALID PART NUMBER";

  switch (request->format) {
  case PART_NAME_DESCRIPTION_SHORT:
    return p->shorty;
  case PART_NAME_DESCRIPTION_LONG:
    return p->longy;
  case PART_NAME_DESCRIPTION_VERY_LONG:
    return p->vlongy;
  }
  return "#-1 INVALID NAME TYPE";
}

size_t part_name_count(const BtechContext *context) {
  return (size_t)context->part_names->object_count;
}

const PartNameEntry *part_name_at(const BtechContext *context, size_t index) {
  if (index >= part_name_count(context))
    return nullptr;
  return sorted_entry(context->part_names->short_sorted,
                      part_name_count(context), index);
}

void destroy_partname_tables(BtechContext *context) {
  PartNameRegistry *registry = context->part_names;

  if (registry == nullptr)
    return;
  hash_table_destroy(&registry->short_hash);
  hash_table_destroy(&registry->vlong_hash);
  for (int brand = 0; brand <= BRANDCOUNT; brand++)
    for (int id = 0; id < NUM_ITEMS; id++) {
      PartNameEntry *part_name = part_index_entry(registry, brand, id);

      if (part_name == nullptr)
        continue;
      free(part_name->shorty);
      free(part_name->longy);
      free(part_name->vlongy);
      free(part_name);
    }
  free((void *)registry->short_sorted);
  free((void *)registry->long_sorted);
  free((void *)registry->vlong_sorted);
  free(registry);
  context->part_names = nullptr;
}
