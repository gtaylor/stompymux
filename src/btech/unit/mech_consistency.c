/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *       All rights reserved
 */
#include "btech/context.h"
#include "btech_channel.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "mech_classification_api.h"
#include "mech_consistency_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mycool.h"
#include "registry_api.h"
#include <math.h>
#include <stdio.h>
#include <strings.h>
static const char mech_loc_table[][2] = {{CTORSO, 1}, {LTORSO, 2}, {RTORSO, 2},
                                         {LARM, 3},   {RARM, 3},   {LLEG, 4},
                                         {RLEG, 4},   {-1, 0}};
static const char quad_loc_table[][2] = {{CTORSO, 1}, {LTORSO, 2}, {RTORSO, 2},
                                         {LARM, 4},   {RARM, 4},   {LLEG, 4},
                                         {RLEG, 4},   {-1, 0}};
static const char int_data[][5] = {
    {10, 4, 3, 1, 2},      {15, 5, 4, 2, 3},     {20, 6, 5, 3, 4},
    {25, 8, 6, 4, 6},      {30, 10, 7, 5, 7},    {35, 11, 8, 6, 8},
    {40, 12, 10, 6, 10},   {45, 14, 11, 7, 11},  {50, 16, 12, 8, 12},
    {55, 18, 13, 9, 13},   {60, 20, 14, 10, 14}, {65, 21, 15, 10, 15},
    {70, 22, 15, 11, 15},  {75, 23, 16, 12, 16}, {80, 25, 17, 13, 17},
    {85, 27, 18, 14, 18},  {90, 29, 19, 15, 19}, {95, 30, 20, 16, 20},
    {100, 31, 21, 17, 21}, {-1, 0, 0, 0, 0}};
static const short engine_data[][2] = {{0, 0},
                                       {10, 1},
                                       {15, 1},
                                       {20, 1},
                                       {25, 1},
                                       {30, 2},
                                       {35, 2},
                                       {40, 2},
                                       {45, 2},
                                       {50, 3},
                                       {55, 3},
                                       {60, 3},
                                       {65, 4},
                                       {70, 4},
                                       {75, 4},
                                       {80, 5},
                                       {85, 5},
                                       {90, 6},
                                       {95, 6},
                                       {100, 6},
                                       {105, 7},
                                       {110, 7},
                                       {115, 8},
                                       {120, 8},
                                       {125, 8},
                                       {130, 9},
                                       {135, 9},
                                       {140, 10},
                                       {145, 10},
                                       {150, 11},
                                       {155, 11},
                                       {160, 12},
                                       {165, 12},
                                       {170, 12},
                                       {175, 14},
                                       {180, 14},
                                       {185, 15},
                                       {190, 15},
                                       {195, 16},
                                       {200, 17},
                                       {205, 17},
                                       {210, 18},
                                       {215, 19},
                                       {220, 20},
                                       {225, 20},
                                       {230, 21},
                                       {235, 22},
                                       {240, 23},
                                       {245, 24},
                                       {250, 25},
                                       {255, 26},
                                       {260, 27},
                                       {265, 28},
                                       {270, 29},
                                       {275, 31},
                                       {280, 32},
                                       {285, 33},
                                       {290, 35},
                                       {295, 36},
                                       {300, 38},
                                       {305, 39},
                                       {310, 41},
                                       {315, 43},
                                       {320, 45},
                                       {325, 47},
                                       {330, 49},
                                       {335, 51},
                                       {340, 54},
                                       {345, 57},
                                       {350, 59},
                                       {355, 63},
                                       {360, 66},
                                       {365, 69},
                                       {370, 73},
                                       {375, 77},
                                       {380, 82},
                                       {385, 87},
                                       {390, 92},
                                       {395, 98},
                                       {400, 105},
                                       {405, 113},
                                       {410, 122},
                                       {415, 133},
                                       {420, 145},
                                       {425, 159},
                                       {430, 87 * 2 + 1},
                                       {435, 97 * 2},
                                       {440, 107 * 2 + 1},
                                       {445, 119 * 2 + 1},
                                       {450, 133 * 2 + 1},
                                       {455, 150 * 2},
                                       {460, 168 * 2 + 1},
                                       {465, 190 * 2},
                                       {470, 214 * 2 + 1},
                                       {475, 243 * 2},
                                       {480, 275 * 2 + 1},
                                       {485, 313 * 2},
                                       {490, 356 * 2},
                                       {495, 405 * 2 + 1},
                                       {500, 462 * 2 + 1},
                                       {-1, 0}};
int susp_factor(Mech *mech) {
  int t = ((mech)->ud.tons);
  if (((mech)->ud.move) == MOVE_TRACK)
    return 0;
  if (((mech)->ud.move) == MOVE_WHEEL)
    return 20;
  if (((mech)->ud.move) == MOVE_FOIL) {
    if (t <= 10)
      return 60;
    if (t <= 20)
      return 105;
    if (t <= 30)
      return 150;
    if (t <= 40)
      return 195;
    if (t <= 50)
      return 255;
    if (t <= 60)
      return 300;
    if (t <= 70)
      return 345;
    if (t <= 80)
      return 390;
    if (t <= 90)
      return 435;
    return 480;
  }
  if (((mech)->ud.move) == MOVE_HOVER) {
    if (t <= 10)
      return 40;
    if (t <= 20)
      return 85;
    if (t <= 30)
      return 130;
    if (t <= 40)
      return 175;
    return 235;
  }
  if (((mech)->ud.move) == MOVE_HULL || ((mech)->ud.move) == MOVE_SUB)
    return 30;
  if (((mech)->ud.move) == MOVE_VTOL) {
    if (t <= 10)
      return 50;
    if (t <= 20)
      return 95;
    return 140;
  }
  return 0;
}
int crit_weight(Mech *mech, int t) {
  int cl;
  if (equipment_is_weapon(t))
    return MechWeapons[weapon_from_equipment_index(t)].weight * 1024 / 100 /
           GetWeaponCrits(mech, weapon_from_equipment_index(t));
  if (equipment_is_ammunition(t))
    return 512;
  if (!(equipment_is_special(t)))
    return 1024;
  t = special_from_equipment_index(t);
  cl = ((mech)->rd.specials) & CLAN_TECH;
  switch (t) {
  case HEAT_SINK:
    return 1024 / mech_heat_sink_critical_size(mech);
  case TARGETING_COMPUTER:
  case AXE:
  case CLAW:
  case MACE:
    return 1024;
  case DUAL_SAW:
    return 1024;
  case ARTEMIS_IV:
  case MASC:
  case C3_SLAVE:
  case TAG:
  case LAMEQUIP:
    return 1024;
  case C3I:
    return 1280 * (((mech)->ud.type) == CLASS_MECH ? 1 : 2);
  case ANGELECM:
    return 1024 * (((mech)->ud.type) == CLASS_MECH ? 1 : 2);
  case BLOODHOUND_PROBE:
    /* Bloodhound is 2 tons for 3 crits */
    return 1024 * (((mech)->ud.type) == CLASS_MECH ? 2 : 6) / 3;
  case C3_MASTER:
    return 1024 * (((mech)->ud.type) == CLASS_MECH ? 1 : 5);
  case SWORD:
    /* A Sword weighs 1/20th of the 'mech tonnage, rounded up to the half
       ton, and is 1/15th (rounded up to int) number of crits. */
    return clamp_float_to_int((ceilf((float)mech->ud.tons / 10.0F) * 512.0F) /
                              ceilf((float)mech->ud.tons / 15.0F));
  case BEAGLE_PROBE:
    return 1024 * 3 / (((mech)->ud.type) == CLASS_MECH ? 4 : 2);
  case ECM:
    /* IS ECM is 1.5 tons for 2 crits, Clan ECM 1 ton for 1 crit. */
    return 1024 * (cl ? 4 : ((mech)->ud.type) == CLASS_MECH ? 3 : 6) / 4;
  case CASE:
    return 512;
  case LIGHT_BAP:
    return 512;
  case JUMP_JET:
    if (((mech)->ud.tons) <= 55)
      return 512;
    if (((mech)->ud.tons) <= 85)
      return 1024;
    return 2048;
  default:
    return 0;
  }
}
int engine_weight(Mech *mech) {
  int s = mech_engine_rating(mech);
  int i;
  if (((mech)->ud.type) != CLASS_MECH)
    s -= susp_factor(mech);
  for (i = 0; engine_data[i][0] >= 0; i++)
    if (s == engine_data[i][0]) {
      int weight = engine_data[i][1] * 512;
      if (((mech)->rd.specials) & ICE_TECH)
        return weight * 2;
      if (((mech)->ud.type) == CLASS_VEH_GROUND ||
          ((mech)->ud.type) == CLASS_VTOL ||
          ((mech)->ud.type) == CLASS_VEH_NAVAL)
        /* Vehicles need extra shielding in case of a fusion engine */
        weight = round_to_halfton(weight + weight / 2);
      if (((mech)->rd.specials) & XL_TECH)
        return round_to_halfton(weight / 2);
      if (((mech)->rd.specials) & XXL_TECH)
        return round_to_halfton(weight / 3);
      if (((mech)->rd.specials) & LE_TECH)
        return round_to_halfton(weight * 3 / 4);
      if (((mech)->rd.specials) & CE_TECH)
        return round_to_halfton(weight + weight / 2);
      return weight;
    }

  /* Hack ensues! Most hovers are 1/5th engine weight. Doesn't always register
   * correctly. */
  if (((mech)->ud.move) != MOVE_HOVER) {
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Error in #%ld (%s) : No engine found!", mech->mynum,
                game_object_name(mech->xcode.context->database, mech->mynum)));
  }

  return 0;
}

static void calc_ints(Mech *mech, int *n, int *tot) {
  int i;

  *n = 0;
  *tot = 0;
  for (i = 0; i < NUM_SECTIONS; i++) {
    *n += mech_section_internal(mech, i);
    *tot += mech_section_original_internal(mech, i);
  }
  *tot = MAX(1, *tot);
}

static int ammo_weight(Mech *mech) {
  int i, j, t, w = 0;

  for (i = 0; i < NUM_SECTIONS; i++)
    if (!mech_section_is_destroyed(mech, i))
      for (j = 0; j < CritsInLoc(mech, i); j++)
        if (equipment_is_ammunition((t = mech_critical_part_type(mech, i, j))))
          w += mech_critical_data(mech, i, j) * 1024 /
               MechWeapons[ammunition_to_weapon_index(
                               mech_critical_part_type(mech, i, j))]
                   .ammoperton;
  return w;
}

static int section_weight_armor(const Mech *mech, int location,
                                int interactive) {
  return interactive >= 0 ? mech_section_original_armor(mech, location)
                          : mech_section_armor(mech, location);
}

static int section_weight_rear_armor(const Mech *mech, int location,
                                     int interactive) {
  return interactive >= 0 ? mech_section_original_rear_armor(mech, location)
                          : mech_section_rear_armor(mech, location);
}

static int weight_heat_sink_count(const Mech *mech, int interactive) {
  return interactive >= 0 ? ((mech)->rd.onumsinks) : ((mech)->ud.numsinks);
}

static int gyro_weight(float gyro_weight_in_tons, float multiplier) {
  return clamp_float_to_int(ceilf(gyro_weight_in_tons) * 1024.0F * multiplier);
}

static float engine_rating_in_tons(int rating) {
  return (float)rating / 100.0F;
}

static void weight_entry_add(CoolMenu **menu, int interactive, int *total,
                             const char *text, int weight) {
  if (!weight)
    return;
  if (interactive > 0) {
    cool_menu_add(menu, text);
    cool_menu_add(menu,
                  tprintf("      %6.2f", (double)((float)weight / 1024.0F)));
  }
  *total += weight;
}

static void weight_counted_entry_add(CoolMenu **menu, int interactive,
                                     int *total, const char *text, int count,
                                     int weight) {
  if (!weight)
    return;
  if (interactive > 0) {
    cool_menu_add(menu, text);
    cool_menu_add(
        menu, tprintf("%5d %6.2f", count, (double)((float)weight / 1024.0F)));
  }
  *total += weight;
}
int mech_weight_sub_mech(DbRef player, Mech *mech, int interactive) {
  int pile[NUM_ITEMS_M];
  int i, j, w, cl, id;
  int armor = 0, armor_o;
  int total = 0;
  CoolMenu *c = NULL;
  int shs_size;
  int hs_eff;
  char buf[MBUF_SIZE];
  int ints_c, ints_tot;
  float gyro_calc = -1;
  int t, temp;

  bzero(pile, sizeof(pile));
  if (interactive > 0) {
    cool_menu_add_line(&c);
    cool_menu_add_centered(
        &c, tprintf("Weight totals for %s", mech_display_id(mech).text));
    cool_menu_add_line(&c);
  }
  calc_ints(mech, &ints_c, &ints_tot);
  for (i = 0; i < NUM_SECTIONS; i++) {
    if (!mech_section_original_internal(mech, i))
      continue;
    armor += section_weight_armor(mech, i, interactive);
    armor += section_weight_rear_armor(mech, i, interactive);
    if (interactive >= 0 || !mech_section_is_destroyed(mech, i))
      for (j = 0; j < NUM_CRITICALS; j++) {
        t = mech_critical_part_type(mech, i, j);
        if (interactive >= 0 || !equipment_is_ammunition(t)) {
          // Handle Split Crits
          if (special_from_equipment_index(t) == SPLIT_CRIT_RIGHT ||
              special_from_equipment_index(t) == SPLIT_CRIT_LEFT) {
            temp = ReverseSplitCritLoc(mech, i, j);
            if (temp >= 0) {
              t = mech_critical_part_type(mech, temp,
                                          mech_critical_data(mech, i, j));
              pile[t] += clamp_float_to_int(mech_ammunition_slot_multiplier(
                  mech, temp, mech_critical_data(mech, i, j)));
            }
          } else
            pile[t] +=
                clamp_float_to_int(mech_ammunition_slot_multiplier(mech, i, j));
        }
      }
  }
  shs_size = mech_heat_sink_critical_size(mech);
  hs_eff = mech_has_double_heat_sinks(mech) ? 2 : 1;
  cl = ((mech)->rd.specials) & CLAN_TECH;
  snprintf(buf, sizeof(buf), "%-12s(%d rating)",
           ((mech)->rd.specials) & XL_TECH    ? "Engine (XL)"
           : ((mech)->rd.specials) & XXL_TECH ? "Engine (XXL)"
           : ((mech)->rd.specials) & CE_TECH  ? "Engine (Compact)"
           : ((mech)->rd.specials) & LE_TECH  ? "Engine (Light)"
                                              : "Engine",
           mech_engine_rating(mech));
  if (interactive >= 0 || !mech_section_is_destroyed(mech, CTORSO))
    weight_entry_add(&c, interactive, &total, buf, engine_weight(mech));
  if (interactive >= 0 || !mech_section_is_destroyed(mech, HEAD)) {
    if (((mech)->rd.specials2) & SMALLCOCKPIT_TECH) {
      weight_entry_add(&c, interactive, &total, "Cockpit (Small)", 2 * 1024);
    } else {
      weight_entry_add(&c, interactive, &total, "Cockpit", 3 * 1024);
    }
  }
  if (interactive >= 0 || !mech_section_is_destroyed(mech, CTORSO))
    /* Store the base-line gyro weight */
    gyro_calc = engine_rating_in_tons(mech_engine_rating(mech));

  /* Figure out what kind of gyro we have and adjust weight accordingly */
  if (((mech)->rd.specials2) & XLGYRO_TECH) {
    /* XL Gyro is 1/2 normal gyro weight. */
    weight_entry_add(&c, interactive, &total, "Gyro (XL)",
                     gyro_weight(gyro_calc, 0.5F));
  } else if (((mech)->rd.specials2) & HDGYRO_TECH) {
    /* Hardened Gyro is 2x normal gyro weight. */
    weight_entry_add(&c, interactive, &total, "Gyro (Hardened)",
                     gyro_weight(gyro_calc, 2.0F));
  } else if (((mech)->rd.specials2) & CGYRO_TECH) {
    /* Compact Gyro is 1.5x normal gyro weight. */
    weight_entry_add(&c, interactive, &total, "Gyro (Compact)",
                     gyro_weight(gyro_calc, 1.5F));
  } else {
    /* Standard Gyro. */
    weight_entry_add(&c, interactive, &total, "Gyro",
                     gyro_weight(gyro_calc, 1.0F));
  }

  weight_entry_add(
      &c, interactive, &total,
      ((mech)->rd.specials) & REINFI_TECH  ? "Internals (Reinforced)"
      : ((mech)->rd.specials) & COMPI_TECH ? "Internals (Composite)"
      : ((mech)->rd.specials) & ES_TECH    ? "Internals (ES)"
                                           : "Internals",
      round_to_halfton(((mech)->ud.tons) * 1024 *
                       (interactive >= 0 ? ints_tot : ints_c) / 5 / ints_tot /
                       (((mech)->rd.specials) & REINFI_TECH ? 1
                        : (((mech)->rd.specials) & (ES_TECH | COMPI_TECH))
                            ? 4
                            : 2)));
  armor_o = armor;
  if (((mech)->rd.specials) & FF_TECH)
    armor = armor * 50 / (cl ? 60 : 56);
  else if (((mech)->rd.specials2) & HVY_FF_ARMOR_TECH)
    armor = armor * 50 / 62;
  else if (((mech)->rd.specials2) & LT_FF_ARMOR_TECH)
    armor = armor * 50 / 53;

  weight_counted_entry_add(
      &c, interactive, &total,
      ((mech)->rd.specials2) & STEALTH_ARMOR_TECH  ? "Armor (Stealth)"
      : ((mech)->rd.specials2) & HVY_FF_ARMOR_TECH ? "Armor (Hvy FF)"
      : ((mech)->rd.specials2) & LT_FF_ARMOR_TECH  ? "Armor (Lt FF)"
      : ((mech)->rd.specials) & HARDA_TECH         ? "Armor (Hardened)"
      : ((mech)->rd.specials) & FF_TECH            ? "Armor (FF)"
                                                   : "Armor",
      armor_o,
      round_to_halfton(armor * 1024 /
                       (((mech)->rd.specials) & HARDA_TECH ? 8 : 16)));

  // ceil(armor /

  //					(8. * (((mech)->rd.specials) &
  // HARDA_TECH ?
  // 2 : 1))) * 512);

  if (weight_heat_sink_count(mech, interactive)) {
    pile[special_equipment_index(HEAT_SINK)] =
        MAX(0, weight_heat_sink_count(mech, interactive) * shs_size / hs_eff -
                   (((mech)->rd.specials) & ICE_TECH ? 0 : 10) * shs_size);
  } else if (interactive > 0)
    cool_menu_add_centered(
        &c,
        tprintf("WARNING: HS count may be off, due to certain odd things."));
  for (i = 1; i < NUM_ITEMS_M; i++)
    if (pile[i]) {
      if (equipment_is_weapon(i)) {
        id = weapon_from_equipment_index(i);
        weight_counted_entry_add(&c, interactive, &total, MechWeapons[id].name,
                                 pile[i] / GetWeaponCrits(mech, id),
                                 crit_weight(mech, i) * pile[i]);
      } else {
        if ((w = crit_weight(mech, i)))
          weight_counted_entry_add(
              &c, interactive, &total,
              get_parts_long_name(mech->xcode.context, i, 0), pile[i],
              w * pile[i]);
      }
    }
  if (((mech)->ud.cargospace))
    weight_entry_add(
        &c, interactive, &total,
        tprintf("CargoSpace (%.2ft)",
                (double)((float)mech->ud.cargospace / 100.0F)),
        clamp_float_to_int(((float)mech->ud.cargospace /
                            (((mech)->rd.specials2) & CARRIER_TECH ? 1000.0F
                             : ((mech)->rd.specials) & CARGO_TECH  ? 100.0F
                                                                   : 500.0F)) *
                           1024.0F));

  if (interactive > 0) {
    cool_menu_add_line(&c);
    cool_menu_add_text(
        &c, tprintf("[fg=green]Total: %s%.1f tons (offset: %.1f)[reset]",
                    (total / 1024) > ((mech)->ud.tons) ? "[fg=red bold]" : "",
                    (double)((float)total / 1024.0F),
                    (double)((float)mech->ud.tons - (float)total / 1024.0F)));
    cool_menu_add_line(&c);
    ShowCoolMenu(btech_context_evaluation(mech->xcode.context), player, c);
  }
  KillCoolMenu(c);
  if (interactive < 0)
    total += ammo_weight(mech);
  return MAX(1, total);
}

static int tank_in_pieces(Mech *mech) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++)
    if (mech_section_internal(mech, i))
      return 0;
  return 1;
}

int mech_weight_sub_veh(DbRef player, Mech *mech, int interactive) {
  int pile[NUM_ITEMS_M];
  int i, j, w, cl, id, t;
  int armor = 0, armor_o;
  int total = 0;
  CoolMenu *c = NULL;
  int shs_size;
  int hs_eff;
  char buf[MBUF_SIZE];
  int es;
  int turr_stuff = 0;
  int ints_c, ints_tot;

  bzero(pile, sizeof(pile));
  calc_ints(mech, &ints_c, &ints_tot);
  if (interactive > 0) {
    cool_menu_add_line(&c);
    cool_menu_add_centered(
        &c, tprintf("Weight totals for %s", mech_display_id(mech).text));
    cool_menu_add_line(&c);
  }
  for (i = 0; i < NUM_SECTIONS; i++) {
    if (!(mech_section_original_internal(mech, i)))
      continue;
    armor += section_weight_armor(mech, i, interactive);
    armor += section_weight_rear_armor(mech, i, interactive);
    for (j = 0; j < CritsInLoc(mech, i); j++) {
      if (!(t = mech_critical_part_type(mech, i, j)))
        continue;
      if (interactive >= 0 || !mech_section_is_destroyed(mech, i)) {
        if (interactive >= 0 || !equipment_is_ammunition(t))
          pile[t] +=
              clamp_float_to_int(mech_ammunition_slot_multiplier(mech, i, j));
        if (i == TURRET && (((mech)->ud.type) == CLASS_VEH_GROUND ||
                            ((mech)->ud.type) == CLASS_VEH_NAVAL))
          if (equipment_is_weapon(t))
            turr_stuff += crit_weight(mech, t);
      }
    }
  }
  shs_size = mech_heat_sink_critical_size(mech);
  hs_eff = mech_has_double_heat_sinks(mech) ? 2 : 1;
  cl = ((mech)->rd.specials) & CLAN_TECH;
  es = susp_factor(mech);
  if (es)
    snprintf(buf, sizeof(buf), "%-12s(%d->%d eff/wt rat)",
             ((mech)->rd.specials) & LE_TECH    ? "Engine (Light)"
             : ((mech)->rd.specials) & CE_TECH  ? "Engine (Compact)"
             : ((mech)->rd.specials) & XXL_TECH ? "Engine (XXL)"
             : ((mech)->rd.specials) & XL_TECH  ? "Engine (XL)"
             : ((mech)->rd.specials) & ICE_TECH ? "Engine (ICE)"
                                                : "Engine",
             mech_engine_rating(mech),
             mech_engine_rating(mech) - susp_factor(mech));
  else
    snprintf(buf, sizeof(buf), "%-12s(%d rating)",
             ((mech)->rd.specials) & LE_TECH    ? "Engine (Light)"
             : ((mech)->rd.specials) & CE_TECH  ? "Engine (Compact)"
             : ((mech)->rd.specials) & XXL_TECH ? "Engine (XXL)"
             : ((mech)->rd.specials) & XL_TECH  ? "Engine (XL)"
             : ((mech)->rd.specials) & ICE_TECH ? "Engine (ICE)"
                                                : "Engine",
             mech_engine_rating(mech));
  if (!tank_in_pieces(mech)) {
    weight_entry_add(&c, interactive, &total, buf, (es = engine_weight(mech)));
    if (((mech)->ud.move) == MOVE_HOVER && es < (((mech)->ud.tons) * 1024 / 5))
      weight_entry_add(&c, interactive, &total,
                       "Engine size fix (-> 1/5 hover wt.)",
                       ((mech)->ud.tons) * 1024 / 5 - es);
    weight_entry_add(&c, interactive, &total, "Cockpit",
                     round_to_quarterton(((mech)->ud.tons) * 1024 / 20));
    if (((mech)->ud.type) == CLASS_VTOL || ((mech)->ud.move) == MOVE_HOVER ||
        ((mech)->ud.move) == MOVE_HULL || ((mech)->ud.move) == MOVE_SUB)
      weight_entry_add(&c, interactive, &total, "SpecialComponents",
                       round_to_halfton(((mech)->ud.tons) * 1024 / 10));
  }
  if (interactive >= 0 || !mech_section_is_destroyed(mech, TURRET))
    if (turr_stuff)
      weight_entry_add(&c, interactive, &total, "Turret",
                       round_to_quarterton((turr_stuff / 10)));
  weight_entry_add(
      &c, interactive, &total,
      ((mech)->rd.specials) & REINFI_TECH  ? "Internals (Reinforced)"
      : ((mech)->rd.specials) & COMPI_TECH ? "Internals (Composite)"
      : ((mech)->rd.specials) & ES_TECH    ? "Internals (ES)"
                                           : "Internals",
      round_to_halfton(((mech)->ud.tons) * 1024 *
                       (interactive >= 0 ? ints_tot : ints_c) / 5 / ints_tot /
                       (((mech)->rd.specials) & REINFI_TECH ? 1
                        : (((mech)->rd.specials) & (ES_TECH | COMPI_TECH))
                            ? 4
                            : 2)));
  armor_o = armor;

  if (((mech)->rd.specials) & FF_TECH)
    armor = armor * 50 / (cl ? 60 : 56);
  else if (((mech)->rd.specials2) & HVY_FF_ARMOR_TECH)
    armor = armor * 50 / 62;
  else if (((mech)->rd.specials2) & LT_FF_ARMOR_TECH)
    armor = armor * 50 / 53;
  else if (((mech)->rd.specials) & HARDA_TECH)
    armor *= 2;

  weight_counted_entry_add(
      &c, interactive, &total,
      ((mech)->rd.specials2) & STEALTH_ARMOR_TECH  ? "Armor (Stealth)"
      : ((mech)->rd.specials2) & HVY_FF_ARMOR_TECH ? "Armor (Hvy FF)"
      : ((mech)->rd.specials2) & LT_FF_ARMOR_TECH  ? "Armor (Lt FF)"
      : ((mech)->rd.specials) & HARDA_TECH         ? "Armor (Hardened)"
      : ((mech)->rd.specials) & FF_TECH            ? "Armor (FF)"
                                                   : "Armor",
      armor_o, round_to_halfton(armor * 1024 / 16));

  pile[special_equipment_index(HEAT_SINK)] =
      MAX(0, ((mech)->ud.numsinks) * shs_size / hs_eff -
                 (((mech)->rd.specials) & ICE_TECH ? 0 : 10) * shs_size);
  for (i = 1; i < NUM_ITEMS_M; i++)
    if (pile[i]) {
      if (equipment_is_weapon(i)) {
        id = weapon_from_equipment_index(i);
        weight_counted_entry_add(&c, interactive, &total, MechWeapons[id].name,
                                 pile[i] / GetWeaponCrits(mech, id),
                                 crit_weight(mech, i) * pile[i]);
      } else if ((w = crit_weight(mech, i)))
        weight_counted_entry_add(&c, interactive, &total,
                                 get_parts_long_name(mech->xcode.context, i, 0),
                                 pile[i], w * pile[i]);
    }
  if (((mech)->ud.cargospace))
    weight_entry_add(
        &c, interactive, &total,
        tprintf("CargoSpace (%.2ft)",
                (double)((float)mech->ud.cargospace / 100.0F)),
        clamp_float_to_int(((float)mech->ud.cargospace /
                            (((mech)->rd.specials2) & CARRIER_TECH ? 1000.0F
                             : ((mech)->rd.specials) & CARGO_TECH  ? 100.0F
                                                                   : 500.0F)) *
                           1024.0F));

  if (interactive > 0) {
    cool_menu_add_line(&c);
    cool_menu_add_text(
        &c, tprintf("[fg=green]Total: %s%.1f tons (offset: %.1f)[reset]",
                    (total / 1024) > ((mech)->ud.tons) ? "[fg=red bold]" : "",
                    (double)((float)total / 1024.0F),
                    (double)((float)mech->ud.tons - (float)total / 1024.0F)));
    cool_menu_add_line(&c);
    ShowCoolMenu(btech_context_evaluation(mech->xcode.context), player, c);
  }
  KillCoolMenu(c);
  if (interactive < 0)
    total += ammo_weight(mech);
  return MAX(1, total);
}

/* Returns: 1024 * MechWeight(in tons) */
int mech_weight_sub(DbRef player, Mech *mech, int interactive) {
  if (((mech)->ud.type) == CLASS_MECH)
    return mech_weight_sub_mech(player, mech, interactive);
  if (((mech)->ud.type) == CLASS_VEH_GROUND ||
      ((mech)->ud.type) == CLASS_VTOL || ((mech)->ud.type) == CLASS_VEH_NAVAL)
    return mech_weight_sub_veh(player, mech, interactive);
  if (interactive > 0)
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid vehicle type!");
  return 1;
}

void mech_weight(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  mech_weight_sub(player, mech, 1);
}

static int internal_location_table(const Mech *mech, int row, int column) {
  return mech_is_quad(mech) ? quad_loc_table[row][column]
                            : mech_loc_table[row][column];
}

static int real_int(Mech *mech, int loc, int ti) {
  int i;

  if (loc == HEAD)
    return 3;
  for (i = 0; internal_location_table(mech, i, 0) >= 0; i++)
    if (loc == internal_location_table(mech, i, 0))
      break;
  if (internal_location_table(mech, i, 0) < 0)
    return 0;
  return int_data[ti][internal_location_table(mech, i, 1)];
}

static int vehicle_internal_structure(const Mech *mech) {
  return MAX((((mech)->ud.tons) + 5) / 10, 1);
}

void vehicle_int_check(Mech *mech, int noisy) {
  int i, j;

  j = vehicle_internal_structure(mech);
  for (i = 0; i < NUM_SECTIONS; i++)
    if (mech_section_original_internal(mech, i) &&
        mech_section_original_internal(mech, i) != j) {
      if (noisy)
        btech_channel_send(
            mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Template %s / mech #%ld: Invalid internals in loc %d "
                    "(should be %d, are %d)",
                    ((mech)->ud.mech_type), mech->mynum, i, j,
                    mech_section_original_internal(mech, i)));
      mech_section_original_internal_set(mech, i, j);
      mech_section_internal_set(mech, i, j);
    }
}

void mech_int_check(Mech *mech, int noisy) {
  int i, j, k;

  if (((mech)->ud.type) != CLASS_MECH) {
    if (((mech)->ud.type) == CLASS_VEH_GROUND ||
        ((mech)->ud.type) == CLASS_VTOL || ((mech)->ud.type) == CLASS_VEH_NAVAL)
      vehicle_int_check(mech, noisy);
    return;
  }
  for (i = 0; int_data[i][0] >= 0; i++)
    if (((mech)->ud.tons) == int_data[i][0])
      break;
  if (int_data[i][0] < 0) {
    if (noisy)
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("VERY odd tonnage for #%ld: %d.", mech->mynum,
                                 ((mech)->ud.tons)));
    return;
  }
  k = i;
  for (i = 0; i < NUM_SECTIONS; i++) {
    if (mech_section_original_internal(mech, i) != (j = real_int(mech, i, k))) {
      if (noisy)
        btech_channel_send(
            mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Template %s / mech #%ld: Invalid internals in loc %d "
                    "(should be %d, are %d)",
                    ((mech)->ud.mech_type), mech->mynum, i, j,
                    mech_section_original_internal(mech, i)));
      mech_section_original_internal_set(mech, i, j);
      mech_section_internal_set(mech, i, j);
    }
  }
}
