#include "repair_job.h"

#include <math.h>
#include <stdint.h>

#include "btech/context.h"
#include "btech_event.h"
#include "checked_conversion.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_tech_api.h"
#include "mechrep.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

static bool repair_player_is_wizard(const RepairCommandContext *command) {
  return is_wizard(btech_context_database(command->context), command->player);
}

RepairCommandStatus
repair_command_context_initialize(DbRef player, void *data,
                                  RepairStallPolicy stall_policy,
                                  RepairCommandContext *command) {
  *command = (RepairCommandContext){.player = player, .mech = data};
  if (!command->mech)
    return REPAIR_COMMAND_MISSING_MECH;

  command->context = mech_context(command->mech);
  command->evaluation = btech_context_evaluation(command->context);
  command->is_dropship = mech_is_dropship(command->mech);
  if (mech_event_count(command->mech, EVENT_STARTUP) &&
      !repair_player_is_wizard(command))
    return REPAIR_COMMAND_STARTING;
  if (mech_is_started(command->mech) && !repair_player_is_wizard(command))
    return REPAIR_COMMAND_STARTED;

  bool stall_required =
      stall_policy == REPAIR_STALL_REQUIRED ||
      btech_context_limits_repairs_to_stalls(command->context);
  if (stall_required && !command->is_dropship &&
      mech_repair_stall_dbref(command->mech) <= 0 &&
      !repair_player_is_wizard(command))
    return REPAIR_COMMAND_NO_STALL;
  return REPAIR_COMMAND_READY;
}

const char *repair_command_status_message(RepairCommandStatus status) {
  switch (status) {
  case REPAIR_COMMAND_MISSING_MECH:
    return "Error has occured in techcommand ; please contact a wiz";
  case REPAIR_COMMAND_STARTING:
    return "The mech's starting up! Please stop the sequence first.";
  case REPAIR_COMMAND_STARTED:
    return "The mech's started up ; please shut it down first.";
  case REPAIR_COMMAND_NO_STALL:
    return "The 'mech isn't in a repair stall!";
  case REPAIR_COMMAND_UNAUTHORIZED:
    return "I'm sorry Dave, can't do that.";
  case REPAIR_COMMAND_NO_TARGET:
    return "You must set a target first!";
  case REPAIR_COMMAND_TARGET_UNALLOCATED:
    return "The target's BTech data is not allocated.";
  case REPAIR_COMMAND_READY:
    return "";
  }
  return "";
}

RepairCommandStatus repair_facility_command_context_initialize(
    DbRef player, void *data, bool target_required,
    RepairFacilityCommandContext *command) {
  RepairFacility *facility = data;
  *command = (RepairFacilityCommandContext){
      .player = player,
      .facility = facility,
      .context = facility ? facility->xcode.context : nullptr,
      .evaluation = facility ? btech_context_evaluation(facility->xcode.context)
                             : nullptr,
  };
  if (!facility)
    return REPAIR_COMMAND_MISSING_MECH;
  GameDatabase *database = btech_context_database(command->context);
  if (!is_god(database, player) && !is_wizard(database, player))
    return REPAIR_COMMAND_UNAUTHORIZED;
  if (!target_required)
    return REPAIR_COMMAND_READY;
  if (facility->current_target == -1)
    return REPAIR_COMMAND_NO_TARGET;
  command->mech =
      btech_context_get_mech(command->context, facility->current_target);
  return command->mech ? REPAIR_COMMAND_READY
                       : REPAIR_COMMAND_TARGET_UNALLOCATED;
}

RepairParseStatus repair_selection_parse_part(Mech *mech, char *buffer,
                                              bool parse_position,
                                              bool parse_brand,
                                              RepairSelection *selection) {
  *selection = (RepairSelection){0};
  int result = tech_parsepart(mech, buffer, &selection->location,
                              parse_position ? &selection->part : nullptr,
                              parse_brand ? &selection->brand : nullptr);
  if (result == -1)
    return REPAIR_PARSE_INVALID_SECTION;
  if (result == -2)
    return REPAIR_PARSE_INVALID_PART;
  return REPAIR_PARSE_OK;
}

bool repair_command_parse_part(RepairCommandContext *command, char *buffer,
                               bool parse_position, bool parse_brand,
                               RepairSelection *selection) {
  RepairParseStatus status = repair_selection_parse_part(
      command->mech, buffer, parse_position, parse_brand, selection);
  if (status == REPAIR_PARSE_OK)
    return true;
  mecha_notify(command->evaluation, command->player,
               repair_parse_status_message(status));
  return false;
}

bool repair_command_parse_gun(RepairCommandContext *command, char *buffer,
                              bool parse_brand, RepairSelection *selection) {
  RepairParseStatus status =
      repair_selection_parse_gun(command->mech, buffer, parse_brand, selection);
  if (status == REPAIR_PARSE_OK)
    return true;
  mecha_notify(command->evaluation, command->player,
               repair_parse_status_message(status));
  return false;
}

RepairParseStatus repair_selection_parse_gun(Mech *mech, char *buffer,
                                             bool parse_brand,
                                             RepairSelection *selection) {
  *selection = (RepairSelection){0};
  int result =
      tech_parsegun(mech, buffer, &selection->location, &selection->part,
                    parse_brand ? &selection->brand : nullptr);
  switch (result) {
  case -1:
    return REPAIR_PARSE_INVALID_GUN;
  case -2:
    return REPAIR_PARSE_INVALID_REPLACEMENT;
  case -3:
    return REPAIR_PARSE_MISMATCHED_REPLACEMENT;
  case -4:
    return REPAIR_PARSE_INVALID_GUN_LOCATION;
  default:
    return REPAIR_PARSE_OK;
  }
}

const char *repair_parse_status_message(RepairParseStatus status) {
  switch (status) {
  case REPAIR_PARSE_INVALID_SECTION:
    return "Invalid section!";
  case REPAIR_PARSE_INVALID_PART:
    return "Invalid part!";
  case REPAIR_PARSE_INVALID_GUN:
    return "Invalid gun #!";
  case REPAIR_PARSE_INVALID_REPLACEMENT:
    return "Invalid object to replace with!";
  case REPAIR_PARSE_MISMATCHED_REPLACEMENT:
    return "Invalid object type - not matching with original.";
  case REPAIR_PARSE_INVALID_GUN_LOCATION:
    return "Invalid gun location - subscript out of range.";
  case REPAIR_PARSE_OK:
    return "";
  }
  return "";
}

void repair_event_schedule(Mech *mech, int delay, int event_type,
                           MuxEventCallback callback,
                           RepairEventPayload payload) {
#ifndef BT_FREETECHTIME
  int scheduled_delay = delay > 1 ? delay : 1;
#else
  int scheduled_delay =
      btech_context_uses_free_technology_time(mech_context(mech))
          ? 2
          : (delay > 2 ? delay : 2);
#endif
  btech_context_event_schedule(mech_context(mech), mech, event_type, callback,
                               scheduled_delay,
                               repair_event_payload_pack(payload));
}

void repair_event_schedule_minutes(Mech *mech, int minutes, int event_type,
                                   MuxEventCallback callback,
                                   RepairEventPayload payload) {
  repair_event_schedule(mech, minutes * TECH_TICK, event_type, callback,
                        payload);
}

void repair_event_schedule_with_techtime(RepairCommandContext *command,
                                         int work_time, int multiplier,
                                         int event_type,
                                         MuxEventCallback callback,
                                         RepairEventPayload payload) {
  int delay = tech_addtechtime(command->context, command->player,
                               (work_time * multiplier) / 2);
  repair_event_schedule(command->mech, delay, event_type, callback, payload);
}

void repair_event_schedule_amount(RepairCommandContext *command, int work_time,
                                  int multiplier, int amount, int event_type,
                                  MuxEventCallback callback,
                                  RepairEventPayload payload) {
  int delay = tech_addtechtime(command->context, command->player,
                               (work_time * multiplier) / 2);
  if (amount > 0)
    delay -= TECH_TICK * (work_time * (amount - 1) / amount);
  repair_event_schedule(command->mech, delay, event_type, callback, payload);
}

static RepairJobResult repair_job_schedule(RepairCommandContext *command,
                                           int time, int multiplier,
                                           int event_type,
                                           MuxEventCallback callback,
                                           intptr_t event_data, bool failure) {
  RepairEventPayload payload = repair_event_payload_unpack(event_data);
  payload.player = command->player;
  repair_event_schedule_with_techtime(
      command, time, multiplier, event_type,
      failure ? mech_event_failure_marker : callback, payload);
  return failure ? REPAIR_JOB_SCHEDULED_FAILURE : REPAIR_JOB_SCHEDULED_SUCCESS;
}

RepairJobResult repair_part_job_execute(RepairCommandContext *command,
                                        int location, int part,
                                        const RepairPartJob *job) {
  if (job->resource(command->player, command->mech, location, part) < 0)
    return REPAIR_JOB_REJECTED;
  mecha_notify(command->evaluation, command->player, job->message);
  bool failed =
      job->weapon_roll
          ? tech_weapon_roll(command->player, command->mech, job->difficulty) <
                0
          : tech_roll(command->player, command->mech, job->difficulty) < 0;
  if (failed) {
    if (job->failure(command->player, command->mech, location, part) < 0)
      return repair_job_schedule(command, job->time, 3, job->event_type,
                                 job->event_callback, job->event_data, true);
  } else if (job->success(command->player, command->mech, location, part) < 0) {
    return REPAIR_JOB_CALLBACK_ABORTED;
  }
  return repair_job_schedule(command, job->time, failed ? 3 : 2,
                             job->event_type, job->event_callback,
                             job->event_data, false);
}

RepairJobResult repair_part_amount_job_execute(RepairCommandContext *command,
                                               int location, int part,
                                               int *amount,
                                               const RepairPartAmountJob *job) {
  if (job->resource(command->player, command->mech, location, part, amount) < 0)
    return REPAIR_JOB_REJECTED;
  mecha_notify(command->evaluation, command->player, job->message);
  bool failed = tech_roll(command->player, command->mech, job->difficulty) < 0;
  if (failed) {
    if (job->failure(command->player, command->mech, location, part, amount) <
        0) {
      repair_event_schedule_with_techtime(
          command, job->time, 3, job->event_type, mech_event_failure_marker,
          (RepairEventPayload){.location = location,
                               .position = part,
                               .extra = *amount,
                               .player = command->player});
      return REPAIR_JOB_SCHEDULED_FAILURE;
    }
  } else if (job->success(command->player, command->mech, location, part,
                          amount) < 0) {
    return REPAIR_JOB_CALLBACK_ABORTED;
  }
  repair_event_schedule_with_techtime(
      command, job->time, failed ? 3 : 2, job->event_type, job->event_callback,
      (RepairEventPayload){.location = location,
                           .position = part,
                           .extra = *amount,
                           .player = command->player});
  return REPAIR_JOB_SCHEDULED_SUCCESS;
}

RepairJobResult repair_section_job_execute(RepairCommandContext *command,
                                           int location,
                                           const RepairSectionJob *job) {
  if (job->resource(command->player, command->mech, location) < 0)
    return REPAIR_JOB_REJECTED;
  mecha_notify(command->evaluation, command->player, job->message);
  bool failed = tech_roll(command->player, command->mech, job->difficulty) < 0;
  if (failed) {
    if (job->failure(command->player, command->mech, location) < 0)
      return repair_job_schedule(command, job->time, 3, job->event_type,
                                 job->event_callback, job->event_data, true);
  } else if (job->success(command->player, command->mech, location) < 0) {
    return REPAIR_JOB_CALLBACK_ABORTED;
  }
  return repair_job_schedule(command, job->time, failed ? 3 : 2,
                             job->event_type, job->event_callback,
                             job->event_data, false);
}

RepairJobResult
repair_section_amount_job_execute(RepairCommandContext *command, int location,
                                  int *amount,
                                  const RepairSectionAmountJob *job) {
  if (job->resource(command->player, command->mech, location, amount) < 0)
    return REPAIR_JOB_REJECTED;
  mecha_notify(command->evaluation, command->player, job->message);
  bool failed = tech_roll(command->player, command->mech, job->difficulty) < 0;
  if (failed) {
    if (job->failure(command->player, command->mech, location, amount) < 0) {
      RepairEventPayload payload = {.location = location,
                                    .player = command->player};
      repair_event_schedule_with_techtime(command, job->failure_time, 3,
                                          job->failure_event_type,
                                          mech_event_failure_marker, payload);
      return REPAIR_JOB_SCHEDULED_FAILURE;
    }
  } else if (job->success(command->player, command->mech, location, amount) <
             0) {
    return REPAIR_JOB_CALLBACK_ABORTED;
  }
  RepairEventPayload payload = {
      .location = location, .position = *amount, .player = command->player};
  repair_event_schedule_amount(command, job->unit_time * *amount,
                               failed ? 3 : 2, *amount, job->event_type,
                               job->event_callback, payload);
  return REPAIR_JOB_SCHEDULED_SUCCESS;
}

int repair_part_type_difficulty(int part_type) {
  (void)part_type;
  return 1;
}

int repair_weapon_type_difficulty(int part_type) {
  int weapon = weapon_from_equipment_index(part_type);
  int critical_slots = weapon_catalogue_critical_slots(weapon);
  float const difficulty = sqrtf((float)critical_slots * 1.5F - 1.1F);
  return clamp_float_to_int(difficulty);
}
