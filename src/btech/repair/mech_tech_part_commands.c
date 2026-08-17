#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
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
#include "registry_api.h"
#include "repair_gun_layout.h"
#include "repair_job.h"
#include "section_types.h"

static int clan_modified_time(const Mech *mech, int time) {
  return max(1, time / ((mech_technology_flags(mech) & CLAN_TECH) ? 2 : 1));
}

// NOLINTNEXTLINE(bugprone-easily-swappable-parameters)
static bool gun_footprint_is_busy(Mech *mech, int location, int position,
                                  const RepairGunLayout *footprint,
                                  bool include_repairs) {
  for (int critical = position; critical < position + footprint->local_count;
       critical++) {
    if ((include_repairs && someone_repairing(mech, location, critical)) ||
        someone_scrapping_part(mech, location, critical))
      return true;
  }
  if (footprint->local_count >= footprint->size)
    return false;
  int split_end =
      footprint->split.slot.critical + footprint->size - footprint->local_count;
  for (int critical = footprint->split.slot.critical; critical < split_end;
       critical++) {
    if ((include_repairs &&
         someone_repairing(mech, footprint->split.slot.section, critical)) ||
        someone_scrapping_part(mech, footprint->split.slot.section, critical))
      return true;
  }
  return someone_scrapping_loc(mech, footprint->split.slot.section) != 0;
}

// NOLINTNEXTLINE(bugprone-easily-swappable-parameters)
static bool gun_next_critical_destroyed(Mech *mech, int location, int position,
                                        int size) {
  if (position + 1 < mech_section_critical_count(mech, location))
    return mech_critical_is_destroyed(mech, location, position + 1);
  if (size <= 1 || mech_class(mech) != CLASS_MECH)
    return true;
  SplitCriticalLookup split = split_critical_find(
      mech, (CriticalSlotReference){.section = location, .critical = position});
  return (!split.found || split.slot.section < 0 ||
          split.slot.section >= NUM_SECTIONS || split.slot.critical < 0 ||
          split.slot.critical >=
              mech_section_critical_count(mech, split.slot.section) ||
          mech_critical_is_destroyed(mech, split.slot.section,
                                     split.slot.critical)) != 0;
}

void tech_replacegun(DbRef player, Mech *facility, char *buffer) {
  int brand = 0;
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
      player, facility, REPAIR_STALL_REQUIRED, &repair_command);
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
  RepairGunLayout footprint;
  if (!repair_gun_layout_find(mech, loc, part,
                              REPAIR_GUN_LAYOUT_REQUIRE_WEAPON |
                                  REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS |
                                  REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS,
                              &footprint)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That gun's critical layout is incomplete!");
    return;
  }
  if (gun_footprint_is_busy(mech, loc, part, &footprint, true)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's already working on that gun!");
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

  int replacement_brand = brand ? brand : mech_critical_brand(mech, loc, part);

  parttype = mech_critical_part_type(mech, loc, part);

  if (equipment_is_ammunition(mech_critical_part_type(mech, loc, part))
          ? 0
          : econ_find_items(
                context,
                mech_is_dropship(mech)
                    ? mech_bay_dbref(mech, 0)
                    : game_object_location(btech_context_database(context),
                                           mech_dbref(mech)),
                parttype, replacement_brand) < 1) {
    mecha_notifyf(btech_context_evaluation(context), player,
                  "Not enough units of %s in store.",
                  part_name(context, parttype, replacement_brand).text);
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
      if (!(equipment_is_ammunition(
              mech_critical_part_type(mech, loc, part)))) {
        economy_inventory_change(&(EconomyInventoryChange){
            .context = context,
            .store = mech_is_dropship(mech)
                         ? mech_bay_dbref(mech, 0)
                         : game_object_location(btech_context_database(context),
                                                mech_dbref(mech)),
            .part = {.id = parttype, .brand = replacement_brand},
            .quantity_delta = -1,
        });
      }
      int delay = tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPLG, mech_event_failure_marker, delay,
          repair_event_payload_pack((RepairEventPayload){.location = loc,
                                                         .position = part,
                                                         .extra = brand,
                                                         .player = player}));

    } else {
      notify_printf(evaluation, player, "You manage to save the gun...");
      fixtime = tech_adjusted_time_for_roll(context, fail_fixtime, roll);
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      int delay = tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPLG, mech_event_failure_marker, delay,
          repair_event_payload_pack((RepairEventPayload){.location = loc,
                                                         .position = part,
                                                         .extra = brand,
                                                         .player = player}));
    }

  } else {
    fixtime = tech_adjusted_time_for_roll(context, base_fixtime, roll);
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
          .part = {.id = parttype, .brand = replacement_brand},
          .quantity_delta = -1,
      });
    }
    int delay = tech_addtechtime(&(TechTimeAddition){
        .context = context, .player = player, .units = fixtime});
    btech_context_event_schedule(
        context, mech, EVENT_REPAIR_REPLG, mux_event_tickmech_replacegun, delay,
        repair_event_payload_pack((RepairEventPayload){.location = loc,
                                                       .position = part,
                                                       .extra = brand,
                                                       .player = player}));
  }
}

void tech_repairgun(DbRef player, Mech *facility, char *buffer) {
  int extra_hard = 0;

  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc;
  int part;
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
  evaluation = repair_command.evaluation;
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
  RepairGunLayout footprint;
  if (!repair_gun_layout_find(mech, loc, part,
                              REPAIR_GUN_LAYOUT_REQUIRE_WEAPON |
                                  REPAIR_GUN_LAYOUT_REQUIRE_GUN_START |
                                  REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS |
                                  REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS,
                              &footprint)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That gun's critical layout is incomplete!");
    return;
  }
  if (gun_footprint_is_busy(mech, loc, part, &footprint, true)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's already working on that gun!");
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
    int size = get_weapon_crits(
        mech,
        weapon_from_equipment_index(mech_critical_part_type(mech, loc, part)));
    if (size < 5 || gun_next_critical_destroyed(mech, loc, part, size)) {
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
      .event_type = EVENT_REPAIR_REPAG,
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
  if (!equipment_is_weapon(mech_critical_part_type(mech, loc, part))) {
    mecha_notify(btech_context_evaluation(context), player, "That's no gun!");
    return;
  }
  int weapon_type = mech_critical_part_type(mech, loc, part);
  int weapon_size =
      get_weapon_crits(mech, weapon_from_equipment_index(weapon_type));
  int first = mech_weapon_first_critical(&(WeaponCriticalSearch){
      .mech = mech,
      .weapon = {.section = loc, .critical = part},
      .start_critical = 0,
      .part_type = weapon_type,
      .maximum_criticals = weapon_size,
  });
  RepairGunLayout footprint;
  if (first < 0 ||
      !repair_gun_layout_find(mech, loc, first,
                              REPAIR_GUN_LAYOUT_REQUIRE_WEAPON |
                                  REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS |
                                  REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS,
                              &footprint)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That gun's critical layout is incomplete!");
    return;
  }
  if (gun_footprint_is_busy(mech, loc, first, &footprint, true)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone's already working on that gun!");
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

void tech_replacepart(DbRef player, Mech *facility, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  EvaluationContext *evaluation;
  int loc;
  int part;
  int t;

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
  evaluation = repair_command.evaluation;

  int roll;
  int rollmod;
  int fixtime;
  int base_fixtime;
  int parttype;
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
  if (player_techtime(context, player) >=
      btech_context_maximum_technology_time(context)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're too tired to do that!");
    return;
  }

  parttype = mech_parts_alias(mech, mech_critical_part_type(mech, loc, part));

  if (equipment_is_ammunition(mech_critical_part_type(mech, loc, part))
          ? 0
          : econ_find_items(
                context,
                mech_is_dropship(mech)
                    ? mech_bay_dbref(mech, 0)
                    : game_object_location(btech_context_database(context),
                                           mech_dbref(mech)),
                parttype, mech_critical_brand(mech, loc, part)) < 1) {
    mecha_notifyf(
        btech_context_evaluation(context), player,
        "Not enough units of %s in store.",
        part_name(context, parttype, mech_critical_brand(mech, loc, part))
            .text);
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
      if (!equipment_is_ammunition(t)) {
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
      int delay = tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPL, mech_event_failure_marker, delay,
          repair_event_payload_pack((RepairEventPayload){
              .location = loc, .position = part, .player = player}));
    } else {
      notify_printf(evaluation, player, "You manage to save the part...");
      fixtime = tech_adjusted_time_for_roll(context, fail_fixtime, roll);
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      int delay = tech_addtechtime(&(TechTimeAddition){
          .context = context, .player = player, .units = fixtime});
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPL, mech_event_failure_marker, delay,
          repair_event_payload_pack((RepairEventPayload){
              .location = loc, .position = part, .player = player}));
    }
  } else {
    fixtime = tech_adjusted_time_for_roll(context, base_fixtime, roll);
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");
    if (!equipment_is_ammunition(t)) {
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
    int delay = tech_addtechtime(&(TechTimeAddition){
        .context = context, .player = player, .units = fixtime});
    btech_context_event_schedule(
        context, mech, EVENT_REPAIR_REPL, mux_event_tickmech_repairpart, delay,
        repair_event_payload_pack((RepairEventPayload){
            .location = loc, .position = part, .player = player}));
  }
}
void tech_repairpart(DbRef player, Mech *facility, char *buffer) {
  RepairCommandContext repair_command;
  Mech *mech;
  BtechContext *context;
  int loc;
  int part;
  int t;

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
