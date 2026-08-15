#include "ai_api.h"
#include "autopilot.h"
#include "btech_event.h"
#include "context_internal.h" // IWYU pragma: keep
#include "mech_events.h"
#include "mech_tech_events_api.h"
#include "missile_hit_registry.h"
#include "mux/network/mux_event.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "sqlite_internal.h"

static void (*btech_special_repair_function(int type))(MuxEvent *) {
  switch (type) {
  case EVENT_REPAIR_MOB:
    return mux_event_tickmech_mountbomb;
  case EVENT_REPAIR_UMOB:
    return mux_event_tickmech_umountbomb;
  case EVENT_REPAIR_REPL:
  case EVENT_REPAIR_REPAP:
    return mux_event_tickmech_repairpart;
  case EVENT_REPAIR_REPLG:
    return mux_event_tickmech_replacegun;
  case EVENT_REPAIR_REPENHCRIT:
    return mux_event_tickmech_repairenhcrit;
  case EVENT_REPAIR_REPAG:
    return mux_event_tickmech_repairgun;
  case EVENT_REPAIR_REAT:
    return mux_event_tickmech_reattach;
  case EVENT_REPAIR_RELO:
    return mux_event_tickmech_reload;
  case EVENT_REPAIR_FIX:
    return mux_event_tickmech_repairarmor;
  case EVENT_REPAIR_FIXI:
    return mux_event_tickmech_repairinternal;
  case EVENT_REPAIR_SCRL:
    return mux_event_tickmech_removesection;
  case EVENT_REPAIR_SCRG:
    return mux_event_tickmech_removegun;
  case EVENT_REPAIR_SCRP:
    return mux_event_tickmech_removepart;
  case EVENT_REPAIR_RESE:
    return mux_event_tickmech_reseal;
  case EVENT_REPAIR_REPSUIT:
    return mux_event_tickmech_replacesuit;
  default:
    return nullptr;
  }
}

/* Requeue repair work with its original remaining ticks and fake-event state.
 */
int btech_special_load_repair_events(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  DbRef mech_dbref;
  long event_data;
  void (*function)(MuxEvent *);
  int event_type;
  int fake;
  int remaining_ticks;
  int result;
  int step;

  statement = nullptr;
  result =
      btech_special_prepare_v2(
          sqlite,
          "SELECT mech_dbref, event_type, remaining_ticks, event_data, is_fake "
          "FROM btech_repair_events ORDER BY event_id;",
          -1, &statement, nullptr) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0) {
      result = -1;
      break;
    }
    mech = btech_context_get_mech(context, mech_dbref);
    if (!mech || btech_special_column_int(statement, 1, &event_type) < 0 ||
        btech_special_column_int(statement, 2, &remaining_ticks) < 0 ||
        btech_special_column_long(statement, 3, &event_data) < 0 ||
        btech_special_column_int(statement, 4, &fake) < 0 ||
        event_type < FIRST_TECH_EVENT || event_type > LAST_TECH_EVENT ||
        remaining_ticks < 1 || fake < 0 || fake > 1) {
      result = -1;
      break;
    }
    function = fake ? mech_event_failure_marker
                    : btech_special_repair_function(event_type);
    if (!function) {
      result = -1;
      break;
    }
    mux_event_add(&(MuxEventRequest){.scheduler = context->events,
                                     .delay = remaining_ticks,
                                     .type = event_type,
                                     .callback = function,
                                     .data = mech,
                                     .secondary_data = (void *)event_data});
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore MECH identity and unit-definition fields before child tables. */
