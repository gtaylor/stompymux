#include "ai_api.h"
#include "autopilot.h"
#include "btech_event.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_specification_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/network/mux_event.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "repair_event_validation.h"
#include "repair_gun_layout.h"
#include "repair_job.h"
#include "section_types.h"
#include "sqlite_internal.h"

#include <stdint.h>
#include <stdlib.h>

MuxEventCallback btech_special_repair_function_for_type(int type) {
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

static bool repair_event_section_valid(int section) {
  return (section >= 0 && section < NUM_SECTIONS) != 0;
}

static bool repair_event_critical_valid(Mech *mech, int section, int position) {
  return (repair_event_section_valid(section) && position >= 0 &&
          position < mech_section_critical_count(mech, section)) != 0;
}

static bool repair_event_targets_critical(int event_type);

// NOLINTNEXTLINE(bugprone-easily-swappable-parameters)
static bool repair_event_gun_scrap_actionable(Mech *mech, int section,
                                              int position,
                                              const RepairGunLayout *layout) {
  if (mech_section_is_destroyed(mech, section) ||
      mech_section_is_flooded(mech, section))
    return false;
  for (int critical = position; critical < position + layout->local_count;
       critical++)
    if (mech_critical_is_destroyed(mech, section, critical))
      return false;
  if (layout->local_count >= layout->size)
    return true;
  for (int critical = layout->split.slot.critical;
       critical <
       layout->split.slot.critical + layout->size - layout->local_count;
       critical++)
    if (mech_critical_is_destroyed(mech, layout->split.slot.section, critical))
      return false;
  return true;
}

BtechRepairEventClassification btech_special_repair_event_classify(
    Mech *mech,
    int event_type /* NOLINT(bugprone-easily-swappable-parameters) */,
    intptr_t event_data, bool fake) {
  RepairEventPayload payload = repair_event_payload_unpack(event_data);
  if (!repair_event_payload_structurally_valid(event_type, payload, fake))
    return BTECH_REPAIR_EVENT_INVALID;

  if (fake && event_type != EVENT_REPAIR_SCRP &&
      event_type != EVENT_REPAIR_SCRG)
    return BTECH_REPAIR_EVENT_ACTIONABLE;
  if (repair_event_targets_critical(event_type) &&
      !repair_event_critical_valid(mech, payload.location, payload.position))
    return BTECH_REPAIR_EVENT_STALE;

  switch (event_type) {
  case EVENT_REPAIR_FIX: {
    int section = payload.location % NUM_SECTIONS;
    int deficit = payload.location >= NUM_SECTIONS
                      ? mech_section_original_rear_armor(mech, section) -
                            mech_section_rear_armor(mech, section)
                      : mech_section_original_armor(mech, section) -
                            mech_section_armor(mech, section);
    return deficit > 0 ? BTECH_REPAIR_EVENT_ACTIONABLE
                       : BTECH_REPAIR_EVENT_STALE;
  }
  case EVENT_REPAIR_FIXI: {
    int deficit = mech_section_original_internal(mech, payload.location) -
                  mech_section_internal(mech, payload.location);
    return deficit > 0 ? BTECH_REPAIR_EVENT_ACTIONABLE
                       : BTECH_REPAIR_EVENT_STALE;
  }
  case EVENT_REPAIR_REAT:
    if (mech_section_is_destroyed(mech, payload.location))
      return BTECH_REPAIR_EVENT_ACTIONABLE;
    return BTECH_REPAIR_EVENT_STALE;
  case EVENT_REPAIR_RESE:
    return !mech_section_is_destroyed(mech, payload.location) &&
                   mech_section_is_flooded(mech, payload.location)
               ? BTECH_REPAIR_EVENT_ACTIONABLE
               : BTECH_REPAIR_EVENT_STALE;
  case EVENT_REPAIR_REPSUIT:
    return mech_class(mech) == CLASS_BSUIT &&
                   payload.location < mech_maximum_battle_suits(mech) &&
                   mech_section_is_destroyed(mech, payload.location)
               ? BTECH_REPAIR_EVENT_ACTIONABLE
               : BTECH_REPAIR_EVENT_STALE;
  case EVENT_REPAIR_SCRL:
    return !mech_section_is_destroyed(mech, payload.location)
               ? BTECH_REPAIR_EVENT_ACTIONABLE
               : BTECH_REPAIR_EVENT_STALE;
  case EVENT_REPAIR_REPL:
  case EVENT_REPAIR_REPAP: {
    int part_type =
        mech_critical_part_type(mech, payload.location, payload.position);
    return part_type != EMPTY && !equipment_is_weapon(part_type) &&
                   !mech_part_is_structural_placeholder(part_type) &&
                   !mech_section_is_destroyed(mech, payload.location) &&
                   !mech_section_is_flooded(mech, payload.location) &&
                   mech_critical_is_nonfunctional(mech, payload.location,
                                                  payload.position)
               ? BTECH_REPAIR_EVENT_ACTIONABLE
               : BTECH_REPAIR_EVENT_STALE;
  }
  case EVENT_REPAIR_REPLG:
  case EVENT_REPAIR_REPAG: {
    RepairGunLayout layout;
    return repair_gun_layout_find(mech, payload.location, payload.position,
                                  REPAIR_GUN_LAYOUT_REQUIRE_WEAPON |
                                      REPAIR_GUN_LAYOUT_REQUIRE_GUN_START |
                                      REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS |
                                      REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS,
                                  &layout) &&
                   mech_critical_is_nonfunctional(mech, payload.location,
                                                  payload.position)
               ? BTECH_REPAIR_EVENT_ACTIONABLE
               : BTECH_REPAIR_EVENT_STALE;
  }
  case EVENT_REPAIR_REPENHCRIT: {
    int part_type =
        mech_critical_part_type(mech, payload.location, payload.position);
    if (!equipment_is_weapon(part_type))
      return BTECH_REPAIR_EVENT_STALE;
    int first = mech_weapon_first_critical(&(WeaponCriticalSearch){
        .mech = mech,
        .weapon = {.section = payload.location, .critical = payload.position},
        .start_critical = 0,
        .part_type = part_type,
        .maximum_criticals =
            get_weapon_crits(mech, weapon_from_equipment_index(part_type)),
    });
    return repair_event_critical_valid(mech, payload.location, first) &&
                   !mech_section_is_destroyed(mech, payload.location) &&
                   !mech_section_is_flooded(mech, payload.location) &&
                   mech_critical_is_damaged(mech, payload.location,
                                            payload.position)
               ? BTECH_REPAIR_EVENT_ACTIONABLE
               : BTECH_REPAIR_EVENT_STALE;
  }
  case EVENT_REPAIR_RELO:
    if (!equipment_is_ammunition(mech_critical_part_type(mech, payload.location,
                                                         payload.position)) ||
        mech_section_is_destroyed(mech, payload.location) ||
        mech_section_is_flooded(mech, payload.location) ||
        mech_critical_is_nonfunctional(mech, payload.location,
                                       payload.position))
      return BTECH_REPAIR_EVENT_STALE;
    int current_ammo =
        mech_critical_data(mech, payload.location, payload.position);
    bool actionable;
    if (payload.extra != 0)
      actionable = (bool)(current_ammo > 0);
    else
      actionable = (bool)(current_ammo <
                          full_ammo(mech, payload.location, payload.position));
    return actionable ? BTECH_REPAIR_EVENT_ACTIONABLE
                      : BTECH_REPAIR_EVENT_STALE;
  case EVENT_REPAIR_SCRP: {
    int part_type =
        mech_critical_part_type(mech, payload.location, payload.position);
    return part_type != EMPTY && !equipment_is_weapon(part_type) &&
                   !mech_part_is_structural_placeholder(part_type) &&
                   !mech_critical_is_destroyed(mech, payload.location,
                                               payload.position)
               ? BTECH_REPAIR_EVENT_ACTIONABLE
               : BTECH_REPAIR_EVENT_STALE;
  }
  case EVENT_REPAIR_SCRG: {
    RepairGunLayout layout;
    return repair_gun_layout_find(mech, payload.location, payload.position,
                                  REPAIR_GUN_LAYOUT_REQUIRE_WEAPON |
                                      REPAIR_GUN_LAYOUT_REQUIRE_GUN_START |
                                      REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS |
                                      REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS,
                                  &layout) &&
                   repair_event_gun_scrap_actionable(mech, payload.location,
                                                     payload.position, &layout)
               ? BTECH_REPAIR_EVENT_ACTIONABLE
               : BTECH_REPAIR_EVENT_STALE;
  }
  case EVENT_REPAIR_MOB:
  case EVENT_REPAIR_UMOB:
    return BTECH_REPAIR_EVENT_ACTIONABLE;
  default:
    return BTECH_REPAIR_EVENT_INVALID;
  }
}

typedef struct RestoredRepairEvent {
  Mech *mech;
  MuxEventCallback function;
  long event_data;
  int event_type;
  int remaining_ticks;
  bool fake;
} RestoredRepairEvent;

static RestoredRepairEvent *
restored_repair_event_at(RestoredRepairEvent events[], size_t count,
                         size_t index) {
  return checked_storage_at(events, count, sizeof(*events), index);
}

static const RestoredRepairEvent *
restored_repair_event_at_const(const RestoredRepairEvent events[], size_t count,
                               size_t index) {
  return checked_storage_at_const(events, count, sizeof(*events), index);
}

static bool repair_event_targets_critical(int event_type) {
  switch (event_type) {
  case EVENT_REPAIR_REPL:
  case EVENT_REPAIR_REPAP:
  case EVENT_REPAIR_REPLG:
  case EVENT_REPAIR_REPAG:
  case EVENT_REPAIR_REPENHCRIT:
  case EVENT_REPAIR_RELO:
  case EVENT_REPAIR_SCRP:
  case EVENT_REPAIR_SCRG:
    return true;
  default:
    return false;
  }
}

bool btech_special_repair_events_conflict(
    int first_type /* NOLINT(bugprone-easily-swappable-parameters) */,
    intptr_t first_data,
    int second_type /* NOLINT(bugprone-easily-swappable-parameters) */,
    intptr_t second_data) {
  RepairEventPayload first = repair_event_payload_unpack(first_data);
  RepairEventPayload second = repair_event_payload_unpack(second_data);
  if (repair_event_targets_critical(first_type) &&
      repair_event_targets_critical(second_type))
    return (first.location == second.location &&
            first.position == second.position) != 0;
  if (first_type != second_type)
    return false;
  switch (first_type) {
  case EVENT_REPAIR_FIX:
  case EVENT_REPAIR_FIXI:
  case EVENT_REPAIR_REAT:
  case EVENT_REPAIR_RESE:
  case EVENT_REPAIR_REPSUIT:
  case EVENT_REPAIR_SCRL:
    return first.location == second.location;
  default:
    return false;
  }
}

static bool repair_event_duplicate(const RestoredRepairEvent events[],
                                   size_t count,
                                   const RestoredRepairEvent *candidate) {
  for (size_t index = 0; index < count; index++) {
    const RestoredRepairEvent *event =
        restored_repair_event_at_const(events, count, index);
    if (event->mech != candidate->mech)
      continue;
    if (btech_special_repair_events_conflict(
            event->event_type, event->event_data, candidate->event_type,
            candidate->event_data))
      return true;
  }
  return false;
}

static bool repair_scheduler_has_events(BtechContext *context) {
  for (int type = FIRST_TECH_EVENT; type <= LAST_TECH_EVENT; type++)
    if (mux_event_count_type(context->events, type) > 0)
      return true;
  return false;
}

/* Requeue repair work with its original remaining ticks and fake-event state.
 */
int btech_special_load_repair_events(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  RestoredRepairEvent *events = nullptr;
  size_t event_count = 0;
  Mech *mech;
  DbRef mech_dbref;
  long event_data;
  void (*function)(MuxEvent *);
  int event_type;
  int fake;
  int remaining_ticks;
  int result;
  int step = SQLITE_DONE;

  if (!context || !context->events || repair_scheduler_has_events(context))
    return -1;

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
                    : btech_special_repair_function_for_type(event_type);
    BtechRepairEventClassification classification =
        btech_special_repair_event_classify(mech, event_type, event_data,
                                            fake != 0);
    if (!function || classification == BTECH_REPAIR_EVENT_INVALID) {
      result = -1;
      break;
    }
    if (classification == BTECH_REPAIR_EVENT_STALE)
      continue;
    RestoredRepairEvent candidate = {
        .mech = mech,
        .function = function,
        .event_data = event_data,
        .event_type = event_type,
        .remaining_ticks = remaining_ticks,
        .fake = fake != 0,
    };
    if (repair_event_duplicate(events, event_count, &candidate)) {
      result = -1;
      break;
    }
    RestoredRepairEvent *grown = checked_storage_try_reallocate_array(
        events, event_count + 1, sizeof(*events));
    if (!grown) {
      result = -1;
      break;
    }
    events = grown;
    *restored_repair_event_at(events, event_count + 1, event_count) = candidate;
    event_count++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  if (result == 0) {
    for (size_t index = 0; index < event_count; index++) {
      RestoredRepairEvent *event =
          restored_repair_event_at(events, event_count, index);
      mux_event_add(&(MuxEventRequest){
          .scheduler = context->events,
          .delay = event->remaining_ticks,
          .type = event->event_type,
          .callback = event->function,
          .data = event->mech,
          .secondary_data = (void *)(intptr_t)event->event_data,
      });
    }
  }
  free(events);
  return result;
}

/* Restore MECH identity and unit-definition fields before child tables. */
