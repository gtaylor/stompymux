/* Implements BattleTech repair mechanics for unit tech commands. */

#include <stdint.h>
#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "repair_gun_layout.h"
#include "repair_job.h"

typedef struct TechCheckContext {
  int matches;
  int location;
  int part;
} TechCheckContext;

static int tech_int_at(const int *values, size_t count, size_t index) {
  const int *value =
      checked_storage_at_const(values, count, sizeof(*values), index);
  return *value;
}

static MechEventType tech_event_type_at(const MechEventType *values,
                                        size_t count, size_t index) {
  const MechEventType *value =
      checked_storage_at_const(values, count, sizeof(*values), index);
  return *value;
}

static void tech_check_locpart(MuxEvent *e, void *data) {
  TechCheckContext *context = data;
  intptr_t event_data = e->secondary.integer;

  RepairEventPayload payload = repair_event_payload_unpack(event_data);
  if (payload.location == context->location &&
      payload.position == context->part)
    context->matches++;
}

static void tech_check_loc(MuxEvent *e, void *data) {
  TechCheckContext *context = data;
  RepairEventPayload payload =
      repair_event_payload_unpack(e->secondary.integer);

  if (payload.location == context->location)
    context->matches++;
}

static void tech_check_scrap_gun_footprint(MuxEvent *event, void *data) {
  TechCheckContext *context = data;
  Mech *mech = event->data;
  RepairEventPayload payload =
      repair_event_payload_unpack(event->secondary.integer);
  if (payload.location == context->location) {
    context->matches++;
    return;
  }
  RepairGunLayout layout;
  if (mech &&
      repair_gun_layout_find(mech, payload.location, payload.position,
                             REPAIR_GUN_LAYOUT_REQUIRE_WEAPON, &layout) &&
      layout.local_count < layout.size &&
      layout.split.slot.section == context->location)
    context->matches++;
}

typedef struct TechEventPartQuery {
  Mech *mech;
  int location;
  int part;
  MechEventType event_type;
} TechEventPartQuery;

typedef struct TechEventLocationQuery {
  Mech *mech;
  int location;
  MechEventType event_type;
} TechEventLocationQuery;

static int tech_event_part_count(const TechEventPartQuery *query) {
  TechCheckContext check = {.location = query->location, .part = query->part};
  mech_event_visit(query->mech, query->event_type, tech_check_locpart, &check);
  return check.matches;
}

static int tech_event_location_count(const TechEventLocationQuery *query) {
  TechCheckContext check = {.location = query->location};
  mech_event_visit(query->mech, query->event_type, tech_check_loc, &check);
  return check.matches;
}

/* Replace/reload */
int someone_repairing_s(Mech *mech, int loc, int part, MechEventType type) {
  return tech_event_part_count(&(TechEventPartQuery){
      .mech = mech, .location = loc, .part = part, .event_type = type});
}

bool someone_repairing(Mech *mech, int loc, int part) {
  const MechEventType EVENT_TYPES[] = {
      EVENT_REPAIR_RELO,      EVENT_REPAIR_REPL,  EVENT_REPAIR_REPLG,
      EVENT_REPAIR_REPAP,     EVENT_REPAIR_REPAG, EVENT_REPAIR_MOB,
      EVENT_REPAIR_REPENHCRIT};
  for (size_t index = 0; index < (sizeof(EVENT_TYPES) / sizeof(EVENT_TYPES[0]));
       index++) {
    if (someone_repairing_s(
            mech, loc, part,
            tech_event_type_at(EVENT_TYPES,
                               sizeof(EVENT_TYPES) / sizeof(*EVENT_TYPES),
                               index)))
      return true;
  }
  return false;
}

/* Fixinternal/armor */
int someone_fixing_a(Mech *mech, int loc) {
  return tech_event_location_count(&(TechEventLocationQuery){
      .mech = mech, .location = loc, .event_type = EVENT_REPAIR_FIX});
}

int someone_fixing_i(Mech *mech, int loc) {
  return tech_event_location_count(&(TechEventLocationQuery){
      .mech = mech, .location = loc, .event_type = EVENT_REPAIR_FIXI});
}

bool someone_fixing(Mech *mech, int loc) {
  return (someone_fixing_a(mech, loc) || someone_fixing_i(mech, loc)) != 0;
}

/* Reattach */
int someone_attaching(Mech *mech, int loc) {
  return tech_event_location_count(&(TechEventLocationQuery){
      .mech = mech, .location = loc, .event_type = EVENT_REPAIR_REAT});
}

int someone_replacing_suit(Mech *mech, int loc) {
  return tech_event_location_count(&(TechEventLocationQuery){
      .mech = mech, .location = loc, .event_type = EVENT_REPAIR_REPSUIT});
}

/* Reseal
 *
 * Added by Kipsta
 * 8/4/99
 */

int someone_resealing(Mech *mech, int loc) {
  return tech_event_location_count(&(TechEventLocationQuery){
      .mech = mech, .location = loc, .event_type = EVENT_REPAIR_RESE});
}

int someone_scrapping_loc(Mech *mech, int loc) {
  return tech_event_location_count(&(TechEventLocationQuery){
      .mech = mech,
      .location = loc % NUM_SECTIONS,
      .event_type = EVENT_REPAIR_SCRL,
  });
}

bool someone_scrapping_part(Mech *mech, int loc, int part) {
  const MechEventType EVENT_TYPES[] = {EVENT_REPAIR_SCRP, EVENT_REPAIR_SCRG,
                                       EVENT_REPAIR_UMOB};
  for (size_t index = 0; index < (sizeof(EVENT_TYPES) / sizeof(EVENT_TYPES[0]));
       index++) {
    if (someone_repairing_s(
            mech, loc, part,
            tech_event_type_at(EVENT_TYPES,
                               sizeof(EVENT_TYPES) / sizeof(*EVENT_TYPES),
                               index)))
      return true;
  }
  return false;
}

bool can_scrap_loc(Mech *mech, int loc) {
  const MechEventType EVENT_TYPES[] = {
      EVENT_REPAIR_REPL,       EVENT_REPAIR_REPAP,   EVENT_REPAIR_MOB,
      EVENT_REPAIR_REPENHCRIT, EVENT_REPAIR_RELO,    EVENT_REPAIR_FIXI,
      EVENT_REPAIR_REAT,       EVENT_REPAIR_REPSUIT, EVENT_REPAIR_RESE,
      EVENT_REPAIR_SCRP,       EVENT_REPAIR_UMOB,
  };
  TechCheckContext check = {.location = loc % NUM_SECTIONS};

  for (size_t index = 0; index < (sizeof(EVENT_TYPES) / sizeof(EVENT_TYPES[0]));
       index++) {
    mech_event_visit(
        mech,
        tech_event_type_at(EVENT_TYPES,
                           sizeof(EVENT_TYPES) / sizeof(*EVENT_TYPES), index),
        tech_check_loc, &check);
  }
  mech_event_visit(mech, EVENT_REPAIR_SCRG, tech_check_scrap_gun_footprint,
                   &check);
  mech_event_visit(mech, EVENT_REPAIR_REPLG, tech_check_scrap_gun_footprint,
                   &check);
  mech_event_visit(mech, EVENT_REPAIR_REPAG, tech_check_scrap_gun_footprint,
                   &check);
  return (check.matches == 0 && !someone_fixing_a(mech, check.location) &&
          !someone_fixing_a(mech, check.location + NUM_SECTIONS)) != 0;
}

bool can_scrap_part(Mech *mech, int loc, int part) {
  return (!(someone_repairing(mech, loc, part))) != 0;
}

bool valid_gun_pos(const RepairCriticalSelection *selection) {
  Mech *mech = selection->mech;
  const int LOC = selection->location;
  const int POS = selection->position;
  unsigned char weaparray_f[MAX_WEAPS_SECTION];
  unsigned char weapdata_f[MAX_WEAPS_SECTION];
  int critical_f[MAX_WEAPS_SECTION];
  int i;
  int num_weaps_f;

  num_weaps_f =
      find_weapons_advanced(mech, LOC, weaparray_f, weapdata_f, critical_f, 1);
  if (num_weaps_f < 0)
    return false;
  for (i = 0; i < num_weaps_f; i++)
    if (tech_int_at(critical_f, MAX_WEAPS_SECTION, (size_t)i) == POS)
      return true;
  return false;
}

void tech_checkstatus(DbRef player, Mech *mech, char *buffer [[maybe_unused]]) {
  BtechContext *context = mech_context(mech);
  EvaluationContext *evaluation = btech_context_evaluation(context);
  int i = figure_latest_tech_event(mech);
  UptimeText uptime;

  if (!i) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The mech's ready to rock!");
    return;
  }
  uptime = uptime_text(game_lag_time(context, i));
  notify_printf(evaluation, player,
                "The 'mech has approximately %s until done.", uptime.text);
}
