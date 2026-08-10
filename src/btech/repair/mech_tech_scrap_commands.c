/* Implements BattleTech repair mechanics for unit tech scrap commands. */

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "repair_job.h"
#include "section_types.h"

static int clan_modified_time(const Mech *mech, int time) {
  return MAX(1, time / ((mech_technology_flags(mech) & CLAN_TECH) ? 2 : 1));
}

typedef struct TechCheckContext {
  int matches;
  int location;
  int part;
} TechCheckContext;
void tech_removegun(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc, part, mod = 2;
  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, data, REPAIR_STALL_REQUIRED, &repair_command);
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
  if (!repair_command_parse_gun(&repair_command, buffer, false, &selection))
    return;
  loc = selection.location;
  part = selection.part;
  if (mech_section_is_destroyed(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part's blown off! You can assume the gun's gone too!");
    return;
  }
  if (!equipment_is_weapon(mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player, "That's no gun!");
    return;
  }
  if (mech_critical_is_destroyed(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That gun's gone already!");
    return;
  }
  if (!ValidGunPos(&(RepairCriticalSelection){
          .mech = mech, .location = loc, .position = part})) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't remove middle of a gun!");
    return;
  }
  if (SomeoneScrappingPart(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping it already!");
    return;
  }
  if (!CanScrapPart(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's tinkering with it already!");
    return;
  }
  if (SomeoneScrappingLoc(mech, loc)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "Someone's scrapping that section - no additional removals are "
        "possible!");
    return;
  }
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  /* Ok.. Everything's valid (we hope). */
  if (tech_weapon_roll(player, mech, REMOVEG_DIFFICULTY) < 0) {
    mecha_notify(evaluation, player,
                 "Ack! Your attempt is far from perfect, you try to recover "
                 "the gun..");
    if (tech_weapon_roll(player, mech, REMOVEG_DIFFICULTY) < 0) {
      mecha_notify(evaluation, player, "No good. Consider the part gone.");
      int time =
          REMOVEG_TIME *
          clan_modified_time(
              mech, GetWeaponCrits(
                        mech, weapon_from_equipment_index(
                                  mech_critical_part_type(mech, loc, part))));
      repair_event_schedule_with_techtime(
          &(RepairWorkSchedule){.command = &repair_command,
                                .work_time = time,
                                .multiplier = mod,
                                .event_type = EVENT_REPAIR_SCRG,
                                .callback = mech_event_failure_marker,
                                .payload = {.location = loc,
                                            .position = part,
                                            .extra = mod,
                                            .player = player}});
      return;
    }
  }
  mecha_notify(evaluation, player, "You start removing the gun..");
  int time =
      REMOVEG_TIME *
      clan_modified_time(
          mech,
          GetWeaponCrits(mech, weapon_from_equipment_index(
                                   mech_critical_part_type(mech, loc, part))));
  repair_event_schedule_with_techtime(&(RepairWorkSchedule){
      .command = &repair_command,
      .work_time = time,
      .multiplier = mod,
      .event_type = EVENT_REPAIR_SCRG,
      .callback = mux_event_tickmech_removegun,
      .payload = {
          .location = loc, .position = part, .extra = mod, .player = player}});
}

void tech_removepart(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc, part, t, mod = 2;
  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, data, REPAIR_STALL_REQUIRED, &repair_command);
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
  t = mech_critical_part_type(mech, loc, part);
  if (t == EMPTY) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That location is empty!");
    return;
  }
  if (mech_section_is_destroyed(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part's blown off! You can assume the part's gone too!");
    return;
  }
  if (equipment_is_weapon(t)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That's a gun - use removegun instead!");
    return;
  }
  if (mech_critical_is_destroyed(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part's gone already!");
    return;
  }
  if (mech_part_is_structural_placeholder(
          mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That type isn't scrappable!");
    return;
  }
  if (t == special_equipment_index(ENDO_STEEL) ||
      t == special_equipment_index(FERRO_FIBROUS) ||
      t == special_equipment_index(STEALTH_ARMOR) ||
      t == special_equipment_index(HVY_FERRO_FIBROUS) ||
      t == special_equipment_index(LT_FERRO_FIBROUS)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That type of item can't be removed!");
    return;
  }
  if (SomeoneScrappingPart(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping it already!");
    return;
  }
  if (SomeoneScrappingLoc(mech, loc)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "Someone's scrapping that section - no additional removals are "
        "possible!");
    return;
  }
  if (!CanScrapPart(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's tinkering with it already!");
    return;
  }
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  /* Ok.. Everything's valid (we hope). */
  mecha_notify(evaluation, player, "You start removing the part..");
  if (tech_roll(player, mech, REMOVEP_DIFFICULTY) < 0) {
    mecha_notify(evaluation, player,
                 "Ack! Your attempt is far from perfect, you try to recover "
                 "the part..");
    if (tech_roll(player, mech, REMOVEP_DIFFICULTY) < 0) {
      mecha_notify(evaluation, player, "No good. Consider the part gone.");
      mod = 3;
      repair_event_schedule_with_techtime(
          &(RepairWorkSchedule){.command = &repair_command,
                                .work_time = REMOVEP_TIME,
                                .multiplier = mod,
                                .event_type = EVENT_REPAIR_SCRP,
                                .callback = mech_event_failure_marker,
                                .payload = {.location = loc,
                                            .position = part,
                                            .extra = mod,
                                            .player = player}});
      return;
    }
  }
  repair_event_schedule_with_techtime(&(RepairWorkSchedule){
      .command = &repair_command,
      .work_time = REMOVEP_TIME,
      .multiplier = mod,
      .event_type = EVENT_REPAIR_SCRP,
      .callback = mux_event_tickmech_removepart,
      .payload = {
          .location = loc, .position = part, .extra = mod, .player = player}});
}

static bool invalid_scrap_dependency(Mech *mech, int location) {
  return !mech_section_is_destroyed(mech, location) ||
         Invalid_Scrap_Path(mech, location);
}

int Invalid_Scrap_Path(Mech *mech, int loc) {
  if (loc < 0)
    return 0;
  if (mech_class(mech) != CLASS_MECH)
    return 0;
  switch (loc) {
  case CTORSO:
    return invalid_scrap_dependency(mech, HEAD) ||
           invalid_scrap_dependency(mech, LTORSO) ||
           invalid_scrap_dependency(mech, RTORSO);
  case LTORSO:
    return invalid_scrap_dependency(mech, LARM);
  case RTORSO:
    return invalid_scrap_dependency(mech, RARM);
  }
  return 0;
}

void tech_removesection(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc, mod = 2;
  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, data, REPAIR_STALL_REQUIRED, &repair_command);
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
  if (!repair_command_parse_part(&repair_command, buffer, false, false,
                                 &selection))
    return;
  loc = selection.location;
  if (mech_section_is_destroyed(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That section's gone already!");
    return;
  }
  if (Invalid_Scrap_Path(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to remove the outer sections first!");
    return;
  }
  if (SomeoneScrappingLoc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping it already!");
    return;
  }
  if (!CanScrapLoc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's tinkering with it already!");
    return;
  }
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  /* Ok.. Everything's valid (we hope). */
  if (tech_roll(player, mech, REMOVES_DIFFICULTY) < 0)
    mod = 3;
  mecha_notify(evaluation, player, "You start removing the section..");
  repair_event_schedule_with_techtime(&(RepairWorkSchedule){
      .command = &repair_command,
      .work_time = REMOVES_TIME,
      .multiplier = mod,
      .event_type = EVENT_REPAIR_SCRL,
      .callback = mux_event_tickmech_removesection,
      .payload = {
          .location = loc, .position = 0, .extra = mod, .player = player}});
}
