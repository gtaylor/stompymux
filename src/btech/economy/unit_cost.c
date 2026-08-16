#include "context_internal.h"
#include "part_cost_api.h"
#include "unit_cost_api.h"
#include "weapon_catalogue_api.h"

#include <math.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#include "aero_bomb_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_consistency_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_partnames_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"
#include "template_internal.h"

static const int *int_at(const int *values, size_t count, size_t index) {
  return checked_storage_at_const(values, count, sizeof(*values), index);
}

static const bool *bool_at(const bool *values, size_t count, size_t index) {
  return checked_storage_at_const(values, count, sizeof(*values), index);
}

static bool *mutable_bool_at(bool *values, size_t count, size_t index) {
  return checked_storage_at(values, count, sizeof(*values), index);
}

static const unsigned char *weapon_index_at(const unsigned char *values,
                                            size_t count, size_t index) {
  return checked_storage_at_const(values, count, sizeof(*values), index);
}

static const unsigned short *unsigned_short_at(const unsigned short *values,
                                               size_t count, size_t index) {
  return checked_storage_at_const(values, count, sizeof(*values), index);
}

static const unsigned int *unsigned_int_at(const unsigned int *values,
                                           size_t count, size_t index) {
  return checked_storage_at_const(values, count, sizeof(*values), index);
}

int btech_part_weight(int part) {
  if (equipment_is_weapon(part)) {
    const int WEAPON_INDEX = weapon_from_equipment_index(part);
    const int CATALOGUE_WEIGHT = weapon_catalogue_weight(WEAPON_INDEX);
    const float PART_WEIGHT = 10.24F * (float)CATALOGUE_WEIGHT;
    return (int)PART_WEIGHT;
  }
  if (equipment_is_ammunition(part))
    return 1024;
  if (equipment_is_bomb(part))
    return 102 * bomb_weight(bomb_from_equipment_index(part));
  if (equipment_is_special(
          part)) /* && i <= special_equipment_index(LAMEQUIP) */
    return *int_at(INTERNALSWEIGHT, (size_t)TEMPLATE_INTERNAL_COUNT,
                   (size_t)special_from_equipment_index(part));
  if (equipment_is_cargo(part))
    return *int_at(CARGOWEIGHT, (size_t)TEMPLATE_CARGO_COUNT,
                   (size_t)cargo_from_equipment_index(part));
  /* hmm.. tricky, suppose we'll make things light */
  return 102;
}

struct BtechPartCosts {
  unsigned long long specials[SPECIALCOST_SIZE];
  unsigned long long ammunition[AMMOCOST_SIZE];
  unsigned long long weapons[WEAPCOST_SIZE];
  unsigned long long cargo[CARGOCOST_SIZE];
  unsigned long long bombs[BOMBCOST_SIZE];
};

static unsigned long long *part_cost_at(unsigned long long *costs, size_t count,
                                        size_t index) {
  return checked_storage_at(costs, count, sizeof(*costs), index);
}

static const unsigned long long *
part_cost_at_const(const unsigned long long *costs, size_t count,
                   size_t index) {
  return checked_storage_at_const(costs, count, sizeof(*costs), index);
}

static BtechPartCostSet *part_cost_set_at(BtechPartCostSet *sets,
                                          size_t index) {
  return checked_storage_at(sets, BTECH_PART_COST_SET_COUNT, sizeof(*sets),
                            index);
}

void btech_part_costs_initialize(BtechContext *context) {
  context->part_costs =
      checked_storage_try_allocate_array(1, sizeof(*context->part_costs));
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
  *part_cost_set_at(sets, 0) =
      (BtechPartCostSet){costs->specials, SPECIALCOST_SIZE, SPECIAL_BASE_INDEX};
  *part_cost_set_at(sets, 1) =
      (BtechPartCostSet){costs->ammunition, AMMOCOST_SIZE, AMMO_BASE_INDEX};
  *part_cost_set_at(sets, 2) =
      (BtechPartCostSet){costs->weapons, WEAPCOST_SIZE, WEAPON_BASE_INDEX};
  *part_cost_set_at(sets, 3) =
      (BtechPartCostSet){costs->cargo, CARGOCOST_SIZE, CARGO_BASE_INDEX};
  *part_cost_set_at(sets, 4) =
      (BtechPartCostSet){costs->bombs, BOMBCOST_SIZE, BOMB_BASE_INDEX};
}

unsigned long long btech_part_cost_get(const BtechContext *context, int part) {
  const BtechPartCosts *costs = context->part_costs;
  if (equipment_is_weapon(part))
    return *part_cost_at_const(costs->weapons, WEAPCOST_SIZE,
                               (size_t)weapon_from_equipment_index(part));
  if (equipment_is_ammunition(part))
    return *part_cost_at_const(costs->ammunition, AMMOCOST_SIZE,
                               (size_t)ammunition_to_weapon_index(part));
  if (equipment_is_special(part))
    return *part_cost_at_const(costs->specials, SPECIALCOST_SIZE,
                               (size_t)special_from_equipment_index(part));
  if (equipment_is_bomb(part))
    return *part_cost_at_const(costs->bombs, BOMBCOST_SIZE,
                               (size_t)bomb_from_equipment_index(part));
  if (equipment_is_cargo(part))
    return *part_cost_at_const(costs->cargo, CARGOCOST_SIZE,
                               (size_t)cargo_from_equipment_index(part));
  return 0;
}

void btech_part_cost_set(BtechContext *context, int part,
                         unsigned long long cost) {
  BtechPartCosts *costs = context->part_costs;
  if (equipment_is_weapon(part))
    *part_cost_at(costs->weapons, WEAPCOST_SIZE,
                  (size_t)weapon_from_equipment_index(part)) = cost;
  else if (equipment_is_ammunition(part))
    *part_cost_at(costs->ammunition, AMMOCOST_SIZE,
                  (size_t)ammunition_to_weapon_index(part)) = cost;
  else if (equipment_is_special(part))
    *part_cost_at(costs->specials, SPECIALCOST_SIZE,
                  (size_t)special_from_equipment_index(part)) = cost;
  else if (equipment_is_bomb(part))
    *part_cost_at(costs->bombs, BOMBCOST_SIZE,
                  (size_t)bomb_from_equipment_index(part)) = cost;
  else if (equipment_is_cargo(part))
    *part_cost_at(costs->cargo, CARGOCOST_SIZE,
                  (size_t)cargo_from_equipment_index(part)) = cost;
}

static void mech_cost_add(const Mech *mech, double *total, const char *desc,
                          double value) {
  *total += value;
  if (mech_context(mech)->configuration->btech_cost_debug)
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG,
                       "Addprice - %25s %8.0f", desc, value);
}

int mech_engine_heat_sink_capacity(const Mech *mech) {
  // Heatsinks in Engine = Engine Rating / 25
  return mech_engine_rating(mech) / 25;
}

static void mech_cost_add_arm_actuators(Mech *mech, int loc, double *total) {
  int i = 0;
  int const TONS = mech_tonnage(mech);
  for (i = 0; i < NUM_CRITICALS; i++) {
    int part = mech_critical_part_type(mech, loc, i);
    if (!equipment_is_actuator(part))
      continue;
    if (special_from_equipment_index(part) == SHOULDER_OR_HIP)
      continue;
    // BMR Says don't count this.
    // mech_cost_add(mech, total, "Shoulder Actuator", 0);
    if (special_from_equipment_index(part) == UPPER_ACTUATOR)
      mech_cost_add(mech, total, "ARM Upper Actuator", (TONS * 100));
    else if (special_from_equipment_index(part) == LOWER_ACTUATOR)
      mech_cost_add(mech, total, "ARM Lower Actuator", (TONS * 50));
    else if (special_from_equipment_index(part) == HAND_OR_FOOT_ACTUATOR)
      mech_cost_add(mech, total, "ARM Hand Actuator", (TONS * 80));
  }
}

static void mech_cost_add_leg_actuators(Mech *mech, int loc, double *total) {
  int i = 0;
  int const TONS = mech_tonnage(mech);
  for (i = 0; i < NUM_CRITICALS; i++) {
    int part = mech_critical_part_type(mech, loc, i);
    if (!equipment_is_actuator(part))
      continue;
    if (special_from_equipment_index(part) == SHOULDER_OR_HIP)
      continue;
    // BMR Says don't count the Hip
    if (special_from_equipment_index(part) == UPPER_ACTUATOR)
      mech_cost_add(mech, total, "LEG Upper Actuator", (TONS * 150));
    else if (special_from_equipment_index(part) == LOWER_ACTUATOR)
      mech_cost_add(mech, total, "LEG Lower Actuator", (TONS * 80));
    else if (special_from_equipment_index(part) == HAND_OR_FOOT_ACTUATOR)
      mech_cost_add(mech, total, "LEG Actuator", (TONS * 120));
  }
}

/*
 * Calculate the FASA cost of a unit as per an approximation of Maxtech
 * construction/cost rules.
 */
unsigned long long mech_fasa_cost(Mech *mech) {
  int ii;
  int i;
  int part;
  double total = 0.0;
  double mod = 1.0;
  int count;
  int ammoweapcount;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  unsigned char ammoweap[8 * MAX_WEAPS_SECTION];
  unsigned short ammo[8 * MAX_WEAPS_SECTION];
  unsigned short ammomax[8 * MAX_WEAPS_SECTION];
  unsigned int modearray[8 * MAX_WEAPS_SECTION];
  int engine_size = 0;
  int has_sword = 0;
  bool clan_case_sections[NUM_SECTIONS] = {};

  if (!mech)
    return 0;

  int const UNIT_CLASS = mech_class(mech);
  int const MOVEMENT = mech_movement_type(mech);
  int const TONS = mech_tonnage(mech);
  int const TECHNOLOGY = mech_technology_flags(mech);
  int const TECHNOLOGY_SECONDARY = mech_technology_flags_secondary(mech);

  if (!(UNIT_CLASS == CLASS_MECH || UNIT_CLASS == CLASS_VEH_GROUND ||
        UNIT_CLASS == CLASS_VEH_NAVAL || UNIT_CLASS == CLASS_VTOL ||
        UNIT_CLASS == CLASS_BSUIT))
    return 0;

  if (UNIT_CLASS == CLASS_MECH) {

    /* Start MECH Internal Structure Skeleton ( Tech Manual (p278) MaxTech (p87)
     * ) */
    if (TECHNOLOGY & ES_TECH || TECHNOLOGY & COMPI_TECH)
      mech_cost_add(mech, &total, "ES/Co Internals", (TONS * 1600));
    else if (TECHNOLOGY & REINFI_TECH)
      mech_cost_add(mech, &total, "RE Internals", (TONS * 6400));
    else
      mech_cost_add(mech, &total, "Std Internals", (TONS * 400));
    /* End MECH Internal Structure Skeleton */

    /* Cockpit */
    if (TECHNOLOGY_SECONDARY & SMALLCOCKPIT_TECH)
      mech_cost_add(mech, &total, "Small Cockpit", 175000);
    else
      mech_cost_add(mech, &total, "Cockpit", 200000);

    /* Start MECH Life Support ( Tech Manual (p278) ) */
    mech_cost_add(mech, &total, "LifeSupport", 50000);
    /* End MECH Life Support */

    /* Sensors */
    /* TODO: Add variable range and multi-trac II */
    mech_cost_add(mech, &total, "Sensors", (TONS * 2000));

    /* Start MECH Musculatre (Myomer) ( Tech Manual (p278) )*/
    if (TECHNOLOGY & TRIPLE_MYOMER_TECH)
      mech_cost_add(mech, &total, "TS Myomer", (TONS * 16000));
    else
      mech_cost_add(mech, &total, "Myomer", (TONS * 2000));
    /* End MECH Musculatre (Myomer) */

    /* Actuators */
    mech_cost_add_arm_actuators(mech, LARM, &total);
    mech_cost_add_arm_actuators(mech, RARM, &total);

    mech_cost_add_leg_actuators(mech, LLEG, &total);
    mech_cost_add_leg_actuators(mech, RLEG, &total);

    /* Gyro */
    i = mech_engine_rating(mech);
    if (i % 100)
      i += (100 - (mech_engine_rating(mech) % 100));
    i /= 100;

    if (TECHNOLOGY_SECONDARY & XLGYRO_TECH)
      mech_cost_add(mech, &total, "XL Gyro", (i * 0.5 * 750000));
    else if (TECHNOLOGY_SECONDARY & CGYRO_TECH)
      mech_cost_add(mech, &total, "Compact Gyro", (i * 1.5 * 400000));
    else if (TECHNOLOGY_SECONDARY & HDGYRO_TECH)
      mech_cost_add(mech, &total, "HD Gyro", (i * 2 * 500000));
    else
      mech_cost_add(mech, &total, "Gyro", (i * 300000));

  } else if (UNIT_CLASS == CLASS_BSUIT) {
    /* ---------------------------------
     * BSuit Costs
     */
    if (TECHNOLOGY & CLAN_TECH) {
      mech_cost_add(mech, &total, "Clan Point", 3500000);
    } else {
      mech_cost_add(mech, &total, "IS Squad", 2400000);
    }

  } else {
    /* ---------------------------------
     * Vehicle Costs
     */
    int pamp = 0;
    int turret = 0;

    for (i = 0; i < NUM_SECTIONS; i++) {
      for (ii = 0; ii < NUM_CRITICALS; ii++) {
        part = mech_critical_part_type(mech, i, ii);
        if (!part)
          continue;
        if (!equipment_is_weapon(part))
          continue;
        if (i == TURRET)
          turret += crit_weight(mech, part);
        if (weapon_catalogue_is_energy(part)) {
          pamp += crit_weight(mech, part);
          btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG,
                             "PAmp Weight: %d", crit_weight(mech, part));
        }
      }
    }
    /*
     * Internals
     * 10,000 * Structure Tonnage
     */
    const double INTERNALS = (double)TONS * 1000.0;
    mech_cost_add(mech, &total, "Internals", INTERNALS);
    /*
     * Control Components
     * 10,000 * Control Tonnage
     * Control Tonnage = .05 * Tons
     */
    const double CONTROL_EQ = 10000.0 * 0.05 * (double)TONS;
    mech_cost_add(mech, &total, "Cockpit & Controls", CONTROL_EQ);
    /*
     * Power Amp
     * 20,000 * Amplifier Tonnage
     */
    if (TECHNOLOGY & ICE_TECH) {
      int power_amp = 20000 * (pamp / 1024) / 10;
      mech_cost_add(mech, &total, "Power Amplifiers", power_amp);
    }

    /*
     * Turret
     * Standard: 5,000 * Turret Tonnage
     */
    int turret_price = 5000 * (turret / 10) / 1024;
    mech_cost_add(mech, &total, "Turret", turret_price);
    /*
     * Lift/Dive Equip (Hovercraft, Hydrofoils, Submarines)
     * 20,000 * Equipment Tonnage
     */
    if (MOVEMENT == MOVE_HOVER || MOVEMENT == MOVE_FOIL ||
        MOVEMENT == MOVE_SUB) {
      const double LIFT_DIVE = 20000.0 * (0.1 * (double)TONS);
      mech_cost_add(mech, &total, "Lift/Dive Equip", LIFT_DIVE);
    }

    if (MOVEMENT == MOVE_VTOL) {
      const double VTOL_EQ = 40000.0 * (0.1 * (double)TONS);
      mech_cost_add(mech, &total, "Rotor", VTOL_EQ);
    }
  } // end if (Vehicle Calcs)

  /* ----------------------------
   * General Calculations
   */

  if (UNIT_CLASS != CLASS_BSUIT) {
    /* Engine Math
     * (Engine Basecost * Engine Rating * Tonnage) / 75
     */
    int engine_basecost = 5000;
    if (TECHNOLOGY & CE_TECH)
      engine_basecost = 10000;
    else if (TECHNOLOGY & LE_TECH)
      engine_basecost = 15000;
    else if (TECHNOLOGY & XL_TECH)
      engine_basecost = 20000;
    else if (TECHNOLOGY & XXL_TECH)
      engine_basecost = 100000;
    else if (TECHNOLOGY & ICE_TECH)
      engine_basecost = 1250;

    engine_size = mech_engine_rating(mech);

    if (MOVEMENT == MOVE_WHEEL || MOVEMENT == MOVE_FOIL ||
        MOVEMENT == MOVE_HOVER || MOVEMENT == MOVE_HULL ||
        MOVEMENT == MOVE_SUB || MOVEMENT == MOVE_VTOL) {
      engine_size = engine_size - susp_factor(mech);
    }
    /* Don't forget to Round up! */
    const unsigned long long ENGINE_NUMERATOR =
        (unsigned long long)engine_basecost * (unsigned long long)engine_size *
        (unsigned long long)TONS;
    const unsigned long long ENGINE_PRICE = (ENGINE_NUMERATOR + 74ULL) / 75ULL;
    mech_cost_add(mech, &total, "Engine", (double)ENGINE_PRICE);

    /* Jump Jets
     * Standard: Tonnage * (number of JJs^2) * 200
     * Improved: Tonnage * (number of JJs^2) * 500
     * Mechanical: Tonnage * (Jumping MP) * 150
     */
    const float NUM_JJS_FLOAT = mech_jump_speed(mech) * (float)MP_PER_KPH;
    const int NUM_JJS = (int)NUM_JJS_FLOAT;
    const double JJ_PRICE =
        (double)TONS * pow((double)NUM_JJS, 2.0) *
        (TECHNOLOGY_SECONDARY & IMPROVED_JJ_TECH ? 500.0 : 200.0);
    if (NUM_JJS > 0)
      mech_cost_add(mech, &total, "Jumpjets", JJ_PRICE);

    /*
       Heat Sinks
    */
    int numsinks = mech_heat_sink_count(mech);

    int sinkcost;
    if (TECHNOLOGY & DOUBLE_HEAT_TECH || TECHNOLOGY & CLAN_TECH)
      sinkcost = 6000;
    else if (TECHNOLOGY_SECONDARY & COMPACT_HS_TECH)
      sinkcost = 3000;
    else
      sinkcost = 2000;

    if ((TECHNOLOGY & DOUBLE_HEAT_TECH || TECHNOLOGY & CLAN_TECH)) {
      /* We want to divide the heat dissipation by two if DHS */
      numsinks = bounded(0, numsinks / 2, 500);
    }

    // For single heatsinks, we only charge for every heatsink over 10.
    if (TECHNOLOGY & DOUBLE_HEAT_TECH || TECHNOLOGY & CLAN_TECH ||
        TECHNOLOGY_SECONDARY & COMPACT_HS_TECH || TECHNOLOGY & ICE_TECH) {
      mech_cost_add(mech, &total, "Heat Sinks", (numsinks * sinkcost));
    } else {
      mech_cost_add(mech, &total, "Heat Sinks",
                    (bounded(0, numsinks - 10, 500) * sinkcost));
    }

    /* Armor */
    int total_armor = 0;
    int armor_section = 0;
    for (armor_section = 0; armor_section < NUM_SECTIONS; ++armor_section) {
      total_armor += mech_section_original_armor(mech, armor_section);
      total_armor += mech_section_original_rear_armor(mech, armor_section);
    }

    if (TECHNOLOGY & FF_TECH)
      total_armor = total_armor * 50 / ((TECHNOLOGY & CLAN_TECH) ? 60 : 56);
    else if (TECHNOLOGY_SECONDARY & HVY_FF_ARMOR_TECH)
      total_armor = total_armor * 50 / 62;
    else if (TECHNOLOGY_SECONDARY & LT_FF_ARMOR_TECH)
      total_armor = total_armor * 50 / 53;

    /* Come on. Really. We don't do .1 of armor. Round this !!! */
    const int ARMOR_WEIGHT = round_to_halfton(total_armor * 1024 / 16);
    const double ARMOR_TONS = (double)ARMOR_WEIGHT / 1024.0;

    int armor_cost_point = 10000;
    if (TECHNOLOGY & FF_TECH)
      armor_cost_point = 20000;
    else if (TECHNOLOGY_SECONDARY & STEALTH_ARMOR_TECH)
      armor_cost_point = 50000;
    else if (TECHNOLOGY & HARDA_TECH)
      armor_cost_point = 15000;
    else if (TECHNOLOGY_SECONDARY & LT_FF_ARMOR_TECH)
      armor_cost_point = 15000;
    else if (TECHNOLOGY_SECONDARY & HVY_FF_ARMOR_TECH)
      armor_cost_point = 25000;
    const double ARMOR_PRICE = ARMOR_TONS * (double)armor_cost_point;
    mech_cost_add(mech, &total, "Armor", ARMOR_PRICE);
  } // End Non-BSuit General Calculations

  /* Weapons. */
  /* While it might not make much sense to do this twice, we need to go through
   * all the sections */
  /* and handle weapons first, than we'll handle parts */

  for (i = 0; i < NUM_SECTIONS; i++) {
    count = find_weapons_advanced(mech, i, weaparray, weapdata, critical, 1);
    if (count <= 0)
      /* No weapons */
      continue;

    for (ii = 0; ii < count; ii++) {
      const int WEAPON_INDEX =
          *weapon_index_at(weaparray, MAX_WEAPS_SECTION, (size_t)ii);
      mech_cost_add(mech, &total, weapon_catalogue_name(WEAPON_INDEX),
                    weapon_catalogue_cost(WEAPON_INDEX));
    }
  }

  /* Ammo */

  ammoweapcount = find_ammunition(mech, ammoweap, ammo, ammomax, modearray, 0);

  if (ammoweapcount > 0) {
    if (mech_context(mech)->configuration->btech_cost_debug)
      btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG,
                         "Ammo Costs");
    for (i = 0; i < ammoweapcount; i++) {
      const int WEAPON_INDEX =
          *weapon_index_at(ammoweap, 8U * (size_t)MAX_WEAPS_SECTION, (size_t)i);
      const int MAXIMUM_AMMUNITION = *unsigned_short_at(
          ammomax, 8U * (size_t)MAX_WEAPS_SECTION, (size_t)i);
      const int AMMUNITION_PER_TON =
          weapon_catalogue_ammunition_per_ton(WEAPON_INDEX);
      const int AMMUNITION_COST =
          weapon_catalogue_ammunition_cost(WEAPON_INDEX);
      /* ArtemisIV ammo is X2 */
      /* Interesting way to handle half_tons */
      if (MAXIMUM_AMMUNITION < AMMUNITION_PER_TON) {
        const int HALF_TON_DIVISOR = AMMUNITION_PER_TON / MAXIMUM_AMMUNITION;
        const int ADJUSTED_AMMUNITION_COST = AMMUNITION_COST / HALF_TON_DIVISOR;
        mech_cost_add(mech, &total, weapon_catalogue_name(WEAPON_INDEX),
                      (double)ADJUSTED_AMMUNITION_COST);
      } else {
        const int AMMUNITION_TONS = MAXIMUM_AMMUNITION / AMMUNITION_PER_TON;
        const int ADJUSTED_AMMUNITION_COST =
            AMMUNITION_COST * AMMUNITION_TONS *
            ((*unsigned_int_at(modearray, 8U * (size_t)MAX_WEAPS_SECTION,
                               (size_t)i) &
              ARTEMIS_MODE)
                 ? 2
                 : 1);
        mech_cost_add(mech, &total, weapon_catalogue_name(WEAPON_INDEX),
                      (double)ADJUSTED_AMMUNITION_COST);
      }
    }
  }

  /* Parts */

  int masc_count = 0;
  int bloodhound_count = 0;
  for (i = 0; i < NUM_SECTIONS; i++) {
    for (ii = 0; ii < NUM_CRITICALS; ii++) {
      part = mech_critical_part_type(mech, i, ii);
      if (equipment_is_actuator(part) || part == EMPTY)
        continue;
      if (equipment_is_special(part)) {
        /* These parts are handled above, don't count their crits */
        switch (special_from_equipment_index(part)) {
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
          mech_cost_add(mech, &total, "Int Case", 50000);
          continue;
        case CASE_II:
          mech_cost_add(mech, &total, "Int CaseII", 175000);
          continue;
        case AXE:
          mech_cost_add(mech, &total, "Int Axe", 5000);
          continue;
        case BEAGLE_PROBE:
          mech_cost_add(mech, &total, "BAP", 100000);
          continue;
        case LIGHT_BAP:
          mech_cost_add(mech, &total, "LightBAP", 50000);
          continue;
        case BLOODHOUND_PROBE:
          bloodhound_count++;
          continue;
        case ARTEMIS_IV:
          mech_cost_add(mech, &total, "ArtemisIV FCS", 100000);
          continue;
        case ANGELECM:
          mech_cost_add(mech, &total, "Angel ECM", 375000);
          continue;
        case C3_MASTER:
          mech_cost_add(mech, &total, "C3M", 300000);
          continue;
        case C3_SLAVE:
          mech_cost_add(mech, &total, "C3S", 250000);
          continue;
        case C3I:
          mech_cost_add(mech, &total, "C3I", 375000);
          continue;
        case ECM:
          mech_cost_add(mech, &total, "ECM", 100000);
          continue;
        case TAG:
          mech_cost_add(mech, &total, "TAG", 50000);
          continue;
        case TARGETING_COMPUTER:
          mech_cost_add(mech, &total, "TargComp", 10000);
          continue;
        case SPLIT_CRIT_LEFT:
        case SPLIT_CRIT_RIGHT:
        case HARDPOINT:
          continue;

        default:
          break;
        }
      }
      if (equipment_is_ammunition(part)) {
        /* Need Something in here to do CASE for CLAN mechs */
        if ((TECHNOLOGY & CLAN_TECH))
          *mutable_bool_at(clan_case_sections, NUM_SECTIONS, (size_t)i) = true;
        continue;
      }
      if (equipment_is_weapon(part))
        continue;

      const unsigned long long indiv_part_cost =
          btech_part_cost_get(mech_context(mech), part);
      mech_cost_add(mech, &total, part_name(mech_context(mech), part, 0).text,
                    (double)indiv_part_cost);
    }
  }
  /* We have to account for some other stuff that doesn't divide equally here */
  const int BLOODHOUND_PACKAGES = bloodhound_count / 3;
  if (BLOODHOUND_PACKAGES)
    mech_cost_add(mech, &total, "Bloodhound",
                  (double)(500000 * BLOODHOUND_PACKAGES));
  if (masc_count)
    mech_cost_add(mech, &total, "MASC", masc_count * engine_size * 1000);
  if (has_sword) {
    /* Sword Cost is Tonnage of sword * 10000. Sword Tonnage is 1/20th of Mech
     * Tonnage, rounded up to nearest halfton */
    const int SWORD_WEIGHT = round_to_halfton(TONS * 1024 / 20);
    const double SWORD_TONS = (double)SWORD_WEIGHT / 1024.0;
    mech_cost_add(mech, &total, "Sword", SWORD_TONS * 10000.0);
  }

  /* Clan Case */
  if ((TECHNOLOGY & CLAN_TECH)) {
    for (i = 0; i < NUM_SECTIONS; i++) {
      if (*bool_at(clan_case_sections, NUM_SECTIONS, (size_t)i))
        mech_cost_add(mech, &total, "Clan CASE Section", 50000);
    }
  }

  if (UNIT_CLASS != CLASS_MECH && UNIT_CLASS != CLASS_BSUIT) {
    switch (MOVEMENT) {
    case MOVE_TRACK:
      mod = 1.0 + ((double)TONS / 100.0);
      break;
    case MOVE_WHEEL:
      mod = 1.0 + ((double)TONS / 200.0);
      break;
    case MOVE_HOVER:
      mod = 1.0 + ((double)TONS / 50.0);
      break;
    case MOVE_VTOL:
      mod = 1.0 + ((double)TONS / 30.0);
      break;
    case MOVE_HULL:
      mod = 1.0 + ((double)TONS / 200.0);
      break;
    case MOVE_FOIL:
      mod = 1.0 + ((double)TONS / 75.0);
      break;
    case MOVE_SUB:
      mod = 1.0 + ((double)TONS / 50.0);
      break;
    }
  } else if (UNIT_CLASS == CLASS_BSUIT) {
    // There's nothing in Maxtech about this, but we're going to knock the
    // prices down to be competitive with other unit types.
    mod = 0.75;
  } else {
    // The standard mech size cost modifier. 20 ton mech, for example is Cost *
    // .20
    mod = 1.0 + ((double)TONS / 100.0);
  }

  if (mech_is_omni(mech)) {
    mech_cost_add(mech, &total, "OmniMech", total * 0.25);
  }

  return (unsigned long long)(total * mod);
} /* End Function */
