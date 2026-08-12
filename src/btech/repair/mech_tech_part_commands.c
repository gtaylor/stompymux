/* Implements BattleTech repair mechanics for unit tech part commands. */

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "repair_job.h"

static int clan_modified_time(const Mech *mech, int time) {
  return max(1, time / ((mech_technology_flags(mech) & CLAN_TECH) ? 2 : 1));
}

typedef struct TechCheckContext {
  int matches;
  int location;
  int part;
} TechCheckContext;
void tech_replacegun(DbRef player, void *data, char *buffer) {
  int brand = 0;
  int ob = 0;

  int roll;
  int rollmod;
  int fixtime;
  int base_fixtime;
  int parttype;
  int fail_fixtime;

  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc;
  int part;
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
  if (!repair_command_parse_gun(&repair_command, buffer, true, &selection))
    return;
  loc = selection.location;
  part = selection.part;
  brand = selection.brand;
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
  if (someone_repairing(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's repairing that part already!");
    return;
  }
  if (!equipment_is_weapon(mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player, "That's no gun!");
    return;
  }
  if (!valid_gun_pos(&(RepairCriticalSelection){
          .mech = mech, .location = loc, .position = part})) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't replace middle of a gun!");
    return;
  }
  if (!mech_critical_is_nonfunctional(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That gun isn't hurtin'!");
    return;
  }
  if (someone_scrapping_loc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping that section - no repairs are possible!");
    return;
  }
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  if (brand) {
    ob = mech_critical_brand(mech, loc, part);
    mech_critical_brand_set(
        &(CriticalSlotBrandSet){.mech = mech,
                                .slot = {.section = loc, .critical = part},
                                .brand = brand});
  }

  parttype = mech_critical_part_type(mech, loc, part);

  if (equipment_is_ammunition(mech_critical_part_type(mech, loc, part))
          ? 0
          : econ_find_items(
                context,
                mech_is_dropship(mech)
                    ? mech_bay_dbref(mech, 0)
                    : game_object_location(btech_context_database(context),
                                           mech_dbref(mech)),
                parttype, mech_critical_brand(mech, loc, part)) < 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 tprintf("Not enough units of %s in store.",
                         part_name(context, parttype,
                                   mech_critical_brand(mech, loc, part))
                             .text));
    return;
  }

  notify_printf(evaluation, player, "You start replacing the gun...");
  rollmod = REPLACE_DIFFICULTY + repair_weapon_type_difficulty(
                                     mech_critical_part_type(mech, loc, part));
  roll = tech_weapon_roll(player, mech, rollmod);
  base_fixtime =
      REPLACEGUN_TIME *
      clan_modified_time(
          mech, get_weapon_crits(
                    mech, weapon_from_equipment_index(
                              mech_critical_part_type(mech, loc, part))));
  fail_fixtime = (base_fixtime * 3) / 2;

  if (roll < 0) {
    notify_printf(
        evaluation, player,
        "Your attempt is unsuccessful, but you try to save the gun...");
    rollmod = REPLACE_DIFFICULTY;
    roll = tech_roll(player, mech, rollmod);
    if (roll < 0) {
      fixtime = fail_fixtime;
      notify_printf(evaluation, player,
                    "You muck around, wasting the gun for good...");
      /* part goes , 1.5 * techtime*/
      if (!(equipment_is_ammunition(
              mech_critical_part_type(mech, loc, part)))) {
        economy_inventory_change(&(EconomyInventoryChange){
            .context = context,
            .store = mech_is_dropship(mech)
                         ? mech_bay_dbref(mech, 0)
                         : game_object_location(btech_context_database(context),
                                                mech_dbref(mech)),
            .part = {.id = parttype,
                     .brand = mech_critical_brand(mech, loc, part)},
            .quantity_delta = -1,
        });
      }
      tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPLG, mech_event_failure_marker,
          max(1, player_techtime(context, player) * TECH_TICK),
          repair_event_payload_pack((RepairEventPayload){
              .location = loc, .position = part, .extra = brand}) +
              (player * PLAYERPOS));

    } else {
      notify_printf(evaluation, player, "You manage to save the gun...");
      /* part doesn't go. 1.5 * techtime, but lets mod the fix time if
       * applicable*/
      /* We should really MIN(100,mod * roll) for the subtract to cap this out
       */
      if (roll == 0)
        fixtime = fail_fixtime;
      else {
        fixtime =
            btech_context_uses_variable_technology_time(context)
                ? (fail_fixtime * 10) /
                      (1000 /
                       (100 - (roll ? btech_context_technology_time_modifier(
                                          context) *
                                          roll
                                    : 0)))
                : fail_fixtime;
      }
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPLG, mech_event_failure_marker,
          max(1, player_techtime(context, player) * TECH_TICK),
          repair_event_payload_pack((RepairEventPayload){
              .location = loc, .position = part, .extra = brand}) +
              (player * PLAYERPOS));
    }

  } else {
    if (roll == 0)
      fixtime = base_fixtime;
    else {
      fixtime =
          btech_context_uses_variable_technology_time(context)
              ? (base_fixtime * 10) /
                    (1000 /
                     (100 -
                      (roll ? btech_context_technology_time_modifier(context) *
                                  roll
                            : 0)))
              : base_fixtime;
    }
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");
    if (!(equipment_is_ammunition(mech_critical_part_type(mech, loc, part)))) {
      economy_inventory_change(&(EconomyInventoryChange){
          .context = context,
          .store = mech_is_dropship(mech)
                       ? mech_bay_dbref(mech, 0)
                       : game_object_location(btech_context_database(context),
                                              mech_dbref(mech)),
          .part = {.id = parttype,
                   .brand = mech_critical_brand(mech, loc, part)},
          .quantity_delta = -1,
      });
    }
    tech_addtechtime(&(TechTimeAddition){
        .context = context, .player = player, .units = fixtime});
    btech_context_event_schedule(
        context, mech, EVENT_REPAIR_REPLG, mux_event_tickmech_replacegun,
        max(1, player_techtime(context, player) * TECH_TICK),
        repair_event_payload_pack((RepairEventPayload){
            .location = loc, .position = part, .extra = brand}) +
            (player * PLAYERPOS));
  }

  if (brand)
    mech_critical_brand_set(&(CriticalSlotBrandSet){
        .mech = mech, .slot = {.section = loc, .critical = part}, .brand = ob});
}

void tech_repairgun(DbRef player, void *data, char *buffer) {
  int extra_hard = 0;

  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc;
  int part;
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
  /* Find the gun for us */
  RepairSelection selection;
  if (!repair_command_parse_gun(&repair_command, buffer, false, &selection))
    return;
  loc = selection.location;
  part = selection.part;
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
  if (someone_repairing(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's repairing that part already!");
    return;
  }
  if (!equipment_is_weapon(mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player, "That's no gun!");
    return;
  }
  if (!valid_gun_pos(&(RepairCriticalSelection){
          .mech = mech, .location = loc, .position = part})) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't repair middle of a gun!");
    return;
  }
  if (someone_scrapping_part(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping it already!");
    return;
  }
  if (someone_scrapping_loc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping that section - no repairs are possible!");
    return;
  }
  if (mech_critical_is_disabled(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That gun can't be fixed yet!");
    return;
  }

  if (mech_critical_is_destroyed(mech, loc, part)) {
    if (get_weapon_crits(mech, weapon_from_equipment_index(
                                   mech_critical_part_type(mech, loc, part))) <
            5 ||
        mech_critical_is_destroyed(mech, loc, part + 1)) {
      mecha_notify(evaluation, player, "That gun is gone for good!");
      return;
    }
    extra_hard = 1;
  } else if (!mech_critical_temporary_failure(mech, loc, part)) {
    mecha_notify(evaluation, player, "That gun isn't hurtin'!");
    return;
  }

  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  RepairPartJob job = {
      .difficulty = REPAIR_DIFFICULTY +
                    repair_weapon_type_difficulty(
                        mech_critical_part_type(mech, loc, part)) +
                    extra_hard,
      .time = REPAIRGUN_TIME,
      .event_data = repair_event_payload_pack(
          (RepairEventPayload){.location = loc, .position = part}),
      .event_type = EVENT_REPAIR_REPAP,
      .event_callback = mux_event_tickmech_repairgun,
      .message = "You start repairing the weapon..",
      .weapon_roll = true,
      .resource = repair_econ,
      .failure = repairg_fail,
      .success = repairg_succ,
  };
  (void)repair_part_job_execute(&repair_command, loc, part, &job);
}

void tech_fixenhcrit(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc;
  int part;
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
  /* Find the gun for us */
  RepairSelection selection;
  if (!repair_command_parse_gun(&repair_command, buffer, false, &selection))
    return;
  loc = selection.location;
  part = selection.part;
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
  if (someone_repairing(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's repairing that part already!");
    return;
  }
  if (!equipment_is_weapon(mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player, "That's no gun!");
    return;
  }
  if (someone_scrapping_part(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping it already!");
    return;
  }
  if (someone_scrapping_loc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping that section - no repairs are possible!");
    return;
  }
  if (mech_critical_is_disabled(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That gun can't be fixed yet!");
    return;
  }

  if (!mech_critical_is_damaged(mech, loc, part)) {
    mecha_notify(evaluation, player, "That gun isn't damaged!");
    return;
  }

  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  RepairPartJob job = {
      .difficulty = ENHCRIT_DIFFICULTY,
      .time = REPAIRENHCRIT_TIME,
      .event_data = repair_event_payload_pack(
          (RepairEventPayload){.location = loc, .position = part}),
      .event_type = EVENT_REPAIR_REPENHCRIT,
      .event_callback = mux_event_tickmech_repairenhcrit,
      .message = "You start repairing the weapon...",
      .weapon_roll = true,
      .resource = repairenhcrit_econ,
      .failure = repairenhcrit_fail,
      .success = repairenhcrit_succ,
  };
  (void)repair_part_job_execute(&repair_command, loc, part, &job);
}

void tech_replacepart(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc;
  int part;
  int t;

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

  int roll;
  int rollmod;
  int fixtime;
  int base_fixtime;
  int parttype;
  int oparttype;
  int fail_fixtime;

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
  if (!mech_critical_is_nonfunctional(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part looks ok to me..");
    return;
  }
  if (mech_part_is_structural_placeholder(
          mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part isn't hurtin'!");
    return;
  }
  if (equipment_is_weapon(t)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That's a weapon! Use replacegun instead.");
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
  if (someone_repairing(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's repairing that part already!");
    return;
  }
  if (someone_scrapping_loc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping that section - no repairs are possible!");
    return;
  }
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  /* little cheating here to get the proper part, since we aren't doing complex
   * repairs */
  oparttype = mech_critical_part_type(mech, loc, part);
  parttype = oparttype;
  if (equipment_is_actuator(oparttype))
    parttype = cargo_equipment_index(S_ACTUATOR);
  else if (oparttype == special_equipment_index(ENGINE)) {
    if (mech_technology_flags(mech) & XL_TECH)
      parttype = cargo_equipment_index(XL_ENGINE);
    else if (mech_technology_flags(mech) & ICE_TECH)
      parttype = cargo_equipment_index(IC_ENGINE);
    else if (mech_technology_flags(mech) & XXL_TECH)
      parttype = cargo_equipment_index(XXL_ENGINE);
    else if (mech_technology_flags(mech) & CE_TECH)
      parttype = cargo_equipment_index(COMP_ENGINE);
    else if (mech_technology_flags(mech) & LE_TECH)
      parttype = cargo_equipment_index(LIGHT_ENGINE);
  } else if (oparttype == special_equipment_index(HEAT_SINK) &&
             mech_has_double_heat_sinks(mech))
    parttype = cargo_equipment_index(DOUBLE_HEAT_SINK);

  if (equipment_is_ammunition(mech_critical_part_type(mech, loc, part))
          ? 0
          : econ_find_items(
                context,
                mech_is_dropship(mech)
                    ? mech_bay_dbref(mech, 0)
                    : game_object_location(btech_context_database(context),
                                           mech_dbref(mech)),
                parttype, mech_critical_brand(mech, loc, part)) < 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 tprintf("Not enough units of %s in store.",
                         part_name(context, parttype,
                                   mech_critical_brand(mech, loc, part))
                             .text));
    return;
  }

  notify_printf(evaluation, player, "You start replacing the part...");
  rollmod = REPLACE_DIFFICULTY + repair_part_type_difficulty(
                                     mech_critical_part_type(mech, loc, part));
  roll = tech_roll(player, mech, rollmod);
  base_fixtime = REPLACEPART_TIME;
  fail_fixtime = (REPLACEPART_TIME * 3) / 2;

  if (roll < 0) {
    notify_printf(
        evaluation, player,
        "Your attempt is unsuccessful, but you try to save the part...");
    rollmod = rollmod + 1;
    roll = tech_roll(player, mech, rollmod);
    if (roll < 0) {
      fixtime = fail_fixtime;
      notify_printf(evaluation, player,
                    "You muck around, wasting the part for good...");
      /* part goes , 1.5 * techtime*/
      economy_inventory_change(&(EconomyInventoryChange){
          .context = context,
          .store = mech_is_dropship(mech)
                       ? mech_bay_dbref(mech, 0)
                       : game_object_location(btech_context_database(context),
                                              mech_dbref(mech)),
          .part = {.id = parttype,
                   .brand = mech_critical_brand(mech, loc, part)},
          .quantity_delta = -1,
      });
      tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPL, mech_event_failure_marker,
          max(1, player_techtime(context, player) * TECH_TICK),
          repair_event_payload_pack(
              (RepairEventPayload){.location = loc, .position = part}) +
              (player * PLAYERPOS));

    } else {
      notify_printf(evaluation, player, "You manage to save the part...");
      /* part doesn't go. 1.5 * techtime, but lets mod the fix time if
       * applicable*/
      /* We should really MIN(100,mod * roll) for the subtract to cap this out
       */
      if (roll == 0)
        fixtime = fail_fixtime;
      else {
        fixtime =
            btech_context_uses_variable_technology_time(context)
                ? (fail_fixtime * 10) /
                      (1000 /
                       (100 - (roll ? btech_context_technology_time_modifier(
                                          context) *
                                          roll
                                    : 0)))
                : fail_fixtime;
      }
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPL, mech_event_failure_marker,
          max(1, player_techtime(context, player) * TECH_TICK),
          repair_event_payload_pack(
              (RepairEventPayload){.location = loc, .position = part}) +
              (player * PLAYERPOS));
    }

  } else {
    if (roll == 0)
      fixtime = base_fixtime;
    else {
      fixtime =
          btech_context_uses_variable_technology_time(context)
              ? (base_fixtime * 10) /
                    (1000 /
                     (100 -
                      (roll ? btech_context_technology_time_modifier(context) *
                                  roll
                            : 0)))
              : base_fixtime;
    }
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");

    economy_inventory_change(&(EconomyInventoryChange){
        .context = context,
        .store = mech_is_dropship(mech)
                     ? mech_bay_dbref(mech, 0)
                     : game_object_location(btech_context_database(context),
                                            mech_dbref(mech)),
        .part = {.id = parttype, .brand = mech_critical_brand(mech, loc, part)},
        .quantity_delta = -1,
    });
    tech_addtechtime(&(TechTimeAddition){
        .context = context, .player = player, .units = fixtime});
    btech_context_event_schedule(
        context, mech, EVENT_REPAIR_REPL, mux_event_tickmech_repairpart,
        max(1, player_techtime(context, player) * TECH_TICK),
        repair_event_payload_pack(
            (RepairEventPayload){.location = loc, .position = part}) +
            (player * PLAYERPOS));
  }
}

void tech_repairpart(DbRef player, void *data, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  int loc;
  int part;
  int t;

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
  if (mech_critical_is_destroyed(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part is gone for good!");
    return;
  }
  if (mech_critical_is_disabled(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part can't be repaired yet!");
    return;
  }
  if (!mech_critical_temporary_failure(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part isn't hurtin'!");
    return;
  }
  if (mech_part_is_structural_placeholder(
          mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That part isn't hurtin'!");
    return;
  }
  if (equipment_is_weapon(t)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That's a weapon! Use repairgun instead.");
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
  if (someone_repairing(mech, loc, part)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's repairing that part already!");
    return;
  }
  if (someone_scrapping_loc(mech, loc)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's scrapping that section - no repairs are possible!");
    return;
  }
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  RepairPartJob job = {
      .difficulty =
          REPAIR_DIFFICULTY +
          repair_part_type_difficulty(mech_critical_part_type(mech, loc, part)),
      .time = REPAIRPART_TIME,
      .event_data = repair_event_payload_pack(
          (RepairEventPayload){.location = loc, .position = part}),
      .event_type = EVENT_REPAIR_REPAP,
      .event_callback = mux_event_tickmech_repairpart,
      .message = "You start repairing the part..",
      .resource = repair_econ,
      .failure = repairp_fail,
      .success = repairp_succ,
  };
  (void)repair_part_job_execute(&repair_command, loc, part, &job);
}
