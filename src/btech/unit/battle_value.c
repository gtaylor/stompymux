#include "mech_utils_internal.h"

#ifdef BT_CALCULATE_BV
int FindAverageGunnery(Mech *mech) {
#if 1
  /* NULLTODO : Get the multiple skills for gunnery and such ported or working
   * here so this is usefull again. */
  return FindPilotGunnery(mech, 0);
#else
  int runtot = 0;
  int i;

  if (!mech)
    return 12;

  for (i = 0; i < 5; i++) {
    runtot += FindPilotGunnery(mech, (i == 0   ? 0
                                      : i == 1 ? 4
                                      : i == 2 ? 5
                                      : i == 3 ? 6
                                      : i == 4 ? 103
                                               : 0));
  }
  return (runtot / 5);
#endif
}

#undef DEBUG_BV
float skillmul[BTECH_BV_SKILL_LIMIT][BTECH_BV_SKILL_LIMIT] = {
    {2.05, 2.00, 1.95, 1.90, 1.85, 1.80, 1.75, 1.70},
    {1.85, 1.80, 1.75, 1.70, 1.65, 1.60, 1.55, 1.50},
    {1.65, 1.60, 1.55, 1.50, 1.45, 1.40, 1.35, 1.30},
    {1.45, 1.40, 1.35, 1.30, 1.25, 1.20, 1.15, 1.10},
    {1.25, 1.20, 1.15, 1.10, 1.05, 1.00, 0.95, 0.90},
    {1.15, 1.10, 1.05, 1.00, 0.95, 0.90, 0.85, 0.80},
    {1.05, 1.00, 0.95, 0.90, 0.85, 0.80, 0.75, 0.70},
    {0.95, 0.90, 0.85, 0.80, 0.75, 0.70, 0.65, 0.60}};

void Calc_AddOffBV(const Mech *mech, float *offbv, char *desc, float value) {
  *offbv += value;
  if (mech->xcode.context->configuration->btech_cost_debug)
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("AddOffBV %25s %8.2f", desc, value));
}

void Calc_AddDefBV(const Mech *mech, float *defbv, char *desc, float value) {
  *defbv += value;
  if (mech->xcode.context->configuration->btech_cost_debug)
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("AddDefBV %25s %8.2f", desc, value));
}

void Calc_SubDefBV(const Mech *mech, float *defbv, char *desc, float value) {
  *defbv -= value;
  if (mech->xcode.context->configuration->btech_cost_debug)
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("SubDefBV %25s-%8.2f", desc, value));
}

/* Calculate Defensive BV 2.0 per Total Warfare Rules */
float Calculate_Defensive_BV(Mech *mech) {
  float defbv = 0.0;
  float engine_mod = 0.0;
  int i;
  int ii;
  int part;
  int weapindx;
  int ecm_count = 0;
  int bap_count = 0;
  int jump_mp = 0;
  int move_mod = 0;
  int run_mp = 0;
  float def_factor = 0.0;
  char buff[50];

  /* ARMOR
   * Total Armor Factor (Points) * 2.5 * Armor Type Modifier
   * Commercial Armor Modifier = 0.5 (Currently not implemented)
   * All Other Armor Modifier  = 1.0
   */
  Calc_AddDefBV(mech, &defbv, "Armor", mech_armorpoints(mech) * 2.5 * 1.0);

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
  engine_mod = 1.00;

  if (MechSpecials(mech) & LE_TECH)
    engine_mod = 0.75;

  if (MechSpecials(mech) & XL_TECH) {
    if (MechSpecials(mech) & CLAN_TECH)
      engine_mod = 0.75;
    else
      engine_mod = 0.50;
  }

  if (MechSpecials(mech) & XXL_TECH)
    engine_mod = 0.50;

  if (MechType(mech) != CLASS_MECH)
    engine_mod = 1.00;

  Calc_AddDefBV(mech, &defbv, "Internal/Engine",
                mech_intpoints(mech) * 1.5 * 1.0 * engine_mod);

  /* GYRO
   * Mechs Only
   * Mech Tonnage * Mech Gyro Modifier
   * Heavy Duty Modifier = 1.0
   * Everything Else     = 0.5
   */
  if (MechType(mech) == CLASS_MECH)
    Calc_AddDefBV(mech, &defbv, "Gyro (Mech Only)",
                  MechTons(mech) *
                      (MechSpecials2(mech) & HDGYRO_TECH ? 1.0 : 0.5));

  /* DEFENSIVE ITEMS/WEAPONS
   * All Defensive items at their BV value (ECM, A-Pod, B-Pod, BAP, AMS, etc)
   */

  for (i = 0; i < NUM_SECTIONS; i++) {
    for (ii = 0; ii < NUM_CRITICALS; ii++) {
      part = GetPartType(mech, i, ii);
      if (IsSpecial(part)) {
        switch (Special2I(part)) {
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

      if (IsAmmo(part)) {
        weapindx = Ammo2WeaponI(part);
        if (MechWeapons[weapindx].special & AMS) {
          Calc_AddDefBV(mech, &defbv, "AMS Ammo",
                        MechWeapons[weapindx].ammo_bv);
        }
        if (MechType(mech) == CLASS_MECH) {
          if ((i == CTORSO || i == LLEG || i == RLEG || i == HEAD) &&
              (MechSpecials(mech) & CLAN_TECH)) {

            Calc_SubDefBV(mech, &defbv, "Explosive Ammo", 15.0);

          } else if ((MechSpecials(mech) & (XL_TECH | XXL_TECH))) {

            Calc_SubDefBV(mech, &defbv, "Exp Ammo in XL/XXL", 15.0);

          } else if ((i == CTORSO || i == LLEG || i == RLEG || i == HEAD) ||
                     !(MechSections(mech)[i].config & CASE_TECH)) {

            Calc_SubDefBV(mech, &defbv, "Exp Ammo Fusion/!CASE", 15.0);
          }
        }
      } /* End IsAmmo */

      if (IsWeapon(part)) {
        weapindx = Weapon2I(part);
        if (MechWeapons[weapindx].special & A_POD) {
          Calc_AddDefBV(mech, &defbv, "A POD",
                        mech_weapon_battle_value(mech, weapindx));
        }
        if (MechWeapons[weapindx].special & AMS) {
          Calc_AddDefBV(mech, &defbv, "AMS",
                        mech_weapon_battle_value(mech, weapindx));
        }
        /*                      if(MechWeapons[weapindx].special & B_POD) {
                                        Calc_AddDefBV(mech, &defbv,"B POD",
           mech_weapon_battle_value(mech, weapindx));
                                }
        */
        if ((i == CTORSO || i == LLEG || i == RLEG || i == HEAD) &&
            (MechSpecials(mech) & CLAN_TECH)) {
          if (MechWeapons[weapindx].special & GAUSS) {
            Calc_SubDefBV(mech, &defbv, "Gauss Crit", 1.0);
          }
        } else if ((MechSpecials(mech) & (XL_TECH | XXL_TECH))) {
          if (MechWeapons[weapindx].special & GAUSS) {
            Calc_SubDefBV(mech, &defbv, "Gauss Crit XL/XXL", 1.0);
          }
        } else if ((i == CTORSO || i == LLEG || i == RLEG || i == HEAD) &&
                   !(MechSections(mech)[i].config & CASE_TECH)) {
          if (MechWeapons[weapindx].special & GAUSS) {
            Calc_SubDefBV(mech, &defbv, "Gauss Crit !Case", 1.0);
          }
        } else if ((((i == RARM) &&
                     !(MechSections(mech)[RTORSO].config & CASE_TECH)) ||
                    ((i == LARM) &&
                     !(MechSections(mech)[LTORSO].config & CASE_TECH))) &&
                   !(MechSpecials(mech) & (XL_TECH | XXL_TECH))) {
          if (MechWeapons[weapindx].special & GAUSS) {
            Calc_SubDefBV(mech, &defbv, "Gauss Crit Fusion/!Case", 1.0);
          }
        }
      } /* End IsWeapon */

    } /* End Critical For loop */

  } /* End Section For Loop */

  /* TODO: Angel ECM, Bloodhound, Light_BAP */

  if ((((ecm_count / 2) > 0) && (MechType(mech) == CLASS_MECH)) ||
      ((ecm_count > 0) && (MechType(mech) != CLASS_MECH)) ||
      ((ecm_count > 0) && (MechType(mech) == CLASS_MECH) &&
       (MechSpecials(mech) &
        CLAN_TECH))) { /* ECM is 2 crits for mechas, one Crit for Clan Mechas.
                          One System = 61 BV */

    Calc_AddDefBV(mech, &defbv, "ECM", 61.0);
  }

  if ((((bap_count / 2) > 0) && (MechType(mech) == CLASS_MECH)) ||
      ((bap_count > 0) && (MechType(mech) != CLASS_MECH)) ||
      ((bap_count > 0) && (MechType(mech) == CLASS_MECH) &&
       (MechSpecials(mech) &
        CLAN_TECH))) { /* BAP is 2 crits for mechas, one Crit for Clan Mechas.
                          One System = 10 BV */

    Calc_AddDefBV(mech, &defbv, "BAP", 10.0);
  }

  /* UNIT TYPE MODIFIER */
  /* Mainly for vehicles. Chart on Techmanual, Page 316 */
  /* We're doing a reverse on the values to make the addtion easy */

  if (MechMove(mech) == MOVE_TRACK)
    Calc_SubDefBV(mech, &defbv, "UnitType Tracked", defbv * 0.1);

  if (MechMove(mech) == MOVE_WHEEL)
    Calc_SubDefBV(mech, &defbv, "UnitType Wheeled", defbv * 0.2);

  if (MechMove(mech) == MOVE_HOVER)
    Calc_SubDefBV(mech, &defbv, "UnitType Hover", defbv * 0.3);

  if ((MechMove(mech) == MOVE_SUB || MechMove(mech) == MOVE_FOIL ||
       MechMove(mech) == MOVE_HULL))
    if (MechMove(mech) != MOVE_HOVER)
      Calc_SubDefBV(mech, &defbv, "UnitType Naval", defbv * 0.4);

  if (MechMove(mech) == MOVE_VTOL)
    Calc_SubDefBV(mech, &defbv, "UnitType VTOL", defbv * 0.3);

  /* TODO: Airship, Aero (DS is 1.0, no need) */

  /* Defensive Factor
   * Highest target movement modifier. See Techmanual Page 315 for chart.
   */

  /* Based off Standard calcs for Movement Modification (BTH+)
   */

  /* Determine base mp */
  jump_mp = (int)(MechJumpSpeed(mech) / MP1);
  run_mp = (int)(MMaxSpeed(mech) / MP1);

  if (MechSpecials(mech) & TRIPLE_MYOMER_TECH)
    run_mp = ceil((rint((MMaxSpeed(mech) / 1.5) / MP1) + 1) * 1.5);

  if (MechSpecials(mech) & MASC_TECH) {
    if (MechSpecials2(mech) & SUPERCHARGER_TECH) {
      run_mp = ((run_mp * 2) / 3) * 2.5; /* walk mp * 2.5, round down */
    }
    run_mp = ((run_mp * 2) / 3) * 2; /* 2x walk mp */
  } else if (MechSpecials2(mech) & SUPERCHARGER_TECH) {
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

  if (MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_VTOL ||
      MechType(mech) == CLASS_AERO)
    /* vtol/aero/suit = +1 move mod */
    move_mod++;

  if (jump_mp > run_mp)
    move_mod++;

  /* TODO: Add Camo/BA adjustments (TechManual, p315) */

  if (MechSpecials(mech) & STEALTH_ARMOR_TECH)
    move_mod = move_mod + 2;

  def_factor = (move_mod * .1);

  snprintf(buff, 50, "MoveMod (MP: %d MM: %d)", run_mp, move_mod);

  Calc_AddDefBV(mech, &defbv, buff, defbv * def_factor);

  defbv = roundf((defbv * 100.0)) / 100.0;
  /* END DEFENSIVE BV */
  return defbv;
}

/* Calculate Offensive BV 2.0 per Total Warfare Rules */
float Calculate_Offensive_BV(Mech *mech) {
  float offbv = 0.0;
  int heat_efficiency;
  int heat_sinks;
  int jump_mp;
  int i, ii, j;
  [[maybe_unused]] int weapindx;
  int count;
  int tablecount = 0;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int weaptable[64];
  int heattable[64];
  int bvtable[64];
  int wt, bt, ht;
  int heatcount = 0;

  /* First Find Heat Efficiency */

  jump_mp = (int)(MechJumpSpeed(mech) / MP1);
  heat_sinks = MechActiveNumsinks(mech);
  heat_efficiency = 6 + heat_sinks - (jump_mp > 2 ? jump_mp : 2);

  /* Let's Gather Weapons First*/
  for (i = 0; i < NUM_SECTIONS; i++) {
    count = FindWeapons(mech, i, weaparray, weapdata, critical);
    if (count <= 0) {
      continue;
    }

    for (ii = 0; ii < count; ii++) {

      /* Exclude Defensive Weapons */
      if (MechWeapons[weaparray[ii]].special & AMS)
        continue;

      weaptable[tablecount] = weaparray[ii];
      /* TODO: Modify Ultra/RAC/Streak/Oneshot HEAT values.
       * TC/Oneshot/Rear/Artemis BV Values */
      heattable[tablecount] = MechWeapons[weaparray[ii]].heat;
      bvtable[tablecount] = MechWeapons[weaparray[ii]].battlevalue;
      tablecount++;
    }
  }

  /* Sort our temp tables by BV (highest first) */

  for (i = 0; i < (tablecount - 1); i++) {
    for (j = 0; j < tablecount - i - 1; j++) {
      if (bvtable[j] > bvtable[j + 1]) {
        wt = weaptable[j];
        ht = heattable[j];
        bt = bvtable[j];
        weaptable[j] = weaptable[j + 1];
        heattable[j] = heattable[j + 1];
        bvtable[j] = bvtable[j + 1];
        weaptable[j + 1] = wt;
        heattable[j + 1] = ht;
        bvtable[j + 1] = bt;
      }
    }
  }

  /* Go through temp tables, adding BV. Half BV if > heat efficiency */
  for (i = (tablecount - 1); i >= 0; i--) {
    if (heatcount + heattable[i] > heat_efficiency)
      Calc_AddOffBV(mech, &offbv, MechWeapons[weaptable[i]].name,
                    (bvtable[i]) / 2);
    else
      Calc_AddOffBV(mech, &offbv, MechWeapons[weaptable[i]].name, bvtable[i]);
    heatcount = heatcount + heattable[i];
  }

  /* TODO: Physical Weapons */

  /* TODO: Ammo */

  /* TODO: Add Tonnage */

  Calc_AddOffBV(mech, &offbv, "MechTonnage", MechTons(mech));

  /* TODO: Speed Factor */

  /* END OFFENSIVE BV */
  return offbv;
}
#endif
