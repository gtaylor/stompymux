#include "mech_utils_internal.h"

int GetPartWeight(int part) {
  if (IsWeapon(part))
    return 10.24 * MechWeapons[Weapon2I(part)].weight;
  else if (IsAmmo(part))
    return 1024;
  else if (IsBomb(part))
    return 102 * BombWeight(Bomb2I(part));
#ifndef BT_PART_WEIGHTS
  else if (IsSpecial(part) && part <= I2Special(CLAW))
    return 1024;
#else
  else if (IsSpecial(part)) /* && i <= I2Special(LAMEQUIP) */
    return internalsweight[Special2I(part)];
  else if (IsCargo(part))
    return cargoweight[Cargo2I(part)];
#endif /* BT_PART_WEIGHTS */
  else
    /* hmm.. tricky, suppose we'll make things light */
    return 102;
}

#ifdef BT_ADVANCED_ECON
struct BtechPartCosts {
  unsigned long long specials[SPECIALCOST_SIZE];
  unsigned long long ammunition[AMMOCOST_SIZE];
  unsigned long long weapons[WEAPCOST_SIZE];
  unsigned long long cargo[CARGOCOST_SIZE];
  unsigned long long bombs[BOMBCOST_SIZE];
};

void btech_part_costs_initialize(BtechContext *context) {
  context->part_costs = calloc(1, sizeof(*context->part_costs));
  if (context->part_costs == nullptr)
    exit(EXIT_FAILURE);
}

void btech_part_costs_destroy(BtechContext *context) {
  free(context->part_costs);
  context->part_costs = nullptr;
}

void btech_part_costs_reset(BtechContext *context) {
  memset(context->part_costs, 0, sizeof(*context->part_costs));
}

void btech_part_cost_sets(
    const BtechContext *context,
    BtechPartCostSet sets[static BTECH_PART_COST_SET_COUNT]) {
  const BtechPartCosts *costs = context->part_costs;
  sets[0] =
      (BtechPartCostSet){costs->specials, SPECIALCOST_SIZE, SPECIAL_BASE_INDEX};
  sets[1] =
      (BtechPartCostSet){costs->ammunition, AMMOCOST_SIZE, AMMO_BASE_INDEX};
  sets[2] =
      (BtechPartCostSet){costs->weapons, WEAPCOST_SIZE, WEAPON_BASE_INDEX};
  sets[3] = (BtechPartCostSet){costs->cargo, CARGOCOST_SIZE, CARGO_BASE_INDEX};
  sets[4] = (BtechPartCostSet){costs->bombs, BOMBCOST_SIZE, BOMB_BASE_INDEX};
}

unsigned long long btech_part_cost_get(const BtechContext *context, int part) {
  const BtechPartCosts *costs = context->part_costs;
  if (IsWeapon(part))
    return costs->weapons[Weapon2I(part)];
  else if (IsAmmo(part))
    return costs->ammunition[Ammo2I(part)];
  else if (IsSpecial(part))
    return costs->specials[Special2I(part)];
  else if (IsBomb(part))
    return costs->bombs[Bomb2I(part)];
  else if (IsCargo(part))
    return costs->cargo[Cargo2I(part)];
  else
    return 0;
}

void btech_part_cost_set(BtechContext *context, int part,
                         unsigned long long cost) {
  BtechPartCosts *costs = context->part_costs;
  if (IsWeapon(part))
    costs->weapons[Weapon2I(part)] = cost;
  else if (IsAmmo(part))
    costs->ammunition[Ammo2I(part)] = cost;
  else if (IsSpecial(part))
    costs->specials[Special2I(part)] = cost;
  else if (IsBomb(part))
    costs->bombs[Bomb2I(part)] = cost;
  else if (IsCargo(part))
    costs->cargo[Cargo2I(part)] = cost;
}

void CalcFasaCost_AddPrice(const Mech *mech, float *total, char *desc,
                           float value) {
  *total += value;
  if (mech->xcode.context->configuration->btech_cost_debug)
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("Addprice - %25s %8.0f", desc, value));
}

int MechNumHeatsinksInEngine(Mech *mech) {
  // Heatsinks in Engine = Engine Rating / 25
  return (MechEngineSize(mech) / 25);
}

void CalcFasaCost_DoArmMath(Mech *mech, int loc, float *total) {
  int i = 0;
  for (i = 0; i < NUM_CRITICALS; i++) {
    int part = GetPartType(mech, loc, i);
    if (!IsActuator(part))
      continue;
    else if (Special2I(part) == SHOULDER_OR_HIP)
      continue;
    // BMR Says don't count this.
    // CalcFasaCost_AddPrice(mech, total, "Shoulder Actuator", 0);
    else if (Special2I(part) == UPPER_ACTUATOR)
      CalcFasaCost_AddPrice(mech, total, "ARM Upper Actuator",
                            (MechTons(mech) * 100));
    else if (Special2I(part) == LOWER_ACTUATOR)
      CalcFasaCost_AddPrice(mech, total, "ARM Lower Actuator",
                            (MechTons(mech) * 50));
    else if (Special2I(part) == HAND_OR_FOOT_ACTUATOR)
      CalcFasaCost_AddPrice(mech, total, "ARM Hand Actuator",
                            (MechTons(mech) * 80));
  }
}

void CalcFasaCost_DoLegMath(Mech *mech, int loc, float *total) {
  int i = 0;
  for (i = 0; i < NUM_CRITICALS; i++) {
    int part = GetPartType(mech, loc, i);
    if (!IsActuator(part))
      continue;
    else if (Special2I(part) == SHOULDER_OR_HIP)
      continue;
    // BMR Says don't count the Hip
    else if (Special2I(part) == UPPER_ACTUATOR)
      CalcFasaCost_AddPrice(mech, total, "LEG Upper Actuator",
                            (MechTons(mech) * 150));
    else if (Special2I(part) == LOWER_ACTUATOR)
      CalcFasaCost_AddPrice(mech, total, "LEG Lower Actuator",
                            (MechTons(mech) * 80));
    else if (Special2I(part) == HAND_OR_FOOT_ACTUATOR)
      CalcFasaCost_AddPrice(mech, total, "LEG Actuator",
                            (MechTons(mech) * 120));
  }
}

/*
 * Calculate the FASA cost of a unit as per an approximation of Maxtech
 * construction/cost rules.
 */
unsigned long long int CalcFasaCost(Mech *mech) {
  int ii, i, part;
  float total = 0;
  float mod = 1.0;
  int count, ammoweapcount;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  unsigned char ammoweap[8 * MAX_WEAPS_SECTION];
  unsigned short ammo[8 * MAX_WEAPS_SECTION];
  unsigned short ammomax[8 * MAX_WEAPS_SECTION];
  unsigned int modearray[8 * MAX_WEAPS_SECTION];
  int engine_size = 0;
  int has_sword = 0;
  int clan_case_sections[NUM_SECTIONS];

  if (!mech)
    return -1;

  if (!(MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_VEH_GROUND ||
        MechType(mech) == CLASS_VEH_NAVAL || MechType(mech) == CLASS_VTOL ||
        MechType(mech) == CLASS_BSUIT) ||
      is_aero(mech) || IsDS(mech))
    return 0;

  if (MechType(mech) == CLASS_MECH) {

    /* Start MECH Internal Structure Skeleton ( Tech Manual (p278) MaxTech (p87)
     * ) */
    if (MechSpecials(mech) & ES_TECH || MechSpecials(mech) & COMPI_TECH)
      CalcFasaCost_AddPrice(mech, &total, "ES/Co Internals",
                            (MechTons(mech) * 1600));
    else if (MechSpecials(mech) & REINFI_TECH)
      CalcFasaCost_AddPrice(mech, &total, "RE Internals",
                            (MechTons(mech) * 6400));
    else
      CalcFasaCost_AddPrice(mech, &total, "Std Internals",
                            (MechTons(mech) * 400));
/* End MECH Internal Structure Skeleton */

/* Cockpit */
#if 0
/* NULLTODO : Port any of these techs ASAP */
		if(MechSpecials2(mech) & SMALLCOCKPIT_TECH)
			ADDPRICE("SmallCockpit", 175000)
				else
		if(MechSpecials2(mech) & TORSOCOCKPIT_TECH)
			ADDPRICE("TorsoCockpit", 750000)
				else
				EI is 400000 (Clan Tech)
#endif
    if (MechSpecials2(mech) & SMALLCOCKPIT_TECH)
      CalcFasaCost_AddPrice(mech, &total, "Small Cockpit", 175000);
    else
      CalcFasaCost_AddPrice(mech, &total, "Cockpit", 200000);

    /* Start MECH Life Support ( Tech Manual (p278) ) */
    CalcFasaCost_AddPrice(mech, &total, "LifeSupport", 50000);
    /* End MECH Life Support */

    /* Sensors */
    /* TODO: Add variable range and multi-trac II */
    CalcFasaCost_AddPrice(mech, &total, "Sensors", (MechTons(mech) * 2000));

    /* Start MECH Musculatre (Myomer) ( Tech Manual (p278) )*/
    if (MechSpecials(mech) & TRIPLE_MYOMER_TECH)
      CalcFasaCost_AddPrice(mech, &total, "TS Myomer",
                            (MechTons(mech) * 16000));
    else
      CalcFasaCost_AddPrice(mech, &total, "Myomer", (MechTons(mech) * 2000));
    /* End MECH Musculatre (Myomer) */

    /* Actuators */
    CalcFasaCost_DoArmMath(mech, LARM, &total);
    CalcFasaCost_DoArmMath(mech, RARM, &total);

    CalcFasaCost_DoLegMath(mech, LLEG, &total);
    CalcFasaCost_DoLegMath(mech, RLEG, &total);

    /* Gyro */
    i = MechEngineSize(mech);
    if (i % 100)
      i += (100 - (MechEngineSize(mech) % 100));
    i /= 100;

    if (MechSpecials2(mech) & XLGYRO_TECH)
      CalcFasaCost_AddPrice(mech, &total, "XL Gyro", (i * 0.5 * 750000));
    else if (MechSpecials2(mech) & CGYRO_TECH)
      CalcFasaCost_AddPrice(mech, &total, "Compact Gyro", (i * 1.5 * 400000));
    else if (MechSpecials2(mech) & HDGYRO_TECH)
      CalcFasaCost_AddPrice(mech, &total, "HD Gyro", (i * 2 * 500000));
    else
      CalcFasaCost_AddPrice(mech, &total, "Gyro", (i * 300000));

  } else if (MechType(mech) == CLASS_BSUIT) {
    /* ---------------------------------
     * BSuit Costs
     */
    if (MechSpecials(mech) & CLAN_TECH) {
      CalcFasaCost_AddPrice(mech, &total, "Clan Point", 3500000);
    } else {
      CalcFasaCost_AddPrice(mech, &total, "IS Squad", 2400000);
    }

  } else {
    /* ---------------------------------
     * Vehicle Costs
     */
    int pamp = 0, turret = 0;

    for (i = 0; i < NUM_SECTIONS; i++)
      for (ii = 0; ii < NUM_CRITICALS; ii++) {
        if (!(part = GetPartType(mech, i, ii)))
          continue;
        if (!IsWeapon(part))
          continue;
        if (i == TURRET)
          turret += crit_weight(mech, part);
        if (IsEnergy(part)) {
          pamp += crit_weight(mech, part);
          btech_channel_send(
              mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
              tprintf("PAmp Weight: %d", crit_weight(mech, part)));
        }
      }
    /*
     * Internals
     * 10,000 * Structure Tonnage
     */
    int internals = (float)MechTons(mech) * 1000;
    CalcFasaCost_AddPrice(mech, &total, "Internals", internals);
    /*
     * Control Components
     * 10,000 * Control Tonnage
     * Control Tonnage = .05 * Tons
     */
    int control_eq = 10000 * 0.05 * MechTons(mech);
    CalcFasaCost_AddPrice(mech, &total, "Cockpit & Controls", control_eq);
    /*
     * Power Amp
     * 20,000 * Amplifier Tonnage
     */
    if (MechSpecials(mech) & ICE_TECH) {
      int power_amp = 20000 * (pamp / 1024) / 10;
      CalcFasaCost_AddPrice(mech, &total, "Power Amplifiers", power_amp);
    }

    /*
     * Turret
     * Standard: 5,000 * Turret Tonnage
     */
    int turret_price = 5000 * (turret / 10) / 1024;
    CalcFasaCost_AddPrice(mech, &total, "Turret", turret_price);
    /*
     * Lift/Dive Equip (Hovercraft, Hydrofoils, Submarines)
     * 20,000 * Equipment Tonnage
     */
    if (MechMove(mech) == MOVE_HOVER || MechMove(mech) == MOVE_FOIL ||
        MechMove(mech) == MOVE_SUB) {
      float lift_dive = 20000 * (0.1 * MechTons(mech));
      CalcFasaCost_AddPrice(mech, &total, "Lift/Dive Equip", lift_dive);
    }

    if (MechMove(mech) == MOVE_VTOL) {
      float vtol_eq = 40000 * (0.1 * MechTons(mech));
      CalcFasaCost_AddPrice(mech, &total, "Rotor", vtol_eq);
    }
  } // end if (Vehicle Calcs)

  /* ----------------------------
   * General Calculations
   */

  if (MechType(mech) != CLASS_BSUIT) {
    /* Engine Math
     * (Engine Basecost * Engine Rating * Tonnage) / 75
     */
    int engine_basecost = (MechSpecials(mech) & CE_TECH    ? 10000
                           : MechSpecials(mech) & LE_TECH  ? 15000
                           : MechSpecials(mech) & XL_TECH  ? 20000
                           : MechSpecials(mech) & XXL_TECH ? 100000
                           : MechSpecials(mech) & ICE_TECH ? 1250
                                                           : 5000);

    engine_size = MechEngineSize(mech);

    if (MechMove(mech) == MOVE_WHEEL || MechMove(mech) == MOVE_FOIL ||
        MechMove(mech) == MOVE_HOVER || MechMove(mech) == MOVE_HULL ||
        MechMove(mech) == MOVE_SUB || MechMove(mech) == MOVE_VTOL) {
      engine_size = engine_size - susp_factor(mech);
    }
    /* Don't forget to Round up! */
    int engine_price = ceil(((unsigned long long int)engine_basecost *
                             (unsigned long long int)engine_size *
                             (unsigned long long int)MechTons(mech)) /
                            75ULL);
    CalcFasaCost_AddPrice(mech, &total, "Engine", engine_price);

    /* Jump Jets
     * Standard: Tonnage * (number of JJs^2) * 200
     * Improved: Tonnage * (number of JJs^2) * 500
     * Mechanical: Tonnage * (Jumping MP) * 150
     */
    int num_jjs = MechJumpSpeed(mech) * MP_PER_KPH;
    int jj_price = MechTons(mech) * pow(num_jjs, 2) *
                   (MechSpecials2(mech) & IMPROVED_JJ_TECH ? 500.0 : 200.0);
    if (num_jjs > 0)
      CalcFasaCost_AddPrice(mech, &total, "Jumpjets", jj_price);

    /*
       Heat Sinks
    */
    int numsinks = MechRealNumsinks(mech);

    int sinkcost;
    if (MechSpecials(mech) & DOUBLE_HEAT_TECH || MechSpecials(mech) & CLAN_TECH)
      sinkcost = 6000;
    else if (MechSpecials2(mech) & COMPACT_HS_TECH)
      sinkcost = 3000;
    else
      sinkcost = 2000;

    if ((MechSpecials(mech) & DOUBLE_HEAT_TECH ||
         MechSpecials(mech) & CLAN_TECH)) {
      /* We want to divide the heat dissipation by two if DHS */
      numsinks = BOUNDED(0, numsinks / 2, 500);
    }

    // For single heatsinks, we only charge for every heatsink over 10.
    if (MechSpecials(mech) & DOUBLE_HEAT_TECH ||
        MechSpecials(mech) & CLAN_TECH ||
        MechSpecials2(mech) & COMPACT_HS_TECH || MechSpecials(mech) & ICE_TECH)
      CalcFasaCost_AddPrice(mech, &total, "Heat Sinks", (numsinks * sinkcost));
    else {
      CalcFasaCost_AddPrice(mech, &total, "Heat Sinks",
                            (BOUNDED(0, numsinks - 10, 500) * sinkcost));
    }

#if COST_DEBUG
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
        tprintf("Heat Sinks: %d, Cost Per Sink: %d", numsinks, sinkcost));
#endif

    /* Armor */
    int total_armor = 0;
#if COST_DEBUG
    int orig_armor = 0;
#endif
    int armor_section = 0;
    for (armor_section = 0; armor_section < NUM_SECTIONS; ++armor_section) {
      total_armor += GetSectOArmor(mech, armor_section);
      total_armor += GetSectORArmor(mech, armor_section);
    }

#if COST_DEBUG
    orig_armor = total_armor;
#endif

    if (MechSpecials(mech) & FF_TECH)
      total_armor =
          total_armor * 50 / ((MechSpecials(mech) & CLAN_TECH) ? 60 : 56);
    else if (MechSpecials2(mech) & HVY_FF_ARMOR_TECH)
      total_armor = total_armor * 50 / 62;
    else if (MechSpecials2(mech) & LT_FF_ARMOR_TECH)
      total_armor = total_armor * 50 / 53;

    /* Come on. Really. We don't do .1 of armor. Round this !!! */
    float armor_tons = round_to_halfton(total_armor * 1024 / 16);
    armor_tons = armor_tons / 1024;

    int armor_cost_point = (MechSpecials(mech) & FF_TECH               ? 20000
                            : MechSpecials2(mech) & STEALTH_ARMOR_TECH ? 50000
                            : MechSpecials(mech) & HARDA_TECH          ? 15000
                            : MechSpecials2(mech) & LT_FF_ARMOR_TECH   ? 15000
                            : MechSpecials2(mech) & HVY_FF_ARMOR_TECH  ? 25000
                                                                       : 10000);
#if COST_DEBUG
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
        tprintf("Armor Tons %.1f(%d pts) * Armor Cost Per Point %d", armor_tons,
                orig_armor, armor_cost_point));
#endif
    int armor_price = armor_tons * armor_cost_point;
    CalcFasaCost_AddPrice(mech, &total, "Armor", armor_price);
  } // End Non-BSuit General Calculations

  /* Weapons. */
  /* While it might not make much sense to do this twice, we need to go through
   * all the sections */
  /* and handle weapons first, than we'll handle parts */

  for (i = 0; i < NUM_SECTIONS; i++) {
    count = FindWeapons(mech, i, weaparray, weapdata, critical);
    if (count <= 0)
      /* No weapons */
      continue;

    for (ii = 0; ii < count; ii++) {
      CalcFasaCost_AddPrice(mech, &total, MechWeapons[weaparray[ii]].name,
                            MechWeapons[weaparray[ii]].cost);
    }
  }

  /* Ammo */

  ammoweapcount = FindAmmunition(mech, ammoweap, ammo, ammomax, modearray, 0);

  if (ammoweapcount > 0) {
    if (mech->xcode.context->configuration->btech_cost_debug)
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                         "Ammo Costs");
    for (i = 0; i < ammoweapcount; i++) {
      /* ArtemisIV ammo is X2 */
      /* Interesting way to handle half_tons */
      if (ammomax[i] < MechWeapons[ammoweap[i]].ammoperton)
        CalcFasaCost_AddPrice(
            mech, &total, MechWeapons[ammoweap[i]].name,
            MechWeapons[ammoweap[i]].ammo_cost /
                (MechWeapons[ammoweap[i]].ammoperton / ammomax[i]));
      else
        CalcFasaCost_AddPrice(
            mech, &total, MechWeapons[ammoweap[i]].name,
            MechWeapons[ammoweap[i]].ammo_cost *
                (ammomax[i] / MechWeapons[ammoweap[i]].ammoperton) *
                ((modearray[i] & ARTEMIS_MODE) ? 2 : 1));
    }
  }

  /* Parts */

  int masc_count = 0;
  int bloodhound_count = 0;
  for (i = 0; i < NUM_SECTIONS; i++)
    for (ii = 0; ii < NUM_CRITICALS; ii++) {
      part = GetPartType(mech, i, ii);
      if (IsActuator(part) || part == EMPTY)
        continue;
      if (IsSpecial(part))
        /* These parts are handled above, don't count their crits */
        switch (Special2I(part)) {
        case LIFE_SUPPORT:
          continue;
        case SENSORS:
          continue;
        case COCKPIT:
          continue;
        case ENGINE:
          continue;
        case GYRO:
          continue;
        case HEAT_SINK:
          continue;
        case JUMP_JET:
          continue;
        case FERRO_FIBROUS:
          continue;
        case LT_FERRO_FIBROUS:
          continue;
        case ENDO_STEEL:
          continue;
        case TRIPLE_STRENGTH_MYOMER:
          continue;
        case STEALTH_ARMOR:
          continue;
        case MASC:
          masc_count++;
          continue;
        case SWORD:
          has_sword = 1;
          continue;
        case CASE:
          CalcFasaCost_AddPrice(mech, &total, "Int Case", 50000);
          continue;
        case CASEII:
          CalcFasaCost_AddPrice(mech, &total, "Int CaseII", 175000);
          continue;
        case AXE:
          CalcFasaCost_AddPrice(mech, &total, "Int Axe", 5000);
          continue;
        case BEAGLE_PROBE:
          CalcFasaCost_AddPrice(mech, &total, "BAP", 100000);
          continue;
        case LIGHT_BAP:
          CalcFasaCost_AddPrice(mech, &total, "LightBAP", 50000);
          continue;
        case BLOODHOUND_PROBE:
          bloodhound_count++;
          continue;
        case ARTEMIS_IV:
          CalcFasaCost_AddPrice(mech, &total, "ArtemisIV FCS", 100000);
          continue;
        case ANGELECM:
          CalcFasaCost_AddPrice(mech, &total, "Angel ECM", 375000);
          continue;
        case C3_MASTER:
          CalcFasaCost_AddPrice(mech, &total, "C3M", 300000);
          continue;
        case C3_SLAVE:
          CalcFasaCost_AddPrice(mech, &total, "C3S", 250000);
          continue;
        case C3I:
          CalcFasaCost_AddPrice(mech, &total, "C3I", 375000);
          continue;
        case ECM:
          CalcFasaCost_AddPrice(mech, &total, "ECM", 100000);
          continue;
        case TAG:
          CalcFasaCost_AddPrice(mech, &total, "TAG", 50000);
          continue;
        case TARGETING_COMPUTER:
          CalcFasaCost_AddPrice(mech, &total, "TargComp", 10000);
          continue;

#if 0
/* NULLTODO : Port any of these techs ASAP */
						case HARDPOINT:
							continue;
#endif
        default:
          break;
        }
      if (IsAmmo(part))
        /* Need Something in here to do CASE for CLAN mechs */
        if (ClanMech(mech))
          clan_case_sections[i] = 1;
      continue;
      if (IsWeapon(part))
        continue;

      long indiv_part_cost = btech_part_cost_get(mech->xcode.context, part);
      if (MechType(mech) != CLASS_MECH && IsWeapon(part)) {
        indiv_part_cost *= MechWeapons[part - 1].criticals;
        // btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
        // tprintf("Part#: %s(%d) Crits: %d", MechWeapons[part-1].name, part-1,
        // MechWeapons[part-1].criticals));
      }
      CalcFasaCost_AddPrice(
          mech, &total, (char *)part_name(mech->xcode.context, part, 0).text,
          indiv_part_cost);
    }
  /* We have to account for some other stuff that doesn't divide equally here */
  if (bloodhound_count / 3)
    CalcFasaCost_AddPrice(mech, &total, "Bloodhound",
                          500000 * (bloodhound_count / 3));
  if (masc_count)
    CalcFasaCost_AddPrice(mech, &total, "MASC",
                          masc_count * engine_size * 1000);
  if (has_sword) {
    /* Sword Cost is Tonnage of sword * 10000. Sword Tonnage is 1/20th of Mech
     * Tonnage, rounded up to nearest halfton */
    float sword_tons = round_to_halfton(MechTons(mech) * 1024 / 20);
    sword_tons = sword_tons / 1024;
    CalcFasaCost_AddPrice(mech, &total, "Sword", sword_tons * 10000);
  }

  /* Clan Case */
  if (ClanMech(mech)) {
    for (i = 0; i < NUM_SECTIONS; i++) {
      if (clan_case_sections[i] == 1)
        CalcFasaCost_AddPrice(mech, &total, "Clan CASE Section", 50000);
    }
  }

  if (MechType(mech) != CLASS_MECH && MechType(mech) != CLASS_BSUIT) {
    switch (MechMove(mech)) {
    case MOVE_TRACK:
      mod = (float)1 + (float)((float)MechTons(mech) / (float)100);
      break;
    case MOVE_WHEEL:
      mod = (float)1 + (float)((float)MechTons(mech) / (float)200);
      break;
    case MOVE_HOVER:
      mod = (float)1 + (float)((float)MechTons(mech) / (float)50);
      break;
    case MOVE_VTOL:
      mod = (float)1 + (float)((float)MechTons(mech) / (float)30);
      break;
    case MOVE_HULL:
      mod = (float)1 + (float)((float)MechTons(mech) / (float)200);
      break;
    case MOVE_FOIL:
      mod = (float)1 + (float)((float)MechTons(mech) / (float)75);
      break;
    case MOVE_SUB:
      mod = (float)1 + (float)((float)MechTons(mech) / (float)50);
      break;
    }
  } else if (MechType(mech) == CLASS_BSUIT) {
    // There's nothing in Maxtech about this, but we're going to knock the
    // prices down to be competitive with other unit types.
    mod = 0.75;
  } else {
    // The standard mech size cost modifier. 20 ton mech, for example is Cost *
    // .20
    mod = (float)1 + (float)((float)MechTons(mech) / (float)100);
  }

  if (MechIsOmniMech(mech)) {
    CalcFasaCost_AddPrice(mech, &total, "OmniMech", (int)((float)total * .25));
  }

#if COST_DEBUG
  btech_channel_send(
      mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
      tprintf("Price Total %.0f * Mod - %f = %.0f", total, mod, total * mod));
#endif

  return (total * mod);
} /* End Function */

#endif
