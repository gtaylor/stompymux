#include "btech/context.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "repair_job.h"
#include "section_types.h"

void tech_toggletype(DbRef player, void *data, char *buffer) {
  int atype;

  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  int loc, part, t;
  RepairCommandContext repair_command = {
      .player = player,
      .mech = mech,
      .context = context,
      .evaluation = btech_context_evaluation(context),
      .is_dropship = mech_is_dropship(mech),
  };

  if ((!is_wizard(btech_context_database(context), player)) &&
      is_in_character(btech_context_database(context), mech_dbref(mech))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This command only works in simpods!");
    return;
  }
  RepairSelection selection;
  if (!repair_command_parse_part(&repair_command, buffer, true, true,
                                 &selection))
    return;
  loc = selection.location;
  part = selection.part;
  atype = selection.brand;
  if (!equipment_is_ammunition(mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player, "That's no ammo!");
    return;
  }
  if (mech_critical_is_nonfunctional(mech, loc, part) ||
      mech_critical_is_disabled(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The ammo compartment is nonfunctional!");
    return;
  }
  if (!atype) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to give a type to toggle to (use - for normal)");
    return;
  }
  t = valid_ammo_mode(mech, loc, part, atype);
  if (t < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That is invalid ammo type for this weapon!");
    return;
  }
  mech_critical_ammo_mode_set(
      mech, loc, part,
      (mech_critical_ammo_mode(mech, loc, part) & ~AMMO_MODES) | t);
  mech_critical_data_set(mech, loc, part, FullAmmo(mech, loc, part));
  mech_notify(mech, MECHALL, "Ammo toggled.");
}

void tech_reload(DbRef player, void *data, char *buffer) {
  int atype;

  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  int loc, part, t, change;
  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, data, REPAIR_STALL_CONFIGURED, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  mech = repair_command.mech;
  context = repair_command.context;
  RepairSelection selection;
  if (!repair_command_parse_part(&repair_command, buffer, true, true,
                                 &selection))
    return;
  loc = selection.location;
  part = selection.part;
  atype = selection.brand;
  if (!equipment_is_ammunition(mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player, "That's no ammo!");
    return;
  }
  if (mech_critical_is_nonfunctional(mech, loc, part)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "The ammo compartment is destroyed ; repair/replacepart it first.");
    return;
  }
  if (mech_critical_is_disabled(mech, loc, part)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "The ammo compartment is disabled ; repair/replacepart it first.");
    return;
  }
  if (mech_critical_data(mech, loc, part) == FullAmmo(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That particular ammo compartment doesn't need reloading.");
    return;
  }
  if (SomeoneRepairing(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's playing with that part already!");
    return;
  }
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
  if (SomeoneScrappingLoc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping that section - no repairs are possible!");
    return;
  }
  if (atype) {
    t = valid_ammo_mode(mech, loc, part, atype);
    if (t < 0) {
      mecha_notify(btech_context_evaluation(context), player,
                   "That is invalid ammo type for this weapon!");
      return;
    }
    mech_critical_data_set(mech, loc, part, 0);
    mech_critical_ammo_mode_set(
        mech, loc, part,
        (mech_critical_ammo_mode(mech, loc, part) & ~AMMO_MODES) | t);
  }
  change = 0;

  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  RepairPartAmountJob job = {
      .difficulty = RELOAD_DIFFICULTY,
      .time = RELOAD_TIME,
      .event_type = EVENT_REPAIR_RELO,
      .event_callback = mux_event_tickmech_reload,
      .message = "You start reloading the ammo compartment..",
      .resource = reload_econ,
      .failure = reload_fail,
      .success = reload_succ,
  };
  (void)repair_part_amount_job_execute(&repair_command, loc, part, &change,
                                       &job);
}

void tech_unload(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc, part, now, change, mod = 2;

  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, data, REPAIR_STALL_CONFIGURED, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  mech = repair_command.mech;
  context = repair_command.context;
  evaluation = repair_command.evaluation;
  RepairSelection selection;
  if (!repair_command_parse_part(&repair_command, buffer, true, false,
                                 &selection))
    return;
  loc = selection.location;
  part = selection.part;
  if (!equipment_is_ammunition(mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player, "That's no ammo!");
    return;
  }
  if (mech_critical_is_nonfunctional(mech, loc, part)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "The ammo compartment is destroyed ; repair/replacepart it first.");
    return;
  }
  if (mech_critical_is_disabled(mech, loc, part)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "The ammo compartment is disabled ; repair/replacepart it first.");
    return;
  }
  if (!mech_critical_data(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That particular ammo compartment is empty already.");
    return;
  }
  if (SomeoneRepairing(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's playing with that part already!");
    return;
  }
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
  if (SomeoneScrappingLoc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping that section - no repairs are possible!");
    return;
  }
  now = mech_critical_data(mech, loc, part);
  if (FullAmmo(mech, loc, part) == now)
    change = 2;
  else
    change = 1;
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  if (tech_roll(player, mech, REMOVES_DIFFICULTY) < 0)
    mod = 3;
  mecha_notify(evaluation, player,
               "You start unloading the ammo compartment..");
  repair_event_schedule_with_techtime(
      &(RepairWorkSchedule){.command = &repair_command,
                            .work_time = RELOAD_TIME,
                            .multiplier = mod,
                            .event_type = EVENT_REPAIR_RELO,
                            .callback = mux_event_tickmech_reload,
                            .payload = {.location = loc,
                                        .position = part,
                                        .extra = change,
                                        .player = player}});
}
