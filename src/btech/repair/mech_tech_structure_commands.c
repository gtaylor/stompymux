/* Implements BattleTech repair mechanics for unit tech structure commands. */

#include "btech/context.h"
#include "equipment_types.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_parts.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "repair_job.h"

void tech_fixarmor(DbRef player, Mech *facility, char *buffer) {
  int ochange;

  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  int loc;
  int from;
  int to;
  int change;

  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, facility, REPAIR_STALL_CONFIGURED, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  mech = repair_command.mech;
  context = repair_command.context;
  const TechPartParseResult PARSED = tech_part_parse(&(TechPartParseRequest){
      .mech = mech, .text = buffer, .allow_rear = true});
  if (PARSED.status != TECH_PART_PARSE_OK) {
    mecha_notify(btech_context_evaluation(context), player, "Invalid section!");
    return;
  }
  loc = PARSED.location;
  if (loc >= NUM_SECTIONS) {
    from = mech_section_rear_armor(mech, loc % NUM_SECTIONS);
    to = mech_section_original_rear_armor(mech, loc % NUM_SECTIONS);
  } else {
    from = mech_section_armor(mech, loc);
    to = mech_section_original_armor(mech, loc);
  }
  if (mech_section_is_destroyed(mech, loc % NUM_SECTIONS)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part's blown off! Use reattach first!");
    return;
  }
  if (mech_section_is_flooded(mech, loc % NUM_SECTIONS)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That location has been flooded! Use reseal first!");
    return;
  }
  if (someone_fixing_a(mech, loc) ||
      someone_fixing_i(mech, loc % NUM_SECTIONS)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's repairing that section already!");
    return;
  }
  if (mech_section_internal(mech, loc % NUM_SECTIONS) !=
      mech_section_original_internal(mech, loc % NUM_SECTIONS)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The internals need to be fixed first!");
    return;
  }
  if (someone_scrapping_loc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping that section - no repairs are possible!");
    return;
  }
  from = to < from ? to : from;
  if (from >= to) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The location doesn't need armor repair!");
    return;
  }
  change = to - from;
  ochange = change;
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }
  RepairSectionAmountJob job = {
      .difficulty = FIXARMOR_DIFFICULTY,
      .failure_time = FIXARMOR_TIME * ochange,
      .unit_time = FIXARMOR_TIME,
      .failure_event_type = EVENT_REPAIR_FIX,
      .event_type = EVENT_REPAIR_FIX,
      .event_callback = mux_event_tickmech_repairarmor,
      .message = "You start fixing the armor..",
      .resource = fixarmor_econ,
      .failure = fixarmor_fail,
      .success = fixarmor_succ,
  };
  (void)repair_section_amount_job_execute(&repair_command, loc, &change, &job);
}

void tech_fixinternal(DbRef player, Mech *facility, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  int loc;
  int from;
  int to;
  int change;
  int ochange;

  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, facility, REPAIR_STALL_REQUIRED, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  mech = repair_command.mech;
  context = repair_command.context;
  RepairSelection selection;
  if (!repair_command_parse_part(&repair_command, buffer, false, false,
                                 &selection))
    return;
  loc = selection.location;
  from = mech_section_internal(mech, loc);
  to = mech_section_original_internal(mech, loc);
  if (from >= to) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The location doesn't need internals' repair!");
    return;
  }
  change = to - from;
  if (mech_section_is_destroyed(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part's blown off! Use reattach first!");
    return;
  }
  if (mech_section_is_flooded(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That location has been flooded! Use reseal first!");
    return;
  }
  if (someone_fixing_i(mech, loc) || someone_fixing_a(mech, loc) ||
      someone_fixing_a(mech, loc + NUM_SECTIONS)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's repairing that section already!");
    return;
  }
  if (someone_scrapping_loc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping that section - no repairs are possible!");
    return;
  }
  ochange = change;
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  RepairSectionAmountJob job = {
      .difficulty = FIXINTERNAL_DIFFICULTY,
      .failure_time = FIXINTERNAL_TIME * ochange,
      .unit_time = FIXINTERNAL_TIME,
      .failure_event_type = EVENT_REPAIR_FIXI,
      .event_type = EVENT_REPAIR_FIXI,
      .event_callback = mux_event_tickmech_repairinternal,
      .message = "You start fixing the internals..",
      .resource = fixinternal_econ,
      .failure = fixinternal_fail,
      .success = fixinternal_succ,
  };
  (void)repair_section_amount_job_execute(&repair_command, loc, &change, &job);
}
