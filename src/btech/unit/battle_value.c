#include "btech_channel.h"
#include "checked_conversion.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_move_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mech_utils_internal.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"
#include "weapon_settings.h"

int mech_weapon_recycle_time(const Mech *mech, int weapon_index) {
  return btech_weapon_settings_recycle_time(
      &mech->xcode.context->weapon_settings, weapon_index);
}

int mech_weapon_battle_value(const Mech *mech, int weapon_index) {
  return btech_weapon_settings_battle_value(
      &mech->xcode.context->weapon_settings, weapon_index);
}
#include "weapon_catalogue_api.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

static int battle_value_table_get(const int values[64], int index) {
  if (index < 0)
    abort();
  return *(const int *)checked_storage_at_const(values, 64, sizeof(*values),
                                                (size_t)index);
}

static int *battle_value_table_slot(int values[64], int index) {
  if (index < 0)
    abort();
  return checked_storage_at(values, 64, sizeof(*values), (size_t)index);
}

static unsigned char battle_value_weapon_get(const unsigned char *values,
                                             int index) {
  if (index < 0)
    abort();
  return *(const unsigned char *)checked_storage_at_const(
      values, MAX_WEAPS_SECTION, sizeof(*values), (size_t)index);
}

int find_average_gunnery(Mech *mech) {
  /* NULLTODO : Get the multiple skills for gunnery and such ported or working
   * here so this is usefull again. */
  return find_pilot_gunnery(mech, 0);
}

static const float
    SKILL_MULTIPLIERS[BTECH_BV_SKILL_LIMIT][BTECH_BV_SKILL_LIMIT] = {
        {2.05F, 2.00F, 1.95F, 1.90F, 1.85F, 1.80F, 1.75F, 1.70F},
        {1.85F, 1.80F, 1.75F, 1.70F, 1.65F, 1.60F, 1.55F, 1.50F},
        {1.65F, 1.60F, 1.55F, 1.50F, 1.45F, 1.40F, 1.35F, 1.30F},
        {1.45F, 1.40F, 1.35F, 1.30F, 1.25F, 1.20F, 1.15F, 1.10F},
        {1.25F, 1.20F, 1.15F, 1.10F, 1.05F, 1.00F, 0.95F, 0.90F},
        {1.15F, 1.10F, 1.05F, 1.00F, 0.95F, 0.90F, 0.85F, 0.80F},
        {1.05F, 1.00F, 0.95F, 0.90F, 0.85F, 0.80F, 0.75F, 0.70F},
        {0.95F, 0.90F, 0.85F, 0.80F, 0.75F, 0.70F, 0.65F, 0.60F}};

float battle_value_skill_multiplier(int gunnery, int piloting) {
  const int GUN_INDEX = battle_value_skill_index(gunnery);
  const int PILOT_INDEX = battle_value_skill_index(piloting);
  const float (*row)[BTECH_BV_SKILL_LIMIT] =
      checked_storage_at_const(SKILL_MULTIPLIERS, BTECH_BV_SKILL_LIMIT,
                               sizeof(*SKILL_MULTIPLIERS), (size_t)GUN_INDEX);
  return *(const float *)checked_storage_at_const(
      *row, BTECH_BV_SKILL_LIMIT, sizeof(**row), (size_t)PILOT_INDEX);
}

void calc_add_off_bv(const Mech *mech, float *offbv, const char *desc,
                     float value) {
  *offbv += value;
  if (mech->xcode.context->configuration->btech_cost_debug)
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                       "AddOffBV %25s %8.2f", desc, (double)value);
}

void calc_add_def_bv(const Mech *mech, float *defbv, const char *desc,
                     float value) {
  *defbv += value;
  if (mech->xcode.context->configuration->btech_cost_debug)
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                       "AddDefBV %25s %8.2f", desc, (double)value);
}

void calc_sub_def_bv(const Mech *mech, float *defbv, const char *desc,
                     float value) {
  *defbv -= value;
  if (mech->xcode.context->configuration->btech_cost_debug)
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                       "SubDefBV %25s-%8.2f", desc, (double)value);
}

/* Calculate Defensive BV 2.0 per Total Warfare Rules */
float calculate_defensive_bv(Mech *mech) {
  float defbv = 0.0F;
  float engine_mod = 0.0F;
  int i;
  int ii;
  int part;
  int weapindx;
  int ecm_count = 0;
  int bap_count = 0;
  int jump_mp = 0;
  int move_mod = 0;
  int run_mp = 0;
  float def_factor = 0.0F;
  char buff[50];

  /* ARMOR
   * Total Armor Factor (Points) * 2.5 * Armor Type Modifier
   * Commercial Armor Modifier = 0.5 (Currently not implemented)
   * All Other Armor Modifier  = 1.0
   */
  const int ARMOR_POINTS = mech_armorpoints(mech);
  calc_add_def_bv(mech, &defbv, "Armor", (float)ARMOR_POINTS * 2.5F);

  /* INTERNAL/ENGINE
   * Total Internal Points * 1.5 * Internal Type Modifier * Engine Type Modifier
   * Industrial Internal Modifier = 0.5 (Current not implemented)
   * All Other Internal Modifier  = 1.0
   * Light Engine Modifier            = 0.75
   * IS XL/XXL Engine Modifier        = 0.50
   * Clan XL Engine Modifier          = 0.75
   * Standard/Compact Engine Modifier = 1.00
   *
   * Vehicles Engine Modifier = 1.00
   */
  engine_mod = 1.00F;

  if (((mech)->rd.specials) & LE_TECH)
    engine_mod = 0.75F;

  if (((mech)->rd.specials) & XL_TECH) {
    if (((mech)->rd.specials) & CLAN_TECH)
      engine_mod = 0.75F;
    else
      engine_mod = 0.50F;
  }

  if (((mech)->rd.specials) & XXL_TECH)
    engine_mod = 0.50F;

  if (((mech)->ud.type) != CLASS_MECH)
    engine_mod = 1.00F;

  const int INTERNAL_POINTS = mech_intpoints(mech);
  calc_add_def_bv(mech, &defbv, "Internal/Engine",
                  (float)INTERNAL_POINTS * 1.5F * engine_mod);

  /* GYRO
   * Mechs Only
   * Mech Tonnage * Mech Gyro Modifier
   * Heavy Duty Modifier = 1.0
   * Everything Else     = 0.5
   */
  if (((mech)->ud.type) == CLASS_MECH)
    calc_add_def_bv(mech, &defbv, "Gyro (Mech Only)",
                    (float)((mech)->ud.tons) *
                        (((mech)->rd.specials2) & HDGYRO_TECH ? 1.0F : 0.5F));

  /* DEFENSIVE ITEMS/WEAPONS
   * All Defensive items at their BV value (ECM, A-Pod, B-Pod, BAP, AMS, etc)
   */

  for (i = 0; i < NUM_SECTIONS; i++) {
    for (ii = 0; ii < NUM_CRITICALS; ii++) {
      part = mech_critical_part_type(mech, i, ii);
      if (equipment_is_special(part)) {
        switch (special_from_equipment_index(part)) {
        case ECM:
          /* Checking for a full System. Mechas are 2 crits per full system */
          ecm_count++;
          break;
        case BEAGLE_PROBE:
          /* Checking for a full System. Mechas are 2 crits per full system */
          bap_count++;
          break;
        default:
          break;
        }
      } /* End IfSpecial */

      if (equipment_is_ammunition(part)) {
        weapindx = ammunition_to_weapon_index(part);
        if (weapon_catalogue_has_special(weapindx, AMS)) {
          const int AMMO_BV =
              weapon_catalogue_ammunition_battle_value(weapindx);

          calc_add_def_bv(mech, &defbv, "AMS Ammo", (float)AMMO_BV);
        }
        if (((mech)->ud.type) == CLASS_MECH) {
          if ((i == CTORSO || i == LLEG || i == RLEG || i == HEAD) &&
              (((mech)->rd.specials) & CLAN_TECH)) {

            calc_sub_def_bv(mech, &defbv, "Explosive Ammo", 15.0F);

          } else if ((((mech)->rd.specials) & (XL_TECH | XXL_TECH))) {

            calc_sub_def_bv(mech, &defbv, "Exp Ammo in XL/XXL", 15.0F);

          } else if ((i == CTORSO || i == LLEG || i == RLEG || i == HEAD) ||
                     !(mech_section_configuration(mech, i) & CASE_TECH)) {

            calc_sub_def_bv(mech, &defbv, "Exp Ammo Fusion/!CASE", 15.0F);
          }
        }
      } /* End IsAmmo */

      if (equipment_is_weapon(part)) {
        weapindx = weapon_from_equipment_index(part);
        if (weapon_catalogue_has_special(weapindx, A_POD)) {
          const int WEAPON_BV = mech_weapon_battle_value(mech, weapindx);

          calc_add_def_bv(mech, &defbv, "A POD", (float)WEAPON_BV);
        }
        if (weapon_catalogue_has_special(weapindx, AMS)) {
          const int WEAPON_BV = mech_weapon_battle_value(mech, weapindx);

          calc_add_def_bv(mech, &defbv, "AMS", (float)WEAPON_BV);
        }
        /*                      if(MechWeapons[weapindx].special & B_POD) {
                                        Calc_AddDefBV(mech, &defbv,"B POD",
           mech_weapon_battle_value(mech, weapindx));
                                }
        */
        if ((i == CTORSO || i == LLEG || i == RLEG || i == HEAD) &&
            (((mech)->rd.specials) & CLAN_TECH)) {
          if (weapon_catalogue_has_special(weapindx, GAUSS)) {
            calc_sub_def_bv(mech, &defbv, "Gauss Crit", 1.0F);
          }
        } else if ((((mech)->rd.specials) & (XL_TECH | XXL_TECH))) {
          if (weapon_catalogue_has_special(weapindx, GAUSS)) {
            calc_sub_def_bv(mech, &defbv, "Gauss Crit XL/XXL", 1.0F);
          }
        } else if ((i == CTORSO || i == LLEG || i == RLEG || i == HEAD) &&
                   !(mech_section_configuration(mech, i) & CASE_TECH)) {
          if (weapon_catalogue_has_special(weapindx, GAUSS)) {
            calc_sub_def_bv(mech, &defbv, "Gauss Crit !Case", 1.0F);
          }
        } else if ((((i == RARM) &&
                     !(mech_section_configuration(mech, RTORSO) & CASE_TECH)) ||
                    ((i == LARM) && !(mech_section_configuration(mech, LTORSO) &
                                      CASE_TECH))) &&
                   !(((mech)->rd.specials) & (XL_TECH | XXL_TECH))) {
          if (weapon_catalogue_has_special(weapindx, GAUSS)) {
            calc_sub_def_bv(mech, &defbv, "Gauss Crit Fusion/!Case", 1.0F);
          }
        }
      } /* End IsWeapon */

    } /* End Critical For loop */

  } /* End Section For Loop */

  /* TODO: Angel ECM, Bloodhound, Light_BAP */

  if ((((ecm_count / 2) > 0) && (((mech)->ud.type) == CLASS_MECH)) ||
      ((ecm_count > 0) && (((mech)->ud.type) != CLASS_MECH)) ||
      ((ecm_count > 0) && (((mech)->ud.type) == CLASS_MECH) &&
       (((mech)->rd.specials) &
        CLAN_TECH))) { /* ECM is 2 crits for mechas, one Crit for Clan Mechas.
                          One System = 61 BV */

    calc_add_def_bv(mech, &defbv, "ECM", 61.0F);
  }

  if ((((bap_count / 2) > 0) && (((mech)->ud.type) == CLASS_MECH)) ||
      ((bap_count > 0) && (((mech)->ud.type) != CLASS_MECH)) ||
      ((bap_count > 0) && (((mech)->ud.type) == CLASS_MECH) &&
       (((mech)->rd.specials) &
        CLAN_TECH))) { /* BAP is 2 crits for mechas, one Crit for Clan Mechas.
                          One System = 10 BV */

    calc_add_def_bv(mech, &defbv, "BAP", 10.0F);
  }

  /* UNIT TYPE MODIFIER */
  /* Mainly for vehicles. Chart on Techmanual, Page 316 */
  /* We're doing a reverse on the values to make the addtion easy */

  if (((mech)->ud.move) == MOVE_TRACK)
    calc_sub_def_bv(mech, &defbv, "UnitType Tracked", defbv * 0.1F);

  if (((mech)->ud.move) == MOVE_WHEEL)
    calc_sub_def_bv(mech, &defbv, "UnitType Wheeled", defbv * 0.2F);

  if (((mech)->ud.move) == MOVE_HOVER)
    calc_sub_def_bv(mech, &defbv, "UnitType Hover", defbv * 0.3F);

  if ((((mech)->ud.move) == MOVE_SUB || ((mech)->ud.move) == MOVE_FOIL ||
       ((mech)->ud.move) == MOVE_HULL))
    if (((mech)->ud.move) != MOVE_HOVER)
      calc_sub_def_bv(mech, &defbv, "UnitType Naval", defbv * 0.4F);

  if (((mech)->ud.move) == MOVE_VTOL)
    calc_sub_def_bv(mech, &defbv, "UnitType VTOL", defbv * 0.3F);

  /* TODO: Airship, Aero (DS is 1.0, no need) */

  /* Defensive Factor
   * Highest target movement modifier. See Techmanual Page 315 for chart.
   */

  /* Based off Standard calcs for Movement Modification (BTH+)
   */

  /* Determine base mp */
  jump_mp = clamp_float_to_int(((mech)->rd.jumpspeed) / MP1);
  const float MAXIMUM_SPEED = mech_effective_maximum_speed(mech);
  run_mp = clamp_float_to_int(MAXIMUM_SPEED / MP1);

  if (((mech)->rd.specials) & TRIPLE_MYOMER_TECH)
    run_mp =
        clamp_float_to_int((rintf((MAXIMUM_SPEED / 1.5F) / MP1) + 1.0F) * 1.5F);

  if (((mech)->rd.specials) & MASC_TECH) {
    if (((mech)->rd.specials2) & SUPERCHARGER_TECH) {
      const int WALK_MP = (run_mp * 2) / 3;
      run_mp = clamp_float_to_int((float)WALK_MP *
                                  2.5F); /* walk mp * 2.5, round down */
    }
    run_mp = ((run_mp * 2) / 3) * 2; /* 2x walk mp */
  } else if (((mech)->rd.specials2) & SUPERCHARGER_TECH) {
    run_mp = ((run_mp * 2) / 3) * 2; /* 2x walk mp */
  }

  /* Determine move_mod */

  if (run_mp > 24)
    move_mod = 6;
  if (run_mp > 17 && run_mp < 25)
    move_mod = 5;
  if (run_mp > 9 && run_mp < 18)
    move_mod = 4;
  if (run_mp > 6 && run_mp < 10)
    move_mod = 3;
  if (run_mp > 4 && run_mp < 7)
    move_mod = 2;
  if (run_mp > 2 && run_mp < 5)
    move_mod = 1;
  if (run_mp == 1 || run_mp == 2)
    move_mod = 0;

  if (((mech)->ud.type) == CLASS_BSUIT || ((mech)->ud.type) == CLASS_VTOL ||
      ((mech)->ud.type) == CLASS_AERO)
    /* vtol/aero/suit = +1 move mod */
    move_mod++;

  if (jump_mp > run_mp)
    move_mod++;

  /* TODO: Add Camo/BA adjustments (TechManual, p315) */

  if (((mech)->rd.specials) & STEALTH_ARMOR_TECH)
    move_mod = move_mod + 2;

  def_factor = (float)move_mod * 0.1F;

  (void)snprintf(buff, 50, "MoveMod (MP: %d MM: %d)", run_mp, move_mod);

  calc_add_def_bv(mech, &defbv, buff, defbv * def_factor);

  defbv = roundf(defbv * 100.0F) / 100.0F;
  /* END DEFENSIVE BV */
  return defbv;
}

/* Calculate Offensive BV 2.0 per Total Warfare Rules */
float calculate_offensive_bv(Mech *mech) {
  float offbv = 0.0F;
  int heat_efficiency;
  int heat_sinks;
  int jump_mp;
  int i;
  int ii;
  int j;
  [[maybe_unused]] int weapindx;
  int count;
  int tablecount = 0;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int weaptable[64];
  int heattable[64];
  int bvtable[64];
  int wt;
  int bt;
  int ht;
  int heatcount = 0;

  /* First Find Heat Efficiency */

  jump_mp = clamp_float_to_int(((mech)->rd.jumpspeed) / MP1);
  heat_sinks = clamp_float_to_int(mech_active_heat_sinks(mech));
  heat_efficiency = 6 + heat_sinks - (jump_mp > 2 ? jump_mp : 2);

  /* Let's Gather Weapons First*/
  for (i = 0; i < NUM_SECTIONS; i++) {
    count = find_weapons_advanced(mech, i, weaparray, weapdata, critical, 1);
    if (count <= 0) {
      continue;
    }

    for (ii = 0; ii < count; ii++) {
      const int WEAPON = battle_value_weapon_get(weaparray, ii);

      /* Exclude Defensive Weapons */
      if (weapon_catalogue_has_special(WEAPON, AMS))
        continue;

      *battle_value_table_slot(weaptable, tablecount) = WEAPON;
      /* TODO: Modify Ultra/RAC/Streak/Oneshot HEAT values.
       * TC/Oneshot/Rear/Artemis BV Values */
      *battle_value_table_slot(heattable, tablecount) =
          weapon_catalogue_heat(WEAPON);
      *battle_value_table_slot(bvtable, tablecount) =
          weapon_catalogue_battle_value(WEAPON);
      tablecount++;
    }
  }

  /* Sort our temp tables by BV (highest first) */

  for (i = 0; i < (tablecount - 1); i++) {
    for (j = 0; j < tablecount - i - 1; j++) {
      if (battle_value_table_get(bvtable, j) >
          battle_value_table_get(bvtable, j + 1)) {
        wt = battle_value_table_get(weaptable, j);
        ht = battle_value_table_get(heattable, j);
        bt = battle_value_table_get(bvtable, j);
        *battle_value_table_slot(weaptable, j) =
            battle_value_table_get(weaptable, j + 1);
        *battle_value_table_slot(heattable, j) =
            battle_value_table_get(heattable, j + 1);
        *battle_value_table_slot(bvtable, j) =
            battle_value_table_get(bvtable, j + 1);
        *battle_value_table_slot(weaptable, j + 1) = wt;
        *battle_value_table_slot(heattable, j + 1) = ht;
        *battle_value_table_slot(bvtable, j + 1) = bt;
      }
    }
  }

  /* Go through temp tables, adding BV. Half BV if > heat efficiency */
  for (i = (tablecount - 1); i >= 0; i--) {
    const int WEAPON = battle_value_table_get(weaptable, i);
    const int HEAT = battle_value_table_get(heattable, i);
    const int BATTLE_VALUE = battle_value_table_get(bvtable, i);
    if (heatcount + HEAT > heat_efficiency) {
      const int REDUCED_BV = BATTLE_VALUE / 2;

      calc_add_off_bv(mech, &offbv, weapon_catalogue_name(WEAPON),
                      (float)REDUCED_BV);
    } else {
      const int WEAPON_BV = BATTLE_VALUE;

      calc_add_off_bv(mech, &offbv, weapon_catalogue_name(WEAPON),
                      (float)WEAPON_BV);
    }
    heatcount = heatcount + HEAT;
  }

  /* TODO: Physical Weapons */

  /* TODO: Ammo */

  /* TODO: Add Tonnage */

  calc_add_off_bv(mech, &offbv, "MechTonnage", (float)((mech)->ud.tons));

  /* TODO: Speed Factor */

  /* END OFFENSIVE BV */
  return offbv;
}
