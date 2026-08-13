#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_move_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mech_utils_internal.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include <math.h>
#include <stddef.h>

static float walking_speed(float maximum_speed) {
  return 2.0F * maximum_speed / 3.0F;
}

#ifdef BT_CALCULATE_BV
static float battle_value_skill_multiplier(int gunnery, int piloting) {
  const int GUN_INDEX = battle_value_skill_index(gunnery);
  const int PILOT_INDEX = battle_value_skill_index(piloting);
  const float (*row)[BTECH_BV_SKILL_LIMIT] = checked_storage_at_const(
      skillmul, BTECH_BV_SKILL_LIMIT, sizeof(*skillmul), (size_t)GUN_INDEX);
  return *(const float *)checked_storage_at_const(
      *row, BTECH_BV_SKILL_LIMIT, sizeof(**row), (size_t)PILOT_INDEX);
}

int calculate_bv(Mech *mech, int gunstat, int pilstat) {
  int defbv = 0;
  int offbv = 0;
  int i;
  int ii;
  int temp;
  int temp2;
  int deduct = 0;
  int offweapbv = 0;
  int defweapbv = 0;
  int armor = 0;
  int intern = 0;
  int weapindx;
  int mostheat = 0;
  int tempheat = 0;
  int mechspec;
  int mechspec2;
  int type;
  int move;
  int pilskl = pilstat;
  int gunskl = gunstat;
  float maxspeed;
  float mul = 1.00F;

  if (!mech)
    return 0;

  if (gunstat == 100 || pilstat == 100) {
    if (mech->xcode.context->events->tick - ((mech)->ud.mechbv_last) < 30)
      return ((mech)->ud.mechbv);
    ((mech)->ud.mechbv_last) = mech->xcode.context->events->tick;
  }

  type = (unsigned char)((mech)->ud.type);
  move = (unsigned char)((mech)->ud.move);
  mechspec = ((mech)->rd.specials);
  mechspec2 = ((mech)->rd.specials2);
  if (gunstat == 100)
    pilskl = find_pilot_piloting(mech);
  if (pilstat == 100)
    gunskl = find_average_gunnery(mech);

  for (i = 0; i < NUM_SECTIONS; i++) {
    armor += mech_section_armor(mech, i) * (mechspec & HARDA_TECH ? 200 : 100);
    if (type == CLASS_MECH && (i == CTORSO || i == LTORSO || i == RTORSO)) {
      armor += mech_section_rear_armor(mech, i) *
               (mechspec & HARDA_TECH ? 200 : 100);
    }
    if (!mech_is_aerospace_unit(mech)) {
      int internal_multiplier = 100;
      if (mechspec & COMPI_TECH)
        internal_multiplier = 50;
      else if (mechspec & REINFI_TECH)
        internal_multiplier = 200;
      intern += mech_section_internal(mech, i) * internal_multiplier;
    } else {
      intern = (unsigned char)((mech)->ud.si);
    }

    for (ii = 0; ii < crits_in_loc(mech, i); ii++) {
      temp = mech_critical_part_type(mech, i, ii);
      if (equipment_is_weapon(temp)) {
        weapindx = (weapon_from_equipment_index(temp));
        if (mech_critical_is_nonfunctional(mech, i, ii)) {
          if (type == CLASS_MECH)
            ii += (weapon_catalogue_critical_slots(weapindx) - 1);
          continue;
        }
        if (weapon_catalogue_has_special(weapindx, AMS)) {
          const int WEAPON_BV = mech_weapon_battle_value(mech, weapindx);
          const int RECYCLE_TIME = mech_weapon_recycle_time(mech, weapindx);
          const float RECYCLE_MULTIPLIER =
              3000.0F / (float)(RECYCLE_TIME * 100);

          defweapbv +=
              clamp_float_to_int((float)(WEAPON_BV * 100) * RECYCLE_MULTIPLIER);
        } else {
          const int WEAPON_BV = mech_weapon_battle_value(mech, weapindx);
          const int RECYCLE_TIME = mech_weapon_recycle_time(mech, weapindx);
          const int REAR_MULTIPLIER =
              mech_critical_fire_mode(mech, i, ii) & REAR_MOUNT ? 50 : 100;
          const float RECYCLE_MULTIPLIER =
              3000.0F / (float)(RECYCLE_TIME * 100);

          offweapbv += clamp_float_to_int((float)(WEAPON_BV * REAR_MULTIPLIER) *
                                          RECYCLE_MULTIPLIER);
          if (weapon_catalogue_type(weapindx) == TMISSILE)
            if (find_artemis_for_weapon(mech, i, ii))
              offweapbv += (mech_weapon_battle_value(mech, weapindx) * 20);
        }
        if (type == CLASS_MECH) {
          if (!(mech_critical_fire_mode(mech, i, ii) & REAR_MOUNT)) {
            const int RECYCLE_TIME = mech_weapon_recycle_time(mech, weapindx);
            const float RECYCLE_MULTIPLIER =
                3000.0F / (float)(RECYCLE_TIME * 100);
            tempheat = clamp_float_to_int(
                (float)(weapon_catalogue_heat(weapindx) * 100) *
                RECYCLE_MULTIPLIER);
            if (weapon_catalogue_has_special(weapindx, ULTRA))
              tempheat = (tempheat * 2);
            if (weapon_catalogue_has_special(weapindx, STREAK))
              tempheat = (tempheat / 2);
            mostheat += tempheat;
          }
        }
        if (type == CLASS_MECH)
          ii += (weapon_catalogue_critical_slots(weapindx) - 1);
      } else if (equipment_is_ammunition(temp)) {
        if (mech_critical_is_nonfunctional(mech, i, ii) ||
            !mech_critical_data(mech, i, ii))
          continue;
        temp2 = mech_critical_ammo_mode(mech, i, ii);
        mul = 1.0F;
        if (temp2 & AC_AP_MODE)
          mul = 4.0F;
        else if (temp2 & AC_PRECISION_MODE)
          mul = 6.0F;
        else if (temp2 & (SWARM_MODE | SWARM1_MODE | STINGER_MODE))
          mul = 1.5F;
        const int AMMUNITION = mech_critical_data(mech, i, ii);
        weapindx = ammunition_to_weapon_index(temp);
        const int AMMUNITION_PER_TON =
            weapon_catalogue_ammunition_per_ton(weapindx);
        mul *= (float)AMMUNITION / (float)AMMUNITION_PER_TON;

        if (weapon_catalogue_has_special(weapindx, AMS)) {
          const int WEAPON_BV = mech_weapon_battle_value(mech, weapindx);
          const int RECYCLE_TIME = mech_weapon_recycle_time(mech, weapindx);
          const float RECYCLE_MULTIPLIER =
              3000.0F / (float)(RECYCLE_TIME * 100);
          const int SCALED_WEAPON_BV = (WEAPON_BV / 10) * 100;

          defweapbv += clamp_float_to_int((float)SCALED_WEAPON_BV * mul *
                                          RECYCLE_MULTIPLIER);
        } else {
          const int WEAPON_BV = mech_weapon_battle_value(mech, weapindx);
          const int RECYCLE_TIME = mech_weapon_recycle_time(mech, weapindx);
          const float RECYCLE_MULTIPLIER =
              3000.0F / (float)(RECYCLE_TIME * 100);
          const int SCALED_WEAPON_BV = (WEAPON_BV / 10) * 100;

          offweapbv += clamp_float_to_int((float)SCALED_WEAPON_BV * mul *
                                          RECYCLE_MULTIPLIER);
        }
      }
      if ((equipment_is_ammunition(temp) ||
           (equipment_is_weapon(temp) &&
            weapon_catalogue_has_special(weapon_from_equipment_index(temp),
                                         GAUSS))) &&
          type == CLASS_MECH) {
        if (mechspec & CLAN_TECH) {
          if (i == CTORSO || i == HEAD || i == RLEG || i == LLEG) {

            deduct += 2000;
            continue;
          }
        }
        if (mechspec & (XL_TECH | XXL_TECH | ICE_TECH | LE_TECH)) {

          deduct += 2000;
          continue;
        }
        if ((i == CTORSO || i == RLEG || i == LLEG || i == HEAD) &&
            !(mech_section_configuration(mech, i) & CASE_TECH)) {

          deduct += 2000;
          continue;
        }
        if ((i == RARM || i == LARM) &&
            (!(mech_section_configuration(mech, i) & CASE_TECH) &&
             !(mech_section_configuration(mech, i == RARM ? RTORSO : LTORSO) &
               CASE_TECH))) {

          deduct += 2000;
          continue;
        }
      }
    }
  }
  if (type == CLASS_MECH) {
    mostheat +=
        ((mech)->rd.jumpspeed > 0
             ? max(clamp_float_to_int(((mech)->rd.jumpspeed / MP1) * 100.0F),
                   300)
             : 200);
    if (mechspec2 & (NULLSIGSYS_TECH | STEALTH_ARMOR_TECH))
      mostheat += 1000;
    const int ACTIVE_HEAT_SINKS =
        clamp_float_to_int(mech_active_heat_sinks(mech));
    temp = mostheat - (ACTIVE_HEAT_SINKS * 100);
    if (temp > 0) {
      deduct += temp * 5;
    }
  }

  if (mechspec & ECM_TECH)
    defweapbv += 6100;

  if (mechspec & BEAGLE_PROBE_TECH) {
    if (mechspec & CLAN_TECH)
      offweapbv += 1200;
    else
      offweapbv += 1000;
  }
  if (mechspec & (XL_TECH | XXL_TECH | LE_TECH)) {
    if (mechspec & (CLAN_TECH | LE_TECH))
      mul = 1.125F;
    else
      mul = 0.75F;
  } else if (mechspec & ICE_TECH || ((mech)->ud.type) == CLASS_VEH_GROUND ||
             ((mech)->ud.type) == CLASS_VEH_NAVAL) {
    mul = 0.5F;
  } else {
    mul = 1.5F;
  }

  armor = (armor * (((mech)->ud.type) == CLASS_MECH ? 2 : 1));
  intern = clamp_float_to_int((float)intern * mul);

  maxspeed = mech_effective_maximum_speed(mech);
  if (mechspec & MASC_TECH || mechspec2 & SUPERCHARGER_TECH) {
    if (mechspec & MASC_TECH && mechspec2 & SUPERCHARGER_TECH)
      maxspeed *= 2.5F;
    else
      maxspeed *= 1.5F;
  }
  if (mechspec & TRIPLE_MYOMER_TECH)
    maxspeed = (walking_speed(maxspeed) + MP1) * 1.5F;

  if (maxspeed <= MP2) {
    mul = 1.0F;
  } else if (maxspeed <= MP4) {
    mul = 1.1F;
  } else if (maxspeed <= MP6) {
    mul = 1.2F;
  } else if (maxspeed <= MP9) {
    mul = 1.3F;
  } else if (maxspeed <= MP1 * 13) {
    mul = 1.4F;
  } else if (maxspeed <= MP1 * 18) {
    mul = 1.5F;
  } else if (maxspeed <= MP1 * 24) {
    mul = 1.6F;
  } else {
    mul = 1.7F;
  }

  if (mech_is_dropship(mech))
    mul = 1.0F;
  else if (mech_is_aerospace_unit(mech))
    mul = 1.1F;

  if (mechspec2 & (NULLSIGSYS_TECH | STEALTH_ARMOR_TECH))
    mul += 1.5F;
  if (((mech)->rd.infantry_specials) & DC_KAGE_STEALTH_TECH)
    mul += 0.75F;
  if (((mech)->rd.infantry_specials) & FWL_ACHILEUS_STEALTH_TECH)
    mul += 1.5F;
  if (((mech)->rd.infantry_specials) & CS_PURIFIER_STEALTH_TECH)
    mul += 2.0F;
  if (((mech)->rd.infantry_specials) & FC_INFILTRATOR_STEALTH_TECH)
    mul += 0.75F;
  if (((mech)->rd.infantry_specials) & FC_INFILTRATOR_STEALTH_TECH)
    mul += 2.0F;

  defbv = (armor + intern + (((mech)->ud.tons) * 100) + defweapbv);

  if ((defbv - deduct) < 1)
    defbv = 1;
  else
    defbv -= deduct;
  if (type != CLASS_MECH) {
    float movement_modifier = 1.0F;
    if (move == MOVE_TRACK)
      movement_modifier = 0.8F;
    else if (move == MOVE_WHEEL)
      movement_modifier = 0.7F;
    else if (move == MOVE_HOVER)
      movement_modifier = 0.6F;
    else if (move == MOVE_VTOL)
      movement_modifier = 0.4F;
    else if (move == MOVE_FOIL || move == MOVE_SUB || move == MOVE_HULL)
      movement_modifier = 0.5F;
    defbv =
        clamp_float_to_int(((float)defbv * movement_modifier) - (float)deduct);
  }
  defbv = clamp_float_to_int((float)defbv * mul);

  const int ACTIVE_HEAT_SINKS =
      clamp_float_to_int(mech_active_heat_sinks(mech));
  if ((type == CLASS_MECH || mech_is_aerospace_unit(mech)) &&
      mostheat > ACTIVE_HEAT_SINKS * 100) {
    i = ((ACTIVE_HEAT_SINKS / 100) * offweapbv) / mostheat;
    ii = ((offweapbv - i) / 2);
    offweapbv = i + ii;
  }
  /*
  mul = pow(((((mech_effective_maximum_speed(mech) / MP1) + (type == CLASS_AERO
  || type == CLASS_DS ? 0 : (((mech)->rd.jumpspeed) / MP1)) + (mechspec &
  MASC_TECH ? 1 : 0) + (mechspec & TRIPLE_MYOMER_TECH ? 1 : 0)+ (mechspec2 &
  SUPERCHARGER_TECH ? 1 : 0) - 5) / 10) + 1), 1.2);
  */
  const float EFFECTIVE_SPEED = mech_effective_maximum_speed(mech);
  const float SPEED_FOR_BV =
      mech_is_dropship(mech) ? walking_speed(EFFECTIVE_SPEED) : EFFECTIVE_SPEED;
  const float SPEED_FACTOR =
      (((SPEED_FOR_BV / MP1) + (mechspec & MASC_TECH ? 1.0F : 0.0F) +
        (mechspec & TRIPLE_MYOMER_TECH ? 1.0F : 0.0F) +
        (mechspec2 & SUPERCHARGER_TECH ? 1.0F : 0.0F) - 5.0F) /
       10.0F) +
      1.0F;
  mul = powf(SPEED_FACTOR, 1.2F);

  if (mechspec2 & OMNIMECH_TECH)
    mul += 0.3F;

  offweapbv = clamp_float_to_int((float)offweapbv * mul);
  if (type != CLASS_AERO && type != CLASS_DS && ((mech)->rd.jumpspeed) > 0) {
    const int TONNAGE_FACTOR = 100 * ((mech)->ud.tons / 5);
    offweapbv += clamp_float_to_int(((mech)->rd.jumpspeed / MP1) *
                                    (float)TONNAGE_FACTOR);
  }
  offbv = offweapbv;

  mul = battle_value_skill_multiplier(gunskl, pilskl);

  const int BASE_BATTLE_VALUE = (offbv + defbv) / 100;
  return clamp_float_to_int((float)BASE_BATTLE_VALUE * mul);
}
#endif
