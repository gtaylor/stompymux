/* Implements BattleTech repair mechanics for unit tech commands. */

#include <string.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "mech_consistency_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
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
  int loc, pos;
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

static int tech_event_part_count(Mech *mech, int location, int part,
                                 int event_type) {
  TechCheckContext check = {.location = location, .part = part};
  mech_event_visit(mech, event_type, tech_check_locpart, &check);
  return check.matches;
}

static int tech_event_location_count(Mech *mech, int location, int event_type) {
  TechCheckContext check = {.location = location};
  mech_event_visit(mech, event_type, tech_check_loc, &check);
  return check.matches;
}

/* Replace/reload */
int SomeoneRepairing_s(Mech *mech, int loc, int part, int t) {
  return tech_event_part_count(mech, loc, part, t);
}

int SomeoneRepairing(Mech *mech, int loc, int part) {
  const int event_types[] = {EVENT_REPAIR_RELO,      EVENT_REPAIR_REPL,
                             EVENT_REPAIR_REPLG,     EVENT_REPAIR_REPAP,
                             EVENT_REPAIR_REPAG,     EVENT_REPAIR_MOB,
                             EVENT_REPAIR_REPENHCRIT};
  for (size_t index = 0; index < (sizeof(event_types) / sizeof(event_types[0]));
       index++)
    if (SomeoneRepairing_s(
            mech, loc, part,
            tech_int_at(event_types, sizeof(event_types) / sizeof(*event_types),
                        index)))
      return 1;
  return 0;
}

/* Fixinternal/armor */
int SomeoneFixingA(Mech *mech, int loc) {
  return tech_event_location_count(mech, loc, EVENT_REPAIR_FIX);
}

int SomeoneFixingI(Mech *mech, int loc) {
  return tech_event_location_count(mech, loc, EVENT_REPAIR_FIXI);
}

int SomeoneFixing(Mech *mech, int loc) {
  return SomeoneFixingA(mech, loc) || SomeoneFixingI(mech, loc);
}

/* Reattach */
int SomeoneAttaching(Mech *mech, int loc) {
  return tech_event_location_count(mech, loc, EVENT_REPAIR_REAT);
}

int SomeoneReplacingSuit(Mech *mech, int loc) {
  return tech_event_location_count(mech, loc, EVENT_REPAIR_REPSUIT);
}

/* Reseal
 *
 * Added by Kipsta
 * 8/4/99
 */

int SomeoneResealing(Mech *mech, int loc) {
  return tech_event_location_count(mech, loc, EVENT_REPAIR_RESE);
}

int SomeoneScrappingLoc(Mech *mech, int loc) {
  return tech_event_location_count(mech, loc, EVENT_REPAIR_SCRL);
}

int SomeoneScrappingPart(Mech *mech, int loc, int part) {
  const int event_types[] = {EVENT_REPAIR_SCRP, EVENT_REPAIR_SCRG,
                             EVENT_REPAIR_UMOB};
  for (size_t index = 0; index < (sizeof(event_types) / sizeof(event_types[0]));
       index++)
    if (SomeoneRepairing_s(
            mech, loc, part,
            tech_int_at(event_types, sizeof(event_types) / sizeof(*event_types),
                        index)))
      return 1;
  return 0;
}

int CanScrapLoc(Mech *mech, int loc) {
  TechCheckContext check = {.location = loc % 8};

  mech_event_visit(mech, EVENT_REPAIR_REPL, tech_check_loc, &check);
  mech_event_visit(mech, EVENT_REPAIR_RELO, tech_check_loc, &check);
  return !check.matches && !SomeoneFixing(mech, loc);
}

int CanScrapPart(Mech *mech, int loc, int part) {
  return !(SomeoneRepairing(mech, loc, part));
}

int ValidGunPos(Mech *mech, int loc, int pos) {
  unsigned char weaparray_f[MAX_WEAPS_SECTION];
  unsigned char weapdata_f[MAX_WEAPS_SECTION];
  int critical_f[MAX_WEAPS_SECTION];
  int i, num_weaps_f;

  if ((num_weaps_f = FindWeapons_Advanced(mech, loc, weaparray_f, weapdata_f,
                                          critical_f, 1)) < 0)
    return 0;
  for (i = 0; i < num_weaps_f; i++)
    if (tech_int_at(critical_f, MAX_WEAPS_SECTION, (size_t)i) == pos)
      return 1;
  return 0;
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
