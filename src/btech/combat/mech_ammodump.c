/* Implements BattleTech combat mechanics for unit ammodump. */

#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_ammodump_api.h"
#include "mech_api_types.h"
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
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static DumpingAmmunitionItem *ammunition_item_at(DumpingAmmunitionItem *items,
                                                 size_t index) {
  return checked_storage_at(items, MAX_WEAPONS_PER_MECH, sizeof(*items), index);
}

typedef struct AmmunitionItemAddition {
  DumpingAmmunitionItem *items;
  int *count;
  BtechContext *context;
  int part_type;
  CriticalSlotReference slot;
} AmmunitionItemAddition;

static void ammunition_item_add(const AmmunitionItemAddition *addition) {
  DumpingAmmunitionItem *items = addition->items;
  int *count = addition->count;
  if (*count < 0 || *count >= MAX_WEAPONS_PER_MECH)
    abort();
  DumpingAmmunitionItem *item = ammunition_item_at(items, (size_t)*count);
  item->weapon_index = ammunition_to_weapon_index(addition->part_type);
  item->damage =
      weapon_maximum_ammunition_damage(addition->context, item->weapon_index);
  item->location = addition->slot.section;
  item->slot = addition->slot.critical;
  item->part_type = addition->part_type;
  (*count)++;
}

static void mech_dump_event(MuxEvent *ev) {
  Mech *mech = (Mech *)ev->data;
  long arg = (long)ev->secondary.integer;
  int loc;
  int i;
  int l;
  int e = 0;
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
    if (e > 1) {
      mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK,
                          arg);
    } else {
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
          if (mech_critical_data(mech, loc, i))
            mech_ammunition_dump_decrease(mech, loc, i, &e);
    if (e > 1) {
      mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK,
                          arg);
    } else if (e == 1 && mech_is_started(mech)) {
      armor_string_from_index(loc, buf, mech_class(mech),
                              mech_movement_type(mech));
      mech_printf(mech, MECHALL, "All ammunition in %s dumped.", buf);
      mech_los_broadcast(
          mech, "no longer has ammo dumping from hatches on its back.");
    }
    return;
  }
  if (arg < 65536) {
    weapindx = (int)(arg / 256) - 1;
    for (; i >= 0; i--) {
      for (l = mech_section_critical_count(mech, i) - 1; l >= 0; l--) {
        if (equipment_is_ammunition(mech_critical_part_type(mech, i, l)))
          if (ammunition_to_weapon_index(mech_critical_part_type(mech, i, l)) ==
              weapindx)
            if (mech_critical_data(mech, i, l))
              mech_ammunition_dump_decrease(mech, i, l, &e);
      }
    }
    if (e > 1) {
      mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK,
                          arg);
    } else {
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
    armor_string_from_index(l, buf, mech_class(mech), mech_movement_type(mech));
    mech_printf(mech, MECHALL, "Ammunition in %s crit %i dumped!", buf, i + 1);
    mech_los_broadcast(mech,
                       "no longer has ammo dumping from hatches on its back.");
  }
}

static bool mech_is_running_at_desired_speed(const Mech *mech) {
  return mech_desired_speed(mech) >
         ((2.0F * mech_maximum_speed(mech) / 3.0F) + 0.1F);
}

void mech_dump(DbRef player, Mech *mech, char *buffer) {
  int argc;
  char *args[2];
  int weapnum;
  int weapindx;
  int ammo_loc;
  int ammo_crit;
  int loc;
  int i;
  int l;
  int count = 0;
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
  if (!parse_int_checked(args[0], &weapnum)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid weapon number!");
    return;
  }

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
  }
  if (!strcasecmp(args[0], "all")) {
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
  }
  if (!weapnum && strcmp(args[0], "0")) {
    /* Try to find hitloc instead */
    if (mech_event_count(mech, EVENT_DUMP)) {
      mech_notify(mech, MECHALL, "You're already dumping some ammo!");
      return;
    }
    loc = armor_section_from_string(mech_class(mech), mech_movement_type(mech),
                                    args[0]);
    if (loc < 0) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid location or weapon number!");
      return;
    }
    armor_string_from_index(loc, buf, mech_class(mech),
                            mech_movement_type(mech));
    if (args[1]) {
      if (!parse_int_checked(args[1], &i)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Invalid ammunition slot!");
        return;
      }
      i--;
      if (i >= 0 && i < 12) {
        if (equipment_is_ammunition(mech_critical_part_type(mech, loc, i)))
          if (!mech_critical_is_nonfunctional(mech, loc, i))
            if (mech_critical_data(mech, loc, i))
              count++;
        if (!count) {
          mech_printf(mech, MECHALL, "There is no ammunition in %s crit %i!",
                      buf, i + 1);
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
          if (mech_critical_data(mech, loc, i))
            count++;
    if (!count) {
      mech_printf(mech, MECHALL, "There is no ammunition in %s!", buf);
      return;
    }
    type = loc + 1;
    mech_printf(mech, MECHALL, "Starting dumping of ammunition in %s..", buf);
    mech_los_broadcast(mech, "starts dumping ammo from hatches on its back.");
    mech_event_schedule(mech, EVENT_DUMP, mech_dump_event, DUMP_GRAD_TICK,
                        type);
    return;
  }
  weapindx = find_weapon_index(mech, weapnum);
  if (weapnum < 0)
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
        "CHEATER: #%d tried to crash mux with command 'dump %d'!", (int)player,
        weapnum);
  if (mech_event_count(mech, EVENT_DUMP)) {
    mech_notify(mech, MECHALL, "You're already dumping some ammo!");
    return;
  }
  if (weapindx < 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid weapon number!");
    return;
  }
  if (weapon_catalogue_is_energy(weapindx) ||
      weapon_catalogue_is_hand_to_hand(weapindx)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That weapon doesn't use ammunition!");
    return;
  }
  CriticalSlotLookupResult ammunition = ammunition_find(
      &(AmmunitionLookupRequest){.mech = mech, .weapon_index = weapindx});
  if (!ammunition.found) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "You don't have any ammunition for that weapon stored on this mech!");
    return;
  }
  ammo_loc = ammunition.slot.section;
  ammo_crit = ammunition.slot.critical;
  if (mech_critical_data(mech, ammo_loc, ammo_crit) == 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are out of ammunition for that weapon already!");
    return;
  }
  type = 256L * ((long)weapindx + 1L);
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
  int c;
  int index;
  int weapindx;
  int rem;

  /* It _is_ ammo, and contains something */

  index = mech_critical_part_type(mech, loc, pos);
  if (equipment_is_ammunition(index)) {
    if (!mech_critical_is_nonfunctional(mech, loc, pos)) {
      c = mech_critical_data(mech, loc, pos);
      if (c) {
        weapindx = ammunition_to_weapon_index(index);
        const int AMMUNITION_PER_TON =
            weapon_catalogue_ammunition_per_ton(weapindx);
        if (AMMUNITION_PER_TON < DUMP_SPEED) {
          if ((btech_context_event_tick(mech_context(mech)) %
               (DUMP_SPEED / AMMUNITION_PER_TON)))
            return ammunition_dump_result(2, hm);
          /* fine, we remove 1 */
          rem = 1;
        } else {
          rem = min(c, AMMUNITION_PER_TON / DUMP_SPEED);
        }
        mech_ammunition_expenditure_check(&(AmmunitionExpenditureCheck){
            .mech = mech,
            .weapon_index = weapindx,
            .rounds_remaining = rem - 1,
        });
        mech_critical_data_set(mech, loc, pos, c - rem);
        if (c <= rem)
          return ammunition_dump_result(1, hm);
        return ammunition_dump_result(2, hm);
      }
    }
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

void mech_ammunition_dump_explode(Mech *mech, Mech *attacker, int w_hit_loc) {
  DumpingAmmunitionItem ammunition_items[MAX_WEAPONS_PER_MECH];
  int w_event_data = -1;
  int w_sec_iter;
  int w_slot_iter;
  int wc_ammo_items = 0;
  int part_type = 0;
  int w_part_data = 0;
  int w_loc = 0;
  int weapon_index = 0;
  int w_rnd_idx = 0;
  int w_blow_damage = 0;

  w_event_data = (int)mech_event_data(mech, EVENT_DUMP);
  if (w_event_data < 0)
    return;
  if (!w_event_data) { /* Global ammo dump */
    for (w_sec_iter = 7; w_sec_iter >= 0; w_sec_iter--) {
      for (w_slot_iter = mech_section_critical_count(mech, w_sec_iter) - 1;
           w_slot_iter >= 0; w_slot_iter--) {
        part_type = mech_critical_part_type(mech, w_sec_iter, w_slot_iter);
        if (equipment_is_ammunition(part_type)) {
          if (mech_critical_data(mech, w_sec_iter, w_slot_iter)) {
            ammunition_item_add(&(AmmunitionItemAddition){
                .items = ammunition_items,
                .count = &wc_ammo_items,
                .context = mech_context(mech),
                .part_type = part_type,
                .slot = {.section = w_sec_iter, .critical = w_slot_iter}});
          }
        }
      }
    }
  } else if (w_event_data < 256) { /* Location specific ammo dump */
    w_loc = w_event_data - 1;
    for (w_slot_iter = 0;
         w_slot_iter < mech_section_critical_count(mech, w_loc);
         w_slot_iter++) {
      part_type = mech_critical_part_type(mech, w_loc, w_slot_iter);

      /*     part_type = mech_critical_part_type(mech, wSecIter, wSlotIter); */
      if (equipment_is_ammunition(part_type)) {
        if (!mech_critical_is_nonfunctional(mech, w_loc, w_slot_iter) &&
            mech_critical_data(mech, w_loc, w_slot_iter)) {
          ammunition_item_add(&(AmmunitionItemAddition){
              .items = ammunition_items,
              .count = &wc_ammo_items,
              .context = mech_context(mech),
              .part_type = part_type,
              .slot = {.section = w_loc, .critical = w_slot_iter}});
        }
      }
    }
  } else if (w_event_data < 65536) { /* Weapon specific ammo dump */
    weapon_index = (w_event_data / 256) - 1;
    for (w_sec_iter = 7; w_sec_iter >= 0; w_sec_iter--) {
      for (w_slot_iter = mech_section_critical_count(mech, w_sec_iter) - 1;
           w_slot_iter >= 0; w_slot_iter--) {
        part_type = mech_critical_part_type(mech, w_sec_iter, w_slot_iter);
        if (equipment_is_ammunition(part_type) &&
            (ammunition_to_weapon_index(part_type) == weapon_index)) {
          ammunition_item_add(&(AmmunitionItemAddition){
              .items = ammunition_items,
              .count = &wc_ammo_items,
              .context = mech_context(mech),
              .part_type = part_type,
              .slot = {.section = w_sec_iter, .critical = w_slot_iter}});
        }
      }
    }
  } else { /* crit specific dump */
    w_sec_iter = ((w_event_data >> 16) & 0xFF) - 1;
    w_slot_iter = ((w_event_data >> 24) & 0xFF) - 1;
    part_type = mech_critical_part_type(mech, w_sec_iter, w_slot_iter);
    ammunition_item_add(&(AmmunitionItemAddition){
        .items = ammunition_items,
        .count = &wc_ammo_items,
        .context = mech_context(mech),
        .part_type = part_type,
        .slot = {.section = w_sec_iter, .critical = w_slot_iter}});
  }

  if (wc_ammo_items > 0) {
    w_rnd_idx =
        btech_random_range_int(mech_context(mech), 0, wc_ammo_items - 1);
    const DumpingAmmunitionItem *item =
        ammunition_item_at(ammunition_items, (size_t)w_rnd_idx);
    w_blow_damage = item->damage;
    w_sec_iter = item->location;
    w_slot_iter = item->slot;
    weapon_index = item->weapon_index;
    if (w_blow_damage > 0) {
      mech_los_broadcast(
          mech, "'s rear armor lights up as ammo being dumped ignites!");
      mech_printf(mech, MECHALL,
                  "[fg=red bold]Some of the %s ammo dumping out of your mech "
                  "ignites![reset]",
                  get_parts_long_name(mech_context(mech),
                                      weapon_equipment_index(weapon_index), 0));
      mech_damage_apply(
          &(MechDamageRequest){.target = mech,
                               .attacker = attacker,
                               .line_of_sight = false,
                               .attack_pilot = -1,
                               .hit_location = w_hit_loc,
                               .rear = true,
                               .critical = false,
                               .armor_damage = w_blow_damage,
                               .internal_damage = 0,
                               .transfer = MECH_DAMAGE_FORCE_TRANSFER,
                               .cause = -1,
                               .base_to_hit = 0,
                               .weapon_index = -1,
                               .ammunition_mode = 0,
                               .ignore_swarmers = true});
      /*
       * Decrement the ammo one round
       */
      w_part_data = mech_critical_data(mech, w_sec_iter, w_slot_iter);
      if (w_part_data > 0)
        mech_critical_data_set(mech, w_sec_iter, w_slot_iter, w_part_data - 1);
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
    int missile_count = btech_context_missile_hit_count(&(MissileHitLookup){
        .context = context, .weapon = weapon_index, .roll = 10});
    if (missile_count > 0)
      damage *= missile_count;
  }

  return damage;
}
