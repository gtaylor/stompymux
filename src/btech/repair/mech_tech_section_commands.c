/* Implements BattleTech repair mechanics for unit tech section commands. */

#include <stdint.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_consistency_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "repair_job.h"
#include "section_types.h"

typedef struct TechCheckContext {
  int matches;
  int location;
  int part;
} TechCheckContext;

int invalid_repair_path(Mech *mech, int loc) {
  if (mech_class(mech) != CLASS_MECH)
    return 0;
  switch (loc) {
  case HEAD:
  case LTORSO:
  case RTORSO:
  case LLEG:
  case RLEG:
    return mech_section_is_destroyed(mech, CTORSO);
  case LARM:
    return mech_section_is_destroyed(mech, LTORSO);
  case RARM:
    return mech_section_is_destroyed(mech, RTORSO);
  }
  return 0;
}

int unit_is_fixable(Mech *mech) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (!mech_section_original_internal(mech, i))
      continue;
    if (!mech_section_is_destroyed(mech, i))
      continue;
    if (mech_class(mech) == CLASS_MECH)
      if (i == CTORSO)
        return 0;
    if (mech_class(mech) == CLASS_VTOL)
      if (i != ROTOR)
        return 0;
    if (mech_class(mech) == CLASS_VEH_GROUND)
      if (i != TURRET)
        return 0;
  }
  return 1;
}

static int adjusted_technology_time(BtechContext *context, int base_time,
                                    int roll) {
  if (!roll || !btech_context_uses_variable_technology_time(context))
    return base_time;
  return (base_time * 10) /
         (1000 /
          (100 - (btech_context_technology_time_modifier(context) * roll)));
}

static void schedule_section_repair(BtechContext *context, Mech *mech,
                                    DbRef player, int location, int event_type,
                                    MuxEventCallback callback) {
  btech_context_event_schedule(
      context, mech, event_type, callback,
      max(1, player_techtime(context, player) * TECH_TICK),
      (intptr_t)(location + (player * PLAYERPOS)));
}

static void take_section_materials(BtechContext *context, Mech *mech,
                                   int location) {
  int amount = mech_section_original_internal(mech, location);
  DbRef store = mech_parts_store_dbref(mech);
  economy_inventory_change(&(EconomyInventoryChange){
      .context = context,
      .store = store,
      .part = {.id = tech_proper_internal_part(mech), .brand = 0},
      .quantity_delta = -amount,
  });
  economy_inventory_change(&(EconomyInventoryChange){
      .context = context,
      .store = store,
      .part = {.id = cargo_equipment_index(S_ELECTRONIC), .brand = 0},
      .quantity_delta = -amount,
  });
}

void tech_reattach(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc;

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

  int internal_stock = 0;
  int electric_stock = 0;
  int roll;
  int rollmod;
  int fixtime;
  int base_fixtime;
  int fail_fixtime;

  RepairSelection selection;
  if (!repair_command_parse_part(&repair_command, buffer, false, false,
                                 &selection))
    return;
  loc = selection.location;
  if (mech_class(mech) == CLASS_BSUIT) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't reattach a Battlesuit! Use 'replacesuit'!");
    return;
  }
  if (!mech_section_is_destroyed(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That section isn't destroyed!");
    return;
  }
  if (invalid_repair_path(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to reattach adjacent locations first!");
    return;
  }
  if (someone_attaching(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's attaching that section already!");
    return;
  }
  if (!unit_is_fixable(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You see nothing to reattach it to (read:unit is cored).");
    return;
  }
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  internal_stock = econ_find_items(context, mech_parts_store_dbref(mech),
                                   tech_proper_internal_part(mech), 0);
  electric_stock = econ_find_items(context, mech_parts_store_dbref(mech),
                                   cargo_equipment_index(S_ELECTRONIC), 0);

  if (internal_stock < mech_section_original_internal(mech, loc)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        tprintf("Not enough %ss in stock. You need %d more.",
                part_name(context, tech_proper_internal_part(mech), 0).text,
                mech_section_original_internal(mech, loc) - internal_stock));
    return;
  }
  if (electric_stock < mech_section_original_internal(mech, loc)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        tprintf("Not enough Electrics in stock. You need %d more.",
                mech_section_original_internal(mech, loc) - electric_stock));
    return;
  }

  notify_printf(evaluation, player, "You start replacing the section...");
  rollmod = REATTACH_DIFFICULTY;
  roll = tech_roll(player, mech, rollmod);
  base_fixtime = REATTACH_TIME;
  fail_fixtime = (base_fixtime * 3) / 2;

  if (roll < 0) {
    notify_printf(
        evaluation, player,
        "Your attempt is unsuccessful, but you try to save the section...");
    rollmod = REATTACH_DIFFICULTY;
    roll = tech_roll(player, mech, rollmod);
    if (roll < 0) {
      fixtime = fail_fixtime;
      notify_printf(evaluation, player,
                    "You muck around, wasting the section for good...");
      /* TODO: maybe save X% of materials like before? */
      take_section_materials(context, mech, loc);
      tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      schedule_section_repair(context, mech, player, loc, EVENT_REPAIR_REAT,
                              mech_event_failure_marker);

    } else {
      notify_printf(evaluation, player, "You manage to replace the section...");
      /* it's a saving roll, so it is what it is */
      if (roll == 0)
        fixtime = fail_fixtime;
      else
        fixtime = adjusted_technology_time(context, fail_fixtime, roll);
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      take_section_materials(context, mech, loc);
      tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      schedule_section_repair(context, mech, player, loc, EVENT_REPAIR_REAT,
                              mux_event_tickmech_reattach);
    }
  } else {
    if (roll == 0)
      fixtime = base_fixtime;
    else
      fixtime = adjusted_technology_time(context, base_fixtime, roll);
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");
    take_section_materials(context, mech, loc);
    tech_addtechtime(&(TechTimeAddition){
        .context = context, .player = player, .units = fixtime});
    schedule_section_repair(context, mech, player, loc, EVENT_REPAIR_REAT,
                            mux_event_tickmech_reattach);
  }

  //	DOTECH_LOC(REATTACH_DIFFICULTY, reattach_fail, reattach_succ,
  //			   reattach_econ, REATTACH_TIME, mech, loc,
  //			   mux_event_tickmech_reattach, EVENT_REPAIR_REAT,
  //			   "You start replacing the section..");
}

void tech_replacesuit(DbRef player, void *data, char *buffer) {
  int w_suits = 0;

  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  int loc;

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
  RepairSelection selection;
  if (!repair_command_parse_part(&repair_command, buffer, false, false,
                                 &selection))
    return;
  loc = selection.location;
  if (mech_class(mech) != CLASS_BSUIT) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can only use 'replacesuit' on a battlesuit unit!");
    return;
  }

  w_suits = bsuit_member_count(mech);

  if (mech_maximum_battle_suits(mech) <= w_suits) {
    mecha_notify(
        btech_context_evaluation(context), player,
        tprintf("This %s is already full! This %s only consists of %d suits!",
                bsuit_formation_name_lowercase(mech),
                bsuit_formation_name_lowercase(mech),
                mech_maximum_battle_suits(mech)));
    return;
  }
  if ((loc >= mech_maximum_battle_suits(mech)) || (loc < 0)) {
    mecha_notify(btech_context_evaluation(context), player,
                 tprintf("Invalid suit! This %s only consists of %d suits!",
                         bsuit_formation_name_lowercase(mech),
                         mech_maximum_battle_suits(mech)));
    return;
  }

  if (!mech_section_is_destroyed(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That suit isn't destroyed!");
    return;
  }

  if (someone_replacing_suit(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's already rebuilding that suit!");
    return;
  }
  if (w_suits <= 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are unable to replace the suits here! None of the "
                 "buggers are still alive!");
    return;
  }
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  RepairSectionJob job = {
      .difficulty = REPLACESUIT_DIFFICULTY,
      .time = REPLACESUIT_TIME,
      .event_data =
          repair_event_payload_pack((RepairEventPayload){.location = loc}),
      .event_type = EVENT_REPAIR_REPSUIT,
      .event_callback = mux_event_tickmech_replacesuit,
      .message = "You start replacing the missing suit.",
      .resource = replacesuit_econ,
      .failure = replacesuit_fail,
      .success = replacesuit_succ,
  };
  (void)repair_section_job_execute(&repair_command, loc, &job);
}

/*
 * Reseal
 * Added by Kipsta
 * 8/4/99
 */

void tech_reseal(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  int loc;

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
  RepairSelection selection;
  if (!repair_command_parse_part(&repair_command, buffer, false, false,
                                 &selection))
    return;
  loc = selection.location;
  if (mech_section_is_destroyed(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That section is destroyed!");
    return;
  }
  if (!mech_section_is_flooded(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That has not been flooded!");
    return;
  }
  if (invalid_repair_path(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to reattach adjacent locations first!");
    return;
  }
  if (someone_resealing(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's sealing that section already!");
    return;
  }
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  RepairSectionJob job = {
      .difficulty = RESEAL_DIFFICULTY,
      .time = RESEAL_TIME,
      .event_data =
          repair_event_payload_pack((RepairEventPayload){.location = loc}),
      .event_type = EVENT_REPAIR_RESE,
      .event_callback = mux_event_tickmech_reseal,
      .message = "You start resealing the section.",
      .resource = reseal_econ,
      .failure = reseal_fail,
      .success = reseal_succ,
  };
  (void)repair_section_job_execute(&repair_command, loc, &job);
}

void tech_fixextra(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  EvaluationContext *evaluation;

  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, data, REPAIR_STALL_REQUIRED, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  mech = repair_command.mech;
  evaluation = repair_command.evaluation;
  mecha_notify(evaluation, player,
               "Fixed extra stuff - reseals, ammo feeds, etc.");
  do_fixextra(mech);
}

void tech_magic(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  EvaluationContext *evaluation;

  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, data, REPAIR_STALL_REQUIRED, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  mech = repair_command.mech;
  evaluation = repair_command.evaluation;
  mecha_notify(evaluation, player, "Doing the magic..");
  do_magic(mech);
  mech_int_check(mech, 1);
  mecha_notify(evaluation, player, "Done!");
}
