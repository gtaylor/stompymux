/* Implements BattleTech repair mechanics for unit tech events. */

#include "btech_event.h"
#include "checked_conversion.h"
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
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "repair_job.h"
#include "section_types.h"
#include <stdint.h>

static int completely_intact_int(Mech *mech) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++)
    if (!mech_section_internal(mech, i) &&
        mech_section_original_internal(mech, i))
      return 0;
  return 1;
}

void mux_event_tickmech_removesection(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int earg = clamp_intptr_to_int((intptr_t)e->data2) % PLAYERPOS;
  char buf[MBUF_SIZE];
  int loc, extra;

  /* changed Special2I to Special on AddPartsM statements */
  loc = earg % LOCMAX;
  extra = earg / (LOCMAX * POSMAX);
  if (extra == 0)
    return;
#ifndef BT_COMPLEXREPAIRS
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 tech_proper_internal_part(mech), 0,
                 (2 * mech_section_internal(mech, loc)) / extra);
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED, tech_proper_armor_part(mech),
                 0, (2 * mech_section_armor(mech, loc)) / extra);
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 cargo_equipment_index(S_ELECTRONIC), 0,
                 mech_section_internal(mech, loc) / extra);
#else
  mech_parts_add(mech, loc, tech_proper_internal_part(mech), 0,
                 (2 * mech_section_internal(mech, loc)) / extra);
  mech_parts_add(mech, loc, tech_proper_armor_part(mech), 0,
                 (2 * mech_section_armor(mech, loc)) / extra);
  mech_parts_add(mech, loc, cargo_equipment_index(S_ELECTRONIC), 0,
                 mech_section_internal(mech, loc) / extra);
#endif
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
  long earg = (long)(e->data2) % PLAYERPOS;
  int loc, pos, i, extra;
  char buf[MBUF_SIZE];
  int count = 0, nloc, ncrit;
  int size;

  RepairEventPayload payload = repair_event_payload_unpack(earg);
  loc = payload.location;
  pos = payload.position;
  extra = payload.extra;

  size = get_weapon_crits(mech, weapon_from_equipment_index(
                                    mech_critical_part_type(mech, loc, pos)));
  for (i = pos; i < min(NUM_CRITICALS, pos + size); i++) {
    mech_critical_destroy(mech, loc, i);
    count++;
  }
  if (count < size && mech_class(mech) == CLASS_MECH) { // got split crits
    SplitCriticalLookup split_lookup =
        split_critical_find(mech, (CriticalSlotReference){loc, pos});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (size - count); i++) {
        mech_critical_destroy(mech, nloc, i);
      }
    }
  }

  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  if (extra == 2 && (e->function != mech_event_failure_marker)) {
#ifndef BT_COMPLEXREPAIRS
    mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                   find_ammo_type(mech, loc, pos),
                   mech_critical_brand(mech, loc, pos), 1);
#else
    mech_parts_add(mech, loc, FindAmmoType(mech, loc, pos),
                   mech_critical_brand(mech, loc, pos), 1);
#endif
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
  long earg = (long)(e->data2) % PLAYERPOS;
  int loc, pos, extra;
  char buf[MBUF_SIZE];

  RepairEventPayload payload = repair_event_payload_unpack(earg);
  loc = payload.location;
  pos = payload.position;
  extra = payload.extra;
  mech_critical_destroy(mech, loc, pos);
  if (mech_class(mech) == CLASS_MECH)
    do_magic(mech);
  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  if (extra == 2 && (e->function != mech_event_failure_marker)) {
#ifndef BT_COMPLEXREPAIRS
    mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                   find_ammo_type(mech, loc, pos),
                   mech_critical_brand(mech, loc, pos), 1);
#else
    mech_parts_add(mech, loc, FindAmmoType(mech, loc, pos),
                   mech_critical_brand(mech, loc, pos), 1);
#endif
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

void mux_event_tickmech_repairarmor(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int earg = clamp_intptr_to_int((intptr_t)e->data2) % PLAYERPOS;
  int loc = earg % 16;
  int amount = (earg / 16) % 256;
  DbRef player = (DbRef)((intptr_t)e->data2 / PLAYERPOS);
  char buf[MBUF_SIZE];

  if (loc >= 8) {
    mech_section_rear_armor_set(
        mech, loc % 8,
        min(mech_section_rear_armor(mech, loc % 8) + 1,
            mech_section_original_rear_armor(mech, loc % 8)));
  } else {
    mech_section_armor_set(mech, loc,
                           min(mech_section_armor(mech, loc) + 1,
                               mech_section_original_armor(mech, loc)));
  }
  amount--;
  if (amount < 0)
    return;
  if (amount <= 0) {
    armor_string_from_index(loc % 8, buf, mech_class(mech),
                            mech_movement_type(mech));
    if (loc >= 8) {
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

    if (mech_class(mech) != CLASS_MECH && completely_intact_int(mech))
      do_magic(mech);
    return;
  }
  repair_event_schedule_minutes(&(RepairEventSchedule){
      .mech = mech,
      .delay = FIXARMOR_TIME,
      .event_type = EVENT_REPAIR_FIX,
      .callback = mux_event_tickmech_repairarmor,
      .payload = {.location = loc, .position = amount, .player = player}});
}

void mux_event_tickmech_repairinternal(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int earg = clamp_intptr_to_int((intptr_t)e->data2) % PLAYERPOS;
  int loc = earg % 16;
  int amount = (earg / 16) % 256;
  DbRef player = (DbRef)((intptr_t)e->data2 / PLAYERPOS);
  char buf[MBUF_SIZE];

  mech_section_internal_set(mech, loc, mech_section_internal(mech, loc) + 1);
  if (mech_section_internal(mech, loc) >
      mech_section_original_internal(mech, loc))
    mech_section_internal_set(mech, loc,
                              mech_section_original_internal(mech, loc));
  amount--;
  if (amount < 0)
    return;
  if (amount <= 0) {
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
    if (mech_class(mech) != CLASS_MECH && completely_intact_int(mech))
      do_magic(mech);
    return;
  }
  repair_event_schedule_minutes(&(RepairEventSchedule){
      .mech = mech,
      .delay = FIXINTERNAL_TIME,
      .event_type = EVENT_REPAIR_FIXI,
      .callback = mux_event_tickmech_repairinternal,
      .payload = {.location = loc, .position = amount, .player = player}});
}

void mux_event_tickmech_reattach(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int earg = clamp_intptr_to_int((intptr_t)e->data2) % PLAYERPOS;
  char buf[MBUF_SIZE];

  /* Basically: Unset the limb destroyed, without doing a thing to
     damaged parts */
  mech_re_attach(mech, earg);
  armor_string_from_index(earg, buf, mech_class(mech),
                          mech_movement_type(mech));
  if (completely_intact_int(mech))
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
  int earg = clamp_intptr_to_int((intptr_t)e->data2) % PLAYERPOS;
  char buf[MBUF_SIZE];

  armor_string_from_index(earg, buf, mech_class(mech),
                          mech_movement_type(mech));
  mech_replace_suit(mech, earg);
  do_magic(mech);

  mech_printf(mech, MECHALL, "%s has been replaced.", buf);
}

/*
 * Added for new flood code by Kipsta
 * 8/4/99
 */

void mux_event_tickmech_reseal(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int earg = clamp_intptr_to_int((intptr_t)e->data2) % PLAYERPOS;
  char buf[MBUF_SIZE];

  mech_re_seal(mech, earg);
  armor_string_from_index(earg, buf, mech_class(mech),
                          mech_movement_type(mech));
  mech_printf(mech, MECHALL, "%s has been resealed.", buf);
}

void mux_event_tickmech_replacegun(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int earg = clamp_intptr_to_int((intptr_t)e->data2) % PLAYERPOS;
  int loc, pos, i, brand;
  char buf[MBUF_SIZE];
  int count = 0, nloc, ncrit;
  int size;

  RepairEventPayload payload = repair_event_payload_unpack(earg);
  loc = payload.location;
  pos = payload.position;
  brand = payload.extra;

  size = get_weapon_crits(mech, weapon_from_equipment_index(
                                    mech_critical_part_type(mech, loc, pos)));
  for (i = pos; i < min(NUM_CRITICALS, pos + size); i++) {
    mech_repair_part(mech, loc, i);
    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = mech, .slot = {.section = loc, .critical = i}, .failure = 0});
    if (brand) {
      mech_critical_brand_set(
          &(CriticalSlotBrandSet){.mech = mech,
                                  .slot = {.section = loc, .critical = i},
                                  .brand = brand});
    }
    count++;
  }
  if (count < size && mech_class(mech) == CLASS_MECH) { // got split crits
    SplitCriticalLookup split_lookup =
        split_critical_find(mech, (CriticalSlotReference){loc, pos});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (size - count); i++) {
        mech_repair_part(mech, nloc, i);
        mech_critical_temporary_failure_set(
            &(CriticalSlotFailureSet){.mech = mech,
                                      .slot = {.section = nloc, .critical = i},
                                      .failure = 0});
        if (brand) {
          mech_critical_brand_set(
              &(CriticalSlotBrandSet){.mech = mech,
                                      .slot = {.section = nloc, .critical = i},
                                      .brand = brand});
        }
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
  long earg = (long)(e->data2) % PLAYERPOS;
  int loc, pos, i;
  char buf[MBUF_SIZE];
  int count = 0, nloc, ncrit;
  int size;

  RepairEventPayload payload = repair_event_payload_unpack(earg);
  loc = payload.location;
  pos = payload.position;

  size = get_weapon_crits(mech, weapon_from_equipment_index(
                                    mech_critical_part_type(mech, loc, pos)));
  for (i = pos; i < min(NUM_CRITICALS, pos + size); i++) {
    mech_repair_part(mech, loc, i);
    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = mech, .slot = {.section = loc, .critical = i}, .failure = 0});
    count++;
  }
  if (count < size && mech_class(mech) == CLASS_MECH) { // got split crits
    SplitCriticalLookup split_lookup =
        split_critical_find(mech, (CriticalSlotReference){loc, pos});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (size - count); i++) {
        mech_repair_part(mech, nloc, i);
        mech_critical_temporary_failure_set(
            &(CriticalSlotFailureSet){.mech = mech,
                                      .slot = {.section = nloc, .critical = i},
                                      .failure = 0});
      }
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
  long earg = (long)(e->data2) % PLAYERPOS;
  int loc, pos;
  char buf[MBUF_SIZE];
  int w_crit_type, w_weap_size, w_first_crit;

  RepairEventPayload payload = repair_event_payload_unpack(earg);
  loc = payload.location;
  pos = payload.position;
  armor_string_from_index(loc, buf, mech_class(mech), mech_movement_type(mech));
  mech_printf(mech, MECHALL, "%s on %s has been repaired.",
              pos_part_name(mech, loc, pos).text, buf);
  mech_critical_damage_repair(mech, loc, pos);

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

  mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
      .mech = mech,
      .slot = {.section = loc, .critical = w_first_crit},
      .failure = 0});
}

void mux_event_tickmech_repairpart(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long earg = (long)(e->data2) % PLAYERPOS;
  int loc, pos;
  char buf[MBUF_SIZE];

  RepairEventPayload payload = repair_event_payload_unpack(earg);
  loc = payload.location;
  pos = payload.position;
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
  long earg = (long)(e->data2) % PLAYERPOS;
  int loc, pos, extra;
  char buf[MBUF_SIZE];

  RepairEventPayload payload = repair_event_payload_unpack(earg);
  loc = payload.location;
  pos = payload.position;
  extra = payload.extra;
  if (extra) {
    mech_critical_data_set(mech, loc, pos, 0);
    if (extra > 1)
#ifndef BT_COMPLEXREPAIRS
      mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                     find_ammo_type(mech, loc, pos),
                     mech_critical_brand(mech, loc, pos), 1);
#else
      mech_parts_add(mech, loc, FindAmmoType(mech, loc, pos),
                     mech_critical_brand(mech, loc, pos), 1);
#endif
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

void mux_event_tickmech_mountbomb(MuxEvent *e) {

  /*    MECH *mech = (MECH *) e->data; */

  /*    int earg = (int) (e->data2) % PLAYERPOS; */
}

void mux_event_tickmech_umountbomb(MuxEvent *e) {

  /*    MECH *mech = (MECH *) e->data; */

  /*    int earg = (int) (e->data2) % PLAYERPOS; */
}
