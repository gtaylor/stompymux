/* Implements BattleTech repair mechanics for unit tech events. */

#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "repair_gun_layout.h"
#include "repair_job.h"
#include "section_types.h"
#include <stdint.h>

static bool repair_section_location_valid(int location) {
  return (location >= 0 && location < NUM_SECTIONS) != 0;
}

static bool repair_armor_location_valid(int location) {
  return (location >= 0 && location < (NUM_SECTIONS * 2)) != 0;
}

static bool repair_critical_location_valid(Mech *mech, int location,
                                           int position) {
  return (repair_section_location_valid(location) && position >= 0 &&
          position < mech_section_critical_count(mech, location)) != 0;
}

static RepairEventPayload repair_event_payload(const MuxEvent *event) {
  return repair_event_payload_unpack((intptr_t)event->data2);
}

static bool completely_intact(Mech *mech) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++)
    if (!mech_section_internal(mech, i) &&
        mech_section_original_internal(mech, i))
      return false;
  return true;
}

void mux_event_tickmech_removesection(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  char buf[MBUF_SIZE];
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int extra = payload.extra;

  /* changed Special2I to Special on AddPartsM statements */
  if (!repair_section_location_valid(loc) || (extra != 2 && extra != 3) ||
      mech_section_is_destroyed(mech, loc))
    return;
  mech_parts_add(mech, tech_proper_internal_part(mech), 0,
                 (2 * mech_section_internal(mech, loc)) / extra);
  mech_parts_add(mech, tech_proper_armor_part(mech), 0,
                 (2 * mech_section_armor(mech, loc)) / extra);
  mech_parts_add(mech, cargo_equipment_index(S_ELECTRONIC), 0,
                 mech_section_internal(mech, loc) / extra);
  mech_detach(mech, loc);
  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  do {
    int i = 0;

    if (mech_is_destroyed(mech))
      i = 1;
    mech_destroyed_set(mech, false);
    mech_printf(mech, MECHALL, "%s has been removed.", buf);
    if (i)
      mech_destroyed_set(mech, true);
  } while (0);
}

void mux_event_tickmech_removegun(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int pos = payload.position;
  int i;
  int extra = payload.extra;
  char buf[MBUF_SIZE];
  int count = 0;
  int nloc = -1;
  int ncrit = -1;
  int size;
  int local_count;
  SplitCriticalLookup split_lookup = {.found = false};

  if (!repair_critical_location_valid(mech, loc, pos) ||
      (extra != 2 && extra != 3) ||
      !equipment_is_weapon(mech_critical_part_type(mech, loc, pos)) ||
      mech_critical_is_destroyed(mech, loc, pos) ||
      mech_section_is_destroyed(mech, loc) ||
      mech_section_is_flooded(mech, loc) ||
      !valid_gun_pos(&(RepairCriticalSelection){
          .mech = mech, .location = loc, .position = pos}))
    return;

  size = get_weapon_crits(mech, weapon_from_equipment_index(
                                    mech_critical_part_type(mech, loc, pos)));
  local_count = min(mech_section_critical_count(mech, loc) - pos, size);
  if (local_count < size) {
    if (mech_class(mech) != CLASS_MECH)
      return;
    split_lookup = split_critical_find(mech, (CriticalSlotReference){loc, pos});
    nloc = split_lookup.slot.section;
    ncrit = split_lookup.slot.critical;
    if (!split_lookup.found || !repair_section_location_valid(nloc) ||
        ncrit < 0 || mech_section_is_destroyed(mech, nloc) ||
        mech_section_is_flooded(mech, nloc) ||
        ncrit + (size - local_count) > mech_section_critical_count(mech, nloc))
      return;
  }
  for (i = pos; i < pos + local_count; i++)
    if (mech_critical_is_destroyed(mech, loc, i))
      return;
  if (local_count < size)
    for (i = ncrit; i < ncrit + (size - local_count); i++)
      if (mech_critical_is_destroyed(mech, nloc, i))
        return;
  for (i = pos; i < min(mech_section_critical_count(mech, loc), pos + size);
       i++) {
    mech_critical_destroy(mech, loc, i);
    count++;
  }
  if (count < size && mech_class(mech) == CLASS_MECH) { // got split crits
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      if (!repair_section_location_valid(nloc) || ncrit < 0)
        return;
      for (i = ncrit; i < min(mech_section_critical_count(mech, nloc),
                              ncrit + (size - count));
           i++) {
        mech_critical_destroy(mech, nloc, i);
      }
    }
  }

  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  if (extra == 2 && (e->function != mech_event_failure_marker)) {
    mech_parts_add(mech, find_ammo_type(mech, loc, pos),
                   mech_critical_brand(mech, loc, pos), 1);
    do {
      int was_destroyed = 0;

      if (mech_is_destroyed(mech))
        was_destroyed = 1;
      mech_destroyed_set(mech, false);
      mech_printf(mech, MECHALL, "%s from %s has been removed.",
                  pos_part_name(mech, loc, pos).text, buf);
      if (was_destroyed)
        mech_destroyed_set(mech, true);
    } while (0);
  } else {
    do {
      int was_destroyed = 0;

      if (mech_is_destroyed(mech))
        was_destroyed = 1;
      mech_destroyed_set(mech, false);
      mech_printf(mech, MECHALL, "%s from %s has been removed and scrapped.",
                  pos_part_name(mech, loc, pos).text, buf);
      if (was_destroyed)
        mech_destroyed_set(mech, true);
    } while (0);
  }
}

void mux_event_tickmech_removepart(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int pos = payload.position;
  int extra = payload.extra;
  char buf[MBUF_SIZE];

  int part_type = repair_critical_location_valid(mech, loc, pos)
                      ? mech_critical_part_type(mech, loc, pos)
                      : EMPTY;
  if (!repair_critical_location_valid(mech, loc, pos) ||
      (extra != 2 && extra != 3) || part_type == EMPTY ||
      equipment_is_weapon(part_type) ||
      mech_part_is_structural_placeholder(part_type) ||
      mech_critical_is_destroyed(mech, loc, pos))
    return;
  mech_critical_destroy(mech, loc, pos);
  if (mech_class(mech) == CLASS_MECH)
    do_magic(mech);
  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  if (extra == 2 && (e->function != mech_event_failure_marker)) {
    mech_parts_add(mech, find_ammo_type(mech, loc, pos),
                   mech_critical_brand(mech, loc, pos), 1);
    do {
      int i = 0;

      if (mech_is_destroyed(mech))
        i = 1;
      mech_destroyed_set(mech, false);
      mech_printf(mech, MECHALL, "%s from %s has been removed.",
                  pos_part_name(mech, loc, pos).text, buf);
      if (i)
        mech_destroyed_set(mech, true);
    } while (0);
  } else {
    do {
      int i = 0;

      if (mech_is_destroyed(mech))
        i = 1;
      mech_destroyed_set(mech, false);
      mech_printf(mech, MECHALL, "%s from %s has been removed and scrapped.",
                  pos_part_name(mech, loc, pos).text, buf);
      if (i)
        mech_destroyed_set(mech, true);
    } while (0);
  }
}

void mux_event_tickmech_scrap_failure(MuxEvent *event) {
  if (!event)
    return;
  switch (event->type) {
  case EVENT_REPAIR_SCRG:
    mux_event_tickmech_removegun(event);
    break;
  case EVENT_REPAIR_SCRP:
    mux_event_tickmech_removepart(event);
    break;
  default:
    break;
  }
}

void mux_event_tickmech_repairarmor(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int amount = repair_fix_event_amount(payload);
  DbRef player = payload.player;
  char buf[MBUF_SIZE];

  if (!repair_armor_location_valid(loc) || amount <= 0)
    return;
  int section = loc % NUM_SECTIONS;
  int current = loc >= NUM_SECTIONS ? mech_section_rear_armor(mech, section)
                                    : mech_section_armor(mech, section);
  int original = loc >= NUM_SECTIONS
                     ? mech_section_original_rear_armor(mech, section)
                     : mech_section_original_armor(mech, section);
  if (current >= original)
    return;
  if (loc >= NUM_SECTIONS) {
    mech_section_rear_armor_set(
        mech, section,
        min(mech_section_rear_armor(mech, section) + 1,
            mech_section_original_rear_armor(mech, section)));
  } else {
    mech_section_armor_set(mech, loc,
                           min(mech_section_armor(mech, loc) + 1,
                               mech_section_original_armor(mech, loc)));
  }
  amount--;
  if (amount < 0)
    return;
  current = loc >= NUM_SECTIONS ? mech_section_rear_armor(mech, section)
                                : mech_section_armor(mech, section);
  if (amount <= 0 || current >= original) {
    armor_string_from_index(section, buf, mech_class(mech),
                            mech_movement_type(mech));
    if (loc >= NUM_SECTIONS) {
      do {
        int i = 0;

        if (mech_is_destroyed(mech))
          i = 1;
        mech_destroyed_set(mech, false);
        mech_printf(mech, MECHALL,
                    "%s's rear armor repairs have been finished.", buf);
        if (i)
          mech_destroyed_set(mech, true);
      } while (0);
    } else {
      do {
        int i = 0;

        if (mech_is_destroyed(mech))
          i = 1;
        mech_destroyed_set(mech, false);
        mech_printf(mech, MECHALL, "%s's armor repairs have been finished.",
                    buf);
        if (i)
          mech_destroyed_set(mech, true);
      } while (0);
    }

    if (mech_class(mech) != CLASS_MECH && completely_intact(mech))
      do_magic(mech);
    return;
  }
  RepairEventPayload next_payload = {.location = loc, .player = player};
  if (!repair_fix_event_payload_with_amount(&next_payload, amount))
    return;
  repair_event_schedule_minutes(
      &(RepairEventSchedule){.mech = mech,
                             .delay = FIXARMOR_TIME,
                             .event_type = EVENT_REPAIR_FIX,
                             .callback = mux_event_tickmech_repairarmor,
                             .payload = next_payload});
}

void mux_event_tickmech_repairinternal(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int amount = repair_fix_event_amount(payload);
  DbRef player = payload.player;
  char buf[MBUF_SIZE];

  if (!repair_section_location_valid(loc) || amount <= 0)
    return;
  if (mech_section_internal(mech, loc) >=
      mech_section_original_internal(mech, loc))
    return;
  mech_section_internal_set(mech, loc, mech_section_internal(mech, loc) + 1);
  if (mech_section_internal(mech, loc) >
      mech_section_original_internal(mech, loc))
    mech_section_internal_set(mech, loc,
                              mech_section_original_internal(mech, loc));
  amount--;
  if (amount < 0)
    return;
  if (amount <= 0 || mech_section_internal(mech, loc) >=
                         mech_section_original_internal(mech, loc)) {
    armor_string_from_index(loc, buf, mech_class(mech),
                            mech_movement_type(mech));
    do {
      int i = 0;

      if (mech_is_destroyed(mech))
        i = 1;
      mech_destroyed_set(mech, false);
      mech_printf(mech, MECHALL, "%s's internal repairs have been finished.",
                  buf);
      if (i)
        mech_destroyed_set(mech, true);
    } while (0);
    if (mech_class(mech) != CLASS_MECH && completely_intact(mech))
      do_magic(mech);
    return;
  }
  RepairEventPayload next_payload = {.location = loc, .player = player};
  if (!repair_fix_event_payload_with_amount(&next_payload, amount))
    return;
  repair_event_schedule_minutes(
      &(RepairEventSchedule){.mech = mech,
                             .delay = FIXINTERNAL_TIME,
                             .event_type = EVENT_REPAIR_FIXI,
                             .callback = mux_event_tickmech_repairinternal,
                             .payload = next_payload});
}

void mux_event_tickmech_reattach(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int location = payload.location;
  char buf[MBUF_SIZE];

  if (!repair_section_location_valid(location) ||
      !mech_section_is_destroyed(mech, location))
    return;
  /* Basically: Unset the limb destroyed, without doing a thing to
   damaged parts */
  mech_re_attach(mech, location);
  armor_string_from_index(location, buf, mech_class(mech),
                          mech_movement_type(mech));
  if (completely_intact(mech))
    do_magic(mech);
  do {
    int was_destroyed = 0;

    if (mech_is_destroyed(mech))
      was_destroyed = 1;
    mech_destroyed_set(mech, false);
    mech_printf(mech, MECHALL, "%s has been reattached.", buf);
    if (was_destroyed)
      mech_destroyed_set(mech, true);
  } while (0);
}

void mux_event_tickmech_replacesuit(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int location = payload.location;
  char buf[MBUF_SIZE];

  if (mech_class(mech) != CLASS_BSUIT ||
      !repair_section_location_valid(location) ||
      location >= mech_maximum_battle_suits(mech) ||
      !mech_section_is_destroyed(mech, location))
    return;
  armor_string_from_index(location, buf, mech_class(mech),
                          mech_movement_type(mech));
  mech_replace_suit(mech, location);
  do_magic(mech);

  mech_printf(mech, MECHALL, "%s has been replaced.", buf);
}

/*
 * Added for new flood code by Kipsta
 * 8/4/99
 */

void mux_event_tickmech_reseal(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int location = payload.location;
  char buf[MBUF_SIZE];

  if (!repair_section_location_valid(location) ||
      mech_section_is_destroyed(mech, location) ||
      !mech_section_is_flooded(mech, location))
    return;
  mech_re_seal(mech, location);
  armor_string_from_index(location, buf, mech_class(mech),
                          mech_movement_type(mech));
  mech_printf(mech, MECHALL, "%s has been resealed.", buf);
}

void mux_event_tickmech_replacegun(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int pos = payload.position;
  int i;
  int brand = payload.extra;
  char buf[MBUF_SIZE];
  RepairGunLayout layout;

  if (!repair_gun_layout_find(mech, loc, pos,
                              REPAIR_GUN_LAYOUT_REQUIRE_WEAPON |
                                  REPAIR_GUN_LAYOUT_REQUIRE_GUN_START |
                                  REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS |
                                  REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS,
                              &layout) ||
      !mech_critical_is_nonfunctional(mech, loc, pos))
    return;

  for (i = pos; i < pos + layout.local_count; i++) {
    mech_repair_part(mech, loc, i);
    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = mech, .slot = {.section = loc, .critical = i}, .failure = 0});
    if (brand) {
      mech_critical_brand_set(
          &(CriticalSlotBrandSet){.mech = mech,
                                  .slot = {.section = loc, .critical = i},
                                  .brand = brand});
    }
  }
  if (layout.local_count < layout.size) {
    for (i = layout.split.slot.critical;
         i < layout.split.slot.critical + layout.size - layout.local_count;
         i++) {
      mech_repair_part(mech, layout.split.slot.section, i);
      mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
          .mech = mech,
          .slot = {.section = layout.split.slot.section, .critical = i},
          .failure = 0});
      if (brand) {
        mech_critical_brand_set(&(CriticalSlotBrandSet){
            .mech = mech,
            .slot = {.section = layout.split.slot.section, .critical = i},
            .brand = brand});
      }
    }
  }

  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  do {
    int was_destroyed = 0;

    if (mech_is_destroyed(mech))
      was_destroyed = 1;
    mech_destroyed_set(mech, false);
    mech_printf(mech, MECHALL, "%s on %s has been replaced.",
                pos_part_name(mech, loc, pos).text, buf);
    if (was_destroyed)
      mech_destroyed_set(mech, true);
  } while (0);
}

void mux_event_tickmech_repairgun(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int pos = payload.position;
  int i;
  char buf[MBUF_SIZE];
  RepairGunLayout layout;

  if (!repair_gun_layout_find(mech, loc, pos,
                              REPAIR_GUN_LAYOUT_REQUIRE_WEAPON |
                                  REPAIR_GUN_LAYOUT_REQUIRE_GUN_START |
                                  REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS |
                                  REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS,
                              &layout) ||
      !mech_critical_is_nonfunctional(mech, loc, pos))
    return;

  for (i = pos; i < pos + layout.local_count; i++) {
    mech_repair_part(mech, loc, i);
    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = mech, .slot = {.section = loc, .critical = i}, .failure = 0});
  }
  if (layout.local_count < layout.size) {
    for (i = layout.split.slot.critical;
         i < layout.split.slot.critical + layout.size - layout.local_count;
         i++) {
      mech_repair_part(mech, layout.split.slot.section, i);
      mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
          .mech = mech,
          .slot = {.section = layout.split.slot.section, .critical = i},
          .failure = 0});
    }
  }

  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  do {
    int was_destroyed = 0;

    if (mech_is_destroyed(mech))
      was_destroyed = 1;
    mech_destroyed_set(mech, false);
    mech_printf(mech, MECHALL, "%s on %s has been repaired.",
                pos_part_name(mech, loc, pos).text, buf);
    if (was_destroyed)
      mech_destroyed_set(mech, true);
  } while (0);
}

void mux_event_tickmech_repairenhcrit(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int pos = payload.position;
  char buf[MBUF_SIZE];
  int w_crit_type;
  int w_weap_size;
  int w_first_crit;

  if (!repair_critical_location_valid(mech, loc, pos) ||
      !equipment_is_weapon(mech_critical_part_type(mech, loc, pos)) ||
      !mech_critical_is_damaged(mech, loc, pos) ||
      mech_section_is_destroyed(mech, loc) ||
      mech_section_is_flooded(mech, loc))
    return;
  /* Get the crit type */
  w_crit_type = mech_critical_part_type(mech, loc, pos);

  /* Get the max number of crits for this weapon */
  w_weap_size =
      get_weapon_crits(mech, weapon_from_equipment_index(w_crit_type));

  /* Find the first crit */
  w_first_crit = mech_weapon_first_critical(&(WeaponCriticalSearch){
      .mech = mech,
      .weapon = {.section = loc, .critical = pos},
      .start_critical = 0,
      .part_type = w_crit_type,
      .maximum_criticals = w_weap_size,
  });

  if (!repair_critical_location_valid(mech, loc, w_first_crit))
    return;
  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  mech_critical_damage_repair(mech, loc, pos);
  mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
      .mech = mech,
      .slot = {.section = loc, .critical = w_first_crit},
      .failure = 0});
  mech_printf(mech, MECHALL, "%s on %s has been repaired.",
              pos_part_name(mech, loc, pos).text, buf);
}

void mux_event_tickmech_repairpart(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int pos = payload.position;
  char buf[MBUF_SIZE];

  if (!repair_critical_location_valid(mech, loc, pos) ||
      mech_section_is_destroyed(mech, loc) ||
      mech_section_is_flooded(mech, loc))
    return;
  int part_type = mech_critical_part_type(mech, loc, pos);
  if (part_type == EMPTY || equipment_is_weapon(part_type) ||
      mech_part_is_structural_placeholder(part_type) ||
      !mech_critical_is_nonfunctional(mech, loc, pos))
    return;
  mech_repair_part(mech, loc, pos);
  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  do {
    int i = 0;

    if (mech_is_destroyed(mech))
      i = 1;
    mech_destroyed_set(mech, false);
    mech_printf(mech, MECHALL, "%s on %s has been repaired.",
                pos_part_name(mech, loc, pos).text, buf);
    if (i)
      mech_destroyed_set(mech, true);
  } while (0);
}

void mux_event_tickmech_reload(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  RepairEventPayload payload = repair_event_payload(e);
  int loc = payload.location;
  int pos = payload.position;
  int extra = payload.extra;
  char buf[MBUF_SIZE];

  if (!repair_critical_location_valid(mech, loc, pos) ||
      !equipment_is_ammunition(mech_critical_part_type(mech, loc, pos)) ||
      mech_section_is_destroyed(mech, loc) ||
      mech_section_is_flooded(mech, loc) ||
      mech_critical_is_nonfunctional(mech, loc, pos) || extra < 0 || extra > 2)
    return;
  int current = mech_critical_data(mech, loc, pos);
  if ((extra != 0 && current <= 0) ||
      (extra == 0 && current >= full_ammo(mech, loc, pos)))
    return;
  if (extra) {
    mech_critical_data_set(mech, loc, pos, 0);
    if (extra > 1)
      mech_parts_add(mech, find_ammo_type(mech, loc, pos),
                     mech_critical_brand(mech, loc, pos), 1);
  } else {
    mech_fill_part_ammo(mech, loc, pos);
  }
  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  do {
    int i = 0;

    if (mech_is_destroyed(mech))
      i = 1;
    mech_destroyed_set(mech, false);
    mech_printf(mech, MECHALL, "%s on %s has been %sloaded.",
                pos_part_name(mech, loc, pos).text, buf, extra ? "un" : "re");
    if (i)
      mech_destroyed_set(mech, true);
  } while (0);
}

void mux_event_tickmech_mountbomb(MuxEvent *e [[maybe_unused]]) {

  /*    MECH *mech = (MECH *) e->data; */

  /*    int earg = (int) (e->data2) % PLAYERPOS; */
}

void mux_event_tickmech_umountbomb(MuxEvent *e [[maybe_unused]]) {

  /*    MECH *mech = (MECH *) e->data; */

  /*    int earg = (int) (e->data2) % PLAYERPOS; */
}
