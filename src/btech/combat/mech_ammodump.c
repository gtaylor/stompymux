/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_ammodump_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static DumpingAmmunitionItem *ammunition_item_at(DumpingAmmunitionItem *items,
                                                 size_t index) {
  return checked_storage_at(items, MAX_WEAPONS_PER_MECH, sizeof(*items), index);
}

static void ammunition_item_add(DumpingAmmunitionItem *items, int *count,
                                BtechContext *context, int part_type,
                                int location, int slot) {
  if (*count < 0 || *count >= MAX_WEAPONS_PER_MECH)
    abort();
  DumpingAmmunitionItem *item = ammunition_item_at(items, (size_t)*count);
  item->weapon_index = ammunition_to_weapon_index(part_type);
  item->damage = weapon_maximum_ammunition_damage(context, item->weapon_index);
  item->location = location;
  item->slot = slot;
  item->part_type = part_type;
  (*count)++;
}

static void mech_dump_event(MuxEvent *ev) {
  Mech *mech = (Mech *)ev->data;
  long arg = (long)ev->data2;
  int loc;
  int i, l;
  int d, e = 0;
  char buf[SBUF_SIZE];
  int weapindx;

  if (!mech_is_started(mech))
    return;
  i = mech_class(mech) == CLASS_MECH ? 7 : 5;
  /* Global ammo droppage */
  if (!arg) {
    for (; i >= 0; i--)
      for (l = mech_section_critical_count(mech, i) - 1; l >= 0; l--)
        if (equipment_is_ammunition(mech_critical_part_type(mech, i, l)))
          if (mech_critical_data(mech, i, l))
            mech_ammunition_dump_decrease(mech, i, l, &e);
    if (e > 1)
      mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK,
                          arg);
    else {
      mech_notify(mech, MECHALL, "All ammunition dumped.");
      mech_los_broadcast(
          mech, "no longer has ammo dumping from hatches on its back.");
    }
    return;
  }
  if (arg < 256) {
    loc = (int)arg - 1;
    l = mech_section_critical_count(mech, loc);
    for (i = 0; i < l; i++)
      if (equipment_is_ammunition(mech_critical_part_type(mech, loc, i)))
        if (!mech_critical_is_nonfunctional(mech, loc, i))
          if ((d = mech_critical_data(mech, loc, i)))
            mech_ammunition_dump_decrease(mech, loc, i, &e);
    if (e > 1)
      mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK,
                          arg);
    else if (e == 1 && mech_is_started(mech)) {
      ArmorStringFromIndex(loc, buf, mech_class(mech),
                           mech_movement_type(mech));
      mech_printf(mech, MECHALL, "All ammunition in %s dumped.", buf);
      mech_los_broadcast(
          mech, "no longer has ammo dumping from hatches on its back.");
    }
    return;
  }
  if (arg < 65536) {
    weapindx = (int)(arg / 256) - 1;
    for (; i >= 0; i--)
      for (l = mech_section_critical_count(mech, i) - 1; l >= 0; l--)
        if (equipment_is_ammunition(mech_critical_part_type(mech, i, l)))
          if (ammunition_to_weapon_index(mech_critical_part_type(mech, i, l)) ==
              weapindx)
            if (mech_critical_data(mech, i, l))
              mech_ammunition_dump_decrease(mech, i, l, &e);
    if (e > 1)
      mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK,
                          arg);
    else {
      mech_printf(mech, MECHALL, "Ammunition for %s dumped!",
                  get_parts_long_name(mech_context(mech),
                                      weapon_equipment_index(weapindx), 0));
      mech_los_broadcast(
          mech, "no longer has ammo dumping from hatches on its back.");
    }
    return;
  }
  l = ((arg >> 16) & 0xFF) - 1;
  i = ((arg >> 24) & 0xFF) - 1;
  e = 0;
  if (mech_critical_data(mech, l, i))
    mech_ammunition_dump_decrease(mech, l, i, &e);
  if (e > 1) {
    mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK, arg);
  } else {
    ArmorStringFromIndex(l, buf, mech_class(mech), mech_movement_type(mech));
    mech_printf(mech, MECHALL, "Ammunition in %s crit %i dumped!", buf, i + 1);
    mech_los_broadcast(mech,
                       "no longer has ammo dumping from hatches on its back.");
  }
}

static bool mech_is_running_at_desired_speed(const Mech *mech) {
  return mech_desired_speed(mech) >
         (2.0F * mech_maximum_speed(mech) / 3.0F + 0.1F);
}

void mech_dump(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  int argc;
  char *args[2];
  int weapnum;
  int weapindx;
  int section;
  int critical;
  int ammoLoc;
  int ammoCrit;
  int loc;
  int i, l, count = 0, d;
  char buf[MBUF_SIZE];
  long type = 0;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  argc = mech_parseattributes(buffer, args, 2);
  if (argc < 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Not enough arguments to the function");
    return;
  }
  weapnum = atoi(args[0]);

  if (mech_is_jumping(mech)) {
    mech_notify(mech, MECHALL, "You can't dump ammo while jumping!");
    return;
  }
  if (mech_is_running_at_desired_speed(mech)) {
    mech_notify(mech, MECHALL, "You can't dump ammo while running!");
    return;
  }

  if (!strcasecmp(args[0], "stop")) {
    if (!mech_event_count(mech, EVENT_DUMP)) {
      mech_notify(mech, MECHALL, "You aren't dumping anything!");
      return;
    }
    mech_notify(mech, MECHALL, "Ammo dumping halted.");
    mech_event_cancel(mech, EVENT_DUMP);
    mech_los_broadcast(mech,
                       "no longer has ammo dumping from hatches on its back.");
    return;
  } else if (!strcasecmp(args[0], "all")) {
    count = 0;
    i = mech_class(mech) == CLASS_MECH ? 7 : 5;
    for (; i >= 0; i--)
      for (l = mech_section_critical_count(mech, i) - 1; l >= 0; l--)
        if (equipment_is_ammunition(mech_critical_part_type(mech, i, l)))
          if (mech_critical_data(mech, i, l))
            count++;
    if (!count) {
      mech_notify(mech, MECHALL, "You have no ammo to dump!");
      return;
    }
    if (mech_dumping_type(mech, 0)) {
      mech_notify(mech, MECHALL, "You're already dumping your ammo!");
      return;
    }
    mech_event_cancel(mech, EVENT_DUMP);
    mech_notify(mech, MECHALL, "Starting dumping of all ammunition..");
    mech_los_broadcast(mech, "starts dumping ammo from hatches on its back.");
    mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK, 0);
    return;
  } else if (!weapnum && strcmp(args[0], "0")) {
    /* Try to find hitloc instead */
    if (mech_event_count(mech, EVENT_DUMP)) {
      mech_notify(mech, MECHALL, "You're already dumping some ammo!");
      return;
    }
    loc = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                 args[0]);
    if (loc < 0) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid location or weapon number!");
      return;
    }
    ArmorStringFromIndex(loc, buf, mech_class(mech), mech_movement_type(mech));
    if (args[1]) {
      i = atoi(args[1]);
      i--;
      if (i >= 0 && i < 12) {
        if (equipment_is_ammunition(mech_critical_part_type(mech, loc, i)))
          if (!mech_critical_is_nonfunctional(mech, loc, i))
            if ((d = mech_critical_data(mech, loc, i)))
              count++;
        if (!count) {
          mech_notify(
              mech, MECHALL,
              tprintf("There is no ammunition in %s crit %i!", buf, i + 1));
          return;
        }
        type = (((i + 1) << 8) | (loc + 1));
        if (type & ~0xFFFF) {
          mech_notify(mech, MECHALL, "Internal inconsistency, dump failed!");
          return;
        }
        type = type << 16;
        mech_printf(mech, MECHALL,
                    "Starting dumping of ammunition in %s crit %i..", buf,
                    i + 1);
        mech_los_broadcast(mech,
                           "starts dumping ammo from hatches on its back.");
        mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK,
                            type);
        return;
      }
    }
    l = mech_section_critical_count(mech, loc);
    for (i = 0; i < l; i++)
      if (equipment_is_ammunition(mech_critical_part_type(mech, loc, i)))
        if (!mech_critical_is_nonfunctional(mech, loc, i))
          if ((d = mech_critical_data(mech, loc, i)))
            count++;
    if (!count) {
      mech_notify(mech, MECHALL, tprintf("There is no ammunition in %s!", buf));
      return;
    }
    type = loc + 1;
    mech_printf(mech, MECHALL, "Starting dumping of ammunition in %s..", buf);
    mech_los_broadcast(mech, "starts dumping ammo from hatches on its back.");
    mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK,
                        type);
    return;
  }
  weapindx = FindWeaponIndex(mech, weapnum);
  if (weapnum < 0)
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("CHEATER: #%d tried to crash mux with command 'dump %d'!",
                (int)player, weapnum));
  if (mech_event_count(mech, EVENT_DUMP)) {
    mech_notify(mech, MECHALL, "You're already dumping some ammo!");
    return;
  }
  if (weapindx < 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid weapon number!");
    return;
  }
  FindWeaponNumberOnMech(mech, weapnum, &section, &critical);
  if (weapon_catalogue_is_energy(weapindx) ||
      weapon_catalogue_is_hand_to_hand(weapindx)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That weapon doesn't use ammunition!");
    return;
  }
  if (!FindAmmoForWeapon_sub(mech, -1, -1, weapindx, 0, &ammoLoc, &ammoCrit, 0,
                             0)) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "You don't have any ammunition for that weapon stored on this mech!");
    return;
  }
  if (mech_critical_data(mech, ammoLoc, ammoCrit) == 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are out of ammunition for that weapon already!");
    return;
  }
  type = 256 * (weapindx + 1);
  mech_printf(mech, MECHALL, "Starting dumping %s ammunition..",
              get_parts_long_name(mech_context(mech),
                                  weapon_equipment_index(weapindx), 0));
  mech_los_broadcast(mech, "starts dumping ammo from hatches on its back.");
  mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK, type);
}

static int ammunition_dump_result(int result, int *highest_result) {
  if (*highest_result < result)
    *highest_result = result;
  return result;
}

int mech_ammunition_dump_decrease(Mech *mech, int loc, int pos, int *hm) {
  int c, index, weapindx, rem;

  /* It _is_ ammo, and contains something */

  if (equipment_is_ammunition(
          (index = mech_critical_part_type(mech, loc, pos))))
    if (!mech_critical_is_nonfunctional(mech, loc, pos))
      if ((c = mech_critical_data(mech, loc, pos))) {
        weapindx = ammunition_to_weapon_index(index);
        const int ammunition_per_ton =
            weapon_catalogue_ammunition_per_ton(weapindx);
        if (ammunition_per_ton < DUMP_SPEED) {
          if ((btech_context_event_tick(mech_context(mech)) %
               (DUMP_SPEED / ammunition_per_ton)))
            return ammunition_dump_result(2, hm);
          /* fine, we remove 1 */
          rem = 1;
        } else
          rem = MIN(c, ammunition_per_ton / DUMP_SPEED);
        mech_ammunition_expenditure_check(mech, weapindx, rem - 1);
        mech_critical_data_set(mech, loc, pos, c - rem);
        if (c <= rem)
          return ammunition_dump_result(1, hm);
        return ammunition_dump_result(2, hm);
      }
  return 0;
}

/*
 * The function is for blowing up some of the ammo that's being dumped from
 * a mech when it takes a rear torso shot.
 *
 * FASA rules state that if a mech takes a rear torso shot while dumping ammo,
 * all the dumping ammo explodes and goes to the armor of that location. That's
 * a bit harsh in RS as getting behind someone ain't that hard. So what we do is
 * if you're dumping ammo and take a rear torso shot, we, on a roll of 7 or
 * less, call this mech_ammunition_dump_explode function. This function finds
 * all the ammo you're dumping and blows up ONE ROUND of one type, randomly. If
 * you're dumping a lot and get hit a few times (like from an LRM) you could get
 * a bunch of little booms which could really ruin your day.
 */

void mech_ammunition_dump_explode(Mech *mech, Mech *attacker, int wHitLoc) {
  DumpingAmmunitionItem ammunition_items[MAX_WEAPONS_PER_MECH];
  int wEventData = -1;
  int wSecIter, wSlotIter;
  int wcAmmoItems = 0;
  int part_type = 0, wPartData = 0;
  int wLoc = 0;
  int weapon_index = 0;
  int wRndIdx = 0;
  int wBlowDamage = 0;

  wEventData = (int)mech_event_data(mech, EVENT_DUMP);
  if (wEventData < 0)
    return;
  if (!wEventData) { /* Global ammo dump */
    for (wSecIter = 7; wSecIter >= 0; wSecIter--)
      for (wSlotIter = mech_section_critical_count(mech, wSecIter) - 1;
           wSlotIter >= 0; wSlotIter--) {
        part_type = mech_critical_part_type(mech, wSecIter, wSlotIter);
        if (equipment_is_ammunition(part_type))
          if (mech_critical_data(mech, wSecIter, wSlotIter)) {
            ammunition_item_add(ammunition_items, &wcAmmoItems,
                                mech_context(mech), part_type, wSecIter,
                                wSlotIter);
          }
      }
  } else if (wEventData < 256) { /* Location specific ammo dump */
    wLoc = wEventData - 1;
    for (wSlotIter = 0; wSlotIter < mech_section_critical_count(mech, wLoc);
         wSlotIter++) {
      part_type = mech_critical_part_type(mech, wLoc, wSlotIter);

      /*     part_type = mech_critical_part_type(mech, wSecIter, wSlotIter); */
      if (equipment_is_ammunition(part_type))
        if (!mech_critical_is_nonfunctional(mech, wLoc, wSlotIter) &&
            mech_critical_data(mech, wLoc, wSlotIter)) {
          ammunition_item_add(ammunition_items, &wcAmmoItems,
                              mech_context(mech), part_type, wLoc, wSlotIter);
        }
    }
  } else if (wEventData < 65536) { /* Weapon specific ammo dump */
    weapon_index = (wEventData / 256) - 1;
    for (wSecIter = 7; wSecIter >= 0; wSecIter--)
      for (wSlotIter = mech_section_critical_count(mech, wSecIter) - 1;
           wSlotIter >= 0; wSlotIter--) {
        part_type = mech_critical_part_type(mech, wSecIter, wSlotIter);
        if (equipment_is_ammunition(part_type) &&
            (ammunition_to_weapon_index(part_type) == weapon_index)) {
          ammunition_item_add(ammunition_items, &wcAmmoItems,
                              mech_context(mech), part_type, wSecIter,
                              wSlotIter);
        }
      }
  } else { /* crit specific dump */
    wSecIter = ((wEventData >> 16) & 0xFF) - 1;
    wSlotIter = ((wEventData >> 24) & 0xFF) - 1;
    part_type = mech_critical_part_type(mech, wSecIter, wSlotIter);
    ammunition_item_add(ammunition_items, &wcAmmoItems, mech_context(mech),
                        part_type, wSecIter, wSlotIter);
  }

  if (wcAmmoItems > 0) {
    wRndIdx = btech_random_range_int(mech_context(mech), 0, wcAmmoItems - 1);
    const DumpingAmmunitionItem *item =
        ammunition_item_at(ammunition_items, (size_t)wRndIdx);
    wBlowDamage = item->damage;
    wSecIter = item->location;
    wSlotIter = item->slot;
    weapon_index = item->weapon_index;
    if (wBlowDamage > 0) {
      mech_los_broadcast(
          mech, "'s rear armor lights up as ammo being dumped ignites!");
      mech_printf(mech, MECHALL,
                  "[fg=red bold]Some of the %s ammo dumping out of your mech "
                  "ignites![reset]",
                  get_parts_long_name(mech_context(mech),
                                      weapon_equipment_index(weapon_index), 0));
      DamageMech(mech, attacker, 0, -1, wHitLoc, 1, 0, wBlowDamage, -1, -1, 0,
                 -1, 0, 1);
      /*
       * Decrement the ammo one round
       */
      wPartData = mech_critical_data(mech, wSecIter, wSlotIter);
      if (wPartData > 0)
        mech_critical_data_set(mech, wSecIter, wSlotIter, wPartData - 1);
      mech_notify(
          mech, MECHALL,
          "[fg=red bold]All ammo dumping operations have stopped![reset]");
      mech_event_cancel(mech, EVENT_DUMP);
    }
  }
}

int weapon_maximum_ammunition_damage(BtechContext *context, int weapon_index) {
  int damage = weapon_catalogue_damage(weapon_index);

  if (weapon_catalogue_is_missile(weapon_index) ||
      weapon_catalogue_is_artillery(weapon_index)) {
    int missile_count =
        btech_context_missile_hit_count(context, weapon_index, 10);
    if (missile_count > 0)
      damage *= missile_count;
  }

  return damage;
}
