#include "btech/configuration.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "context_internal.h"
#include "map.h"
#include "map_bits_api.h"
#include "map_obj_api.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

#include <limits.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct MapLinkFixture {
  BtechContext context;
  GameDatabase database;
  BattleMap *maps;
  GameObject *objects;
  DbRef *parents;
  size_t map_count;
  size_t processed_count;
  int expected_builds;
  int expected_leaves;
  int expected_skipped;
  size_t expected_processed;
  DbRef expected_leave_map;
  DbRef expected_leave_target;
  char notification[256];
} MapLinkFixture;

static MapLinkFixture *active_fixture;

static BattleMap *fixture_map(DbRef dbref) {
  if (active_fixture == nullptr || dbref < 0 ||
      (size_t)dbref >= active_fixture->map_count) {
    return nullptr;
  }
  return checked_storage_at(active_fixture->maps, active_fixture->map_count,
                            sizeof(*active_fixture->maps), (size_t)dbref);
}

static MapObject **fixture_object_slot(BattleMap *map, int type) {
  if (type < 0)
    abort();
  return checked_storage_at(map->map_object, NUM_MAPOBJTYPES,
                            sizeof(*map->map_object), (size_t)type);
}

BattleMap *btech_context_get_map(BtechContext *context, DbRef dbref) {
  return active_fixture != nullptr && context == &active_fixture->context
             ? fixture_map(dbref)
             : nullptr;
}

void *btech_context_find_object(BtechContext *context, DbRef dbref) {
  return btech_context_get_map(context, dbref);
}

EvaluationContext *btech_context_evaluation(BtechContext *context
                                            [[maybe_unused]]) {
  return nullptr;
}

bool btech_map_link(BtechContext *context, BtechObjectId child,
                    BtechMapLink *link) {
  if (active_fixture == nullptr || context != &active_fixture->context ||
      child < 0 || (size_t)child >= active_fixture->map_count)
    return false;
  DbRef parent = *(const DbRef *)checked_storage_at_const(
      active_fixture->parents, active_fixture->map_count,
      sizeof(*active_fixture->parents), (size_t)child);
  if (parent == NOTHING)
    return false;
  if (link != nullptr)
    *link = (BtechMapLink){.parent = parent};
  return true;
}

void btech_map_links_visit(BtechContext *context, BtechMapLinkVisitor visitor,
                           void *visitor_context) {
  if (active_fixture == nullptr || context != &active_fixture->context)
    return;
  for (size_t child = 0; child < active_fixture->map_count; child++) {
    BtechMapLink link;
    if (btech_map_link(context, (BtechObjectId)child, &link) &&
        !visitor((BtechObjectId)child, &link, visitor_context))
      return;
  }
}

void clear_hex_bits(BattleMap *map [[maybe_unused]],
                    int bits [[maybe_unused]]) {
  active_fixture->processed_count++;
}

void set_hex_enterable(BattleMap *map [[maybe_unused]], int x [[maybe_unused]],
                       int y [[maybe_unused]]) {}

MapObject *first_mapobj(BattleMap *map, int type) {
  return *fixture_object_slot(map, type);
}

void del_mapobjst(BattleMap *map, int type) {
  MapObject **head = fixture_object_slot(map, type);
  while (*head != nullptr) {
    MapObject *next = (*head)->next;
    free(*head);
    *head = next;
  }
}

MapObject *add_mapobj_to_type(BattleMap *map, int type, MapObject *source,
                              int flag [[maybe_unused]]) {
  MapObject **head = fixture_object_slot(map, type);
  MapObject *stored = checked_storage_allocate(sizeof(*stored));
  *stored = *source;
  stored->next = *head;
  *head = stored;
  return stored;
}

void notify_printf(EvaluationContext *evaluation [[maybe_unused]],
                   DbRef player [[maybe_unused]], const char *format, ...) {
  va_list arguments;
  va_start(arguments, format);
  (void)vsnprintf(active_fixture->notification,
                  sizeof(active_fixture->notification), format, arguments);
  va_end(arguments);
}

static bool fixture_load(MapLinkFixture *fixture, const char *path) {
  FILE *input = fopen(path, "r");
  if (input == nullptr)
    return false;

  size_t edge_count;
  if (fscanf(input, "%zu %zu %d %d %d %zu %ld %ld", &fixture->map_count,
             &edge_count, &fixture->expected_builds, &fixture->expected_leaves,
             &fixture->expected_skipped, &fixture->expected_processed,
             &fixture->expected_leave_map,
             &fixture->expected_leave_target) != 8 ||
      fixture->map_count == 0 || fixture->map_count > INT_MAX - 1) {
    (void)fclose(input);
    return false;
  }

  fixture->maps = checked_storage_allocate_array(fixture->map_count,
                                                 sizeof(*fixture->maps));
  fixture->parents = checked_storage_allocate_array(fixture->map_count,
                                                    sizeof(*fixture->parents));
  fixture->database.size = (int)fixture->map_count + 1;
  fixture->objects = checked_storage_allocate_array(
      (size_t)fixture->database.size + 1, sizeof(*fixture->objects));
  fixture->database.object_storage = fixture->objects;
  fixture->database.top = fixture->database.size;
  fixture->context.database = &fixture->database;

  for (size_t index = 0; index < fixture->map_count; ++index) {
    *(DbRef *)checked_storage_at(fixture->parents, fixture->map_count,
                                 sizeof(*fixture->parents), index) = NOTHING;
    BattleMap *map = checked_storage_at(fixture->maps, fixture->map_count,
                                        sizeof(*fixture->maps), index);
    map->xcode.context = &fixture->context;
    map->map_width = 1;
    map->map_height = 1;
  }

  for (size_t edge = 0; edge < edge_count; ++edge) {
    long source;
    long target;
    if (fscanf(input, "%ld %ld", &source, &target) != 2 || source < 0 ||
        target < 0 || (size_t)source >= fixture->map_count ||
        (size_t)target >= fixture->map_count) {
      (void)fclose(input);
      return false;
    }
    DbRef *parent =
        checked_storage_at(fixture->parents, fixture->map_count,
                           sizeof(*fixture->parents), (size_t)target);
    if (*parent == NOTHING)
      *parent = source;
  }

  const DbRef PLAYER = (DbRef)fixture->map_count;
  game_object_set_location(&fixture->database, PLAYER, 0);
  return fclose(input) == 0;
}

static int fixture_build_count(const MapLinkFixture *fixture) {
  int count = 0;
  for (size_t index = 0; index < fixture->map_count; ++index) {
    const BattleMap *map = checked_storage_at_const(
        fixture->maps, fixture->map_count, sizeof(*fixture->maps), index);
    for (const MapObject *object = map->map_object[TYPE_BUILD];
         object != nullptr; object = object->next) {
      count++;
    }
  }
  return count;
}

static DbRef fixture_leave_target(const MapLinkFixture *fixture, DbRef map_id) {
  const BattleMap *map =
      checked_storage_at_const(fixture->maps, fixture->map_count,
                               sizeof(*fixture->maps), (size_t)map_id);
  const MapObject *leave = map->map_object[TYPE_LEAVE];
  return leave == nullptr ? NOTHING : leave->obj;
}

static void fixture_destroy(MapLinkFixture *fixture) {
  if (fixture->maps != nullptr) {
    for (size_t index = 0; index < fixture->map_count; ++index) {
      BattleMap *map = checked_storage_at(fixture->maps, fixture->map_count,
                                          sizeof(*fixture->maps), index);
      for (int type = 0; type < NUM_MAPOBJTYPES; ++type)
        del_mapobjst(map, type);
    }
  }
  free(fixture->maps);
  free(fixture->objects);
  free(fixture->parents);
  *fixture = (MapLinkFixture){};
}

static bool fixture_check(const char *path) {
  MapLinkFixture fixture = {};
  active_fixture = &fixture;
  if (!fixture_load(&fixture, path)) {
    (void)fprintf(stderr, "%s: could not load fixture\n", path);
    fixture_destroy(&fixture);
    active_fixture = nullptr;
    return false;
  }

  const DbRef PLAYER = (DbRef)fixture.map_count;
  char command_buffer[] = "";
  map_updatelinks(PLAYER, fixture_map(0), command_buffer);

  char expected[256];
  (void)snprintf(expected, sizeof(expected),
                 "Updated %d BUILD objs, %d LEAVE objs, 0 ENTRANCE objs; "
                 "skipped %d link descents.",
                 fixture.expected_builds, fixture.expected_leaves,
                 fixture.expected_skipped);
  const bool PASSED =
      fixture.processed_count == fixture.expected_processed &&
      fixture_build_count(&fixture) == fixture.expected_builds &&
      strcmp(fixture.notification, expected) == 0 &&
      (fixture.expected_leave_map < 0 ||
       fixture_leave_target(&fixture, fixture.expected_leave_map) ==
           fixture.expected_leave_target);
  if (!PASSED) {
    (void)fprintf(stderr, "%s: processed=%zu/%zu builds=%d/%d output=\"%s\"\n",
                  path, fixture.processed_count, fixture.map_count,
                  fixture_build_count(&fixture), fixture.expected_builds,
                  fixture.notification);
  }
  fixture_destroy(&fixture);
  active_fixture = nullptr;
  return PASSED;
}

int main(int argc, char **argv) {
  if (argc < 2) {
    (void)fprintf(stderr, "usage: btech_map_links_test FIXTURE...\n");
    return 2;
  }
  for (int index = 1; index < argc; ++index) {
    const char *path = *(char *const *)checked_storage_at_const(
        argv, (size_t)argc, sizeof(*argv), (size_t)index);
    if (!fixture_check(path))
      return 1;
  }
  return 0;
}
