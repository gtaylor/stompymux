/* Implements BattleTech repair mechanics for unit tech commands. */

#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "equipment_types.h"
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

static void tech_check_locpart(MuxEvent *e, void *data) {
  TechCheckContext *context = data;
  int loc;
  int pos;
  long l = (long)e->data2;

  RepairEventPayload payload = repair_event_payload_unpack(l);
  loc = payload.location;
  pos = payload.position;
  if (loc == context->location && pos == context->part)
    context->matches++;
}

static void tech_check_loc(MuxEvent *e, void *data) {
  TechCheckContext *context = data;
  long loc;

  loc = (((long)e->data2) % 16);
  if (loc == context->location)
    context->matches++;
}

typedef struct TechEventPartQuery {
  Mech *mech;
  int location;
  int part;
  int event_type;
} TechEventPartQuery;

typedef struct TechEventLocationQuery {
  Mech *mech;
  int location;
  int event_type;
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
int someone_repairing_s(Mech *mech, int loc, int part, int t) {
  return tech_event_part_count(&(TechEventPartQuery){
      .mech = mech, .location = loc, .part = part, .event_type = t});
}

bool someone_repairing(Mech *mech, int loc, int part) {
  const int EVENT_TYPES[] = {EVENT_REPAIR_RELO,      EVENT_REPAIR_REPL,
                             EVENT_REPAIR_REPLG,     EVENT_REPAIR_REPAP,
                             EVENT_REPAIR_REPAG,     EVENT_REPAIR_MOB,
                             EVENT_REPAIR_REPENHCRIT};
  for (size_t index = 0; index < (sizeof(EVENT_TYPES) / sizeof(EVENT_TYPES[0]));
       index++) {
    if (someone_repairing_s(
            mech, loc, part,
            tech_int_at(EVENT_TYPES, sizeof(EVENT_TYPES) / sizeof(*EVENT_TYPES),
                        index)))
      return 1;
  }
  return 0;
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
  return someone_fixing_a(mech, loc) || someone_fixing_i(mech, loc);
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
      .mech = mech, .location = loc, .event_type = EVENT_REPAIR_SCRL});
}

bool someone_scrapping_part(Mech *mech, int loc, int part) {
  const int EVENT_TYPES[] = {EVENT_REPAIR_SCRP, EVENT_REPAIR_SCRG,
                             EVENT_REPAIR_UMOB};
  for (size_t index = 0; index < (sizeof(EVENT_TYPES) / sizeof(EVENT_TYPES[0]));
       index++) {
    if (someone_repairing_s(
            mech, loc, part,
            tech_int_at(EVENT_TYPES, sizeof(EVENT_TYPES) / sizeof(*EVENT_TYPES),
                        index)))
      return 1;
  }
  return 0;
}

bool can_scrap_loc(Mech *mech, int loc) {
  TechCheckContext check = {.location = loc % 8};

  mech_event_visit(mech, EVENT_REPAIR_REPL, tech_check_loc, &check);
  mech_event_visit(mech, EVENT_REPAIR_RELO, tech_check_loc, &check);
  return !check.matches && !someone_fixing(mech, loc);
}

bool can_scrap_part(Mech *mech, int loc, int part) {
  return !(someone_repairing(mech, loc, part));
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

void tech_checkstatus(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
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
