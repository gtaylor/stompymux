#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_status_types.h"
#include "mech_utils_internal.h"

static float walking_speed(float maximum_speed) {
  return 2.0F * maximum_speed / 3.0F;
}

#ifdef BT_CALCULATE_BV
int CalculateBV(Mech *mech, int gunstat, int pilstat) {
  int defbv = 0, offbv = 0, i, ii, temp, temp2, deduct = 0, offweapbv = 0,
      defweapbv = 0, armor = 0, intern = 0, weapindx, mostheat = 0,
      tempheat = 0, mechspec, mechspec2, type, move, pilskl = pilstat,
      gunskl = gunstat;
  int debug1 = 0, debug2 = 0, debug3 = 0, debug4 = 0;
  float maxspeed, mul = 1.00;

  if (!mech)
    return 0;

  if (gunstat == 100 || pilstat == 100) {
    if (mech->xcode.context->events->tick - ((mech)->ud.mechbv_last) < 30)
      return ((mech)->ud.mechbv);
    else
      ((mech)->ud.mechbv_last) = mech->xcode.context->events->tick;
  }

  type = ((mech)->ud.type);
  move = ((mech)->ud.move);
  mechspec = ((mech)->rd.specials);
  mechspec2 = ((mech)->rd.specials2);
  if (gunstat == 100)
    pilskl = FindPilotPiloting(mech);
  if (pilstat == 100)
    gunskl = FindAverageGunnery(mech);

  for (i = 0; i < NUM_SECTIONS; i++) {
    armor += (debug1 = mech_section_armor(mech, i) *
                       (mechspec & HARDA_TECH ? 200 : 100));
    if (type == CLASS_MECH && (i == CTORSO || i == LTORSO || i == RTORSO)) {
      armor += (debug2 = mech_section_rear_armor(mech, i) *
                         (mechspec & HARDA_TECH ? 200 : 100));
    }
    if (!mech_is_aerospace_unit(mech))
      intern += (debug3 = mech_section_internal(mech, i) *
                          (mechspec & COMPI_TECH    ? 50
                           : mechspec & REINFI_TECH ? 200
                                                    : 100));
    else
      intern = (debug3 = ((mech)->ud.si));
#ifdef DEBUG_BV
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("Armoradd : %d ArmorRadd : %d Internadd : %d",
                               debug1 / 100, debug2 / 100, debug3 / 100));
//				if(mechspec2 & TORSOCOCKPIT_TECH && i == CTORSO)
//					btech_channel_send(mech->xcode.context,
// BTECH_CHANNEL_MECH_DEBUG, // tprintf("TorsoCockpit Armoradd
//: %d", debug4));
#endif

    debug1 = debug2 = debug3 = debug4 = 0;
    for (ii = 0; ii < CritsInLoc(mech, i); ii++) {
      if (equipment_is_weapon(temp = mech_critical_part_type(mech, i, ii))) {
        weapindx = (weapon_from_equipment_index(temp));
        if (mech_critical_is_nonfunctional(mech, i, ii)) {
          if (type == CLASS_MECH)
            ii += (MechWeapons[weapindx].criticals - 1);
          continue;
        }
        if (MechWeapons[weapindx].special & AMS) {
          defweapbv +=
              (debug1 =
                   (mech_weapon_battle_value(mech, weapindx) * 100) *
                   (float)(3000 /
                           (mech_weapon_recycle_time(mech, weapindx) * 100)));

#ifdef DEBUG_BV
          btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                             "%s",
                             tprintf("DefWeapBVadd (%s) : %d - Total : %d",
                                     MechWeapons[weapindx].name, debug1 / 100,
                                     defweapbv / 100));
#endif

        } else {
          offweapbv +=
              (debug1 =
                   (mech_weapon_battle_value(mech, weapindx) *
                    (mech_critical_fire_mode(mech, i, ii) & REAR_MOUNT ? 50
                                                                       : 100)) *
                   (float)((float)3000 /
                           (float)(mech_weapon_recycle_time(mech, weapindx) *
                                   100)));
          if (MechWeapons[weapindx].type == TMISSILE)
            if (FindArtemisForWeapon(mech, i, ii))
              offweapbv += (mech_weapon_battle_value(mech, weapindx) * 20);
#ifdef DEBUG_BV
          btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                             "%s",
                             tprintf("OffWeapBVadd (%s) : %d - Total : %d",
                                     MechWeapons[weapindx].name, debug1 / 100,
                                     offweapbv / 100));
#endif
        }
        if (type == CLASS_MECH) {
          if (!(mech_critical_fire_mode(mech, i, ii) & REAR_MOUNT)) {
            tempheat =
                ((MechWeapons[weapindx].heat * 100) *
                 (float)((float)3000 /
                         (float)(mech_weapon_recycle_time(mech, weapindx) *
                                 100)));
            if (MechWeapons[weapindx].special & ULTRA)
              tempheat = (tempheat * 2);
            if (MechWeapons[weapindx].special & STREAK)
              tempheat = (tempheat / 2);
            mostheat += tempheat;
#ifdef DEBUG_BV
            btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                               "%s",
                               tprintf("Tempheatadded (%s) : %d - Total : %d",
                                       MechWeapons[weapindx].name,
                                       tempheat / 100, mostheat / 100));
#endif
            tempheat = 0;
          }
        }
        if (type == CLASS_MECH)
          ii += (MechWeapons[weapindx].criticals - 1);
      } else if (equipment_is_ammunition(temp)) {
        if (mech_critical_is_nonfunctional(mech, i, ii) ||
            !mech_critical_data(mech, i, ii))
          continue;
        mul = ((temp2 = mech_critical_ammo_mode(mech, i, ii)) & AC_AP_MODE ? 4
               : temp2 & AC_PRECISION_MODE                                 ? 6
               : temp2 & (SWARM_MODE | SWARM1_MODE | STINGER_MODE)         ? 1.5
                                                                           : 1);
        mul = (mul *
               ((float)((float)mech_critical_data(mech, i, ii) /
                        (float)MechWeapons[weapindx =
                                               ammunition_to_weapon_index(temp)]
                            .ammoperton)));

#ifdef DEBUG_BV
        btech_channel_send(
            mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
            tprintf("AmmoBVmul (%s) : %.2f", MechWeapons[weapindx].name, mul));
#endif

        if (MechWeapons[weapindx].special & AMS) {
          defweapbv +=
              (debug1 =
                   (((mech_weapon_battle_value(mech, weapindx) / 10) * 100) *
                    mul) *
                   (float)((float)3000 /
                           (float)(mech_weapon_recycle_time(mech, weapindx) *
                                   100)));

#ifdef DEBUG_BV
          btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                             "%s",
                             tprintf("AmmoDefWeapBVadd (%s) : %d - Total : %d",
                                     MechWeapons[weapindx].name, debug1 / 100,
                                     defweapbv / 100));
#endif

        } else {

#ifdef DEBUG_BV
          btech_channel_send(
              mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
              tprintf("Abattlebalue (%s) : %d", MechWeapons[weapindx].name,
                      (mech_weapon_battle_value(mech, weapindx) / 10)));
#endif

          offweapbv +=
              (debug1 =
                   (((mech_weapon_battle_value(mech, weapindx) / 10) * 100) *
                    mul) *
                   (float)((float)3000 /
                           (float)(mech_weapon_recycle_time(mech, weapindx) *
                                   100)));

#ifdef DEBUG_BV
          btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                             "%s",
                             tprintf("AmmoOffWeapBVadd (%s)  : %d - Total : %d",
                                     MechWeapons[weapindx].name, debug1 / 100,
                                     offweapbv / 100));
#endif
        }
      }
      if ((equipment_is_ammunition(temp) ||
           (equipment_is_weapon(temp) &&
            MechWeapons[(weapon_from_equipment_index(temp))].special &
                GAUSS)) &&
          type == CLASS_MECH) {
        if (mechspec & CLAN_TECH)
          if (i == CTORSO || i == HEAD || i == RLEG || i == LLEG) {

#ifdef DEBUG_BV
            btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                               "20 deduct added for ammo");
#endif
            deduct += 2000;
            continue;
          }
        if (mechspec & (XL_TECH | XXL_TECH | ICE_TECH | LE_TECH)) {

#ifdef DEBUG_BV
          btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                             "20/2000 deduct added for ammo");
#endif

          deduct += 2000;
          continue;
        }
        if ((i == CTORSO || i == RLEG || i == LLEG || i == HEAD) &&
            !(((mech)->ud.sections)[i].config & CASE_TECH)) {

#ifdef DEBUG_BV
          btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                             "20 deduct added for ammo");
#endif

          deduct += 2000;
          continue;
        }
        if ((i == RARM || i == LARM) &&
            (!(((mech)->ud.sections)[i].config & CASE_TECH) &&
             !(((mech)->ud.sections)[(i == RARM ? RTORSO : LTORSO)].config &
               CASE_TECH))) {

#ifdef DEBUG_BV
          btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                             "20 deduct added for ammo");
#endif

          deduct += 2000;
          continue;
        }
      }
    }
  }
  if (type == CLASS_MECH) {
    mostheat += (((mech)->rd.jumpspeed) > 0
                     ? MAX((((mech)->rd.jumpspeed) / MP1) * 100, 300)
                     : 200);
    if (mechspec2 & (NULLSIGSYS_TECH | STEALTH_ARMOR_TECH))
      mostheat += 1000;
    if ((temp = (mostheat - (mech_active_heat_sinks(mech) * 100))) > 0) {
      deduct += temp * 5;
#ifdef DEBUG_BV
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                         tprintf("Deduct add for heat : %d", (temp * 5) / 100));
#endif
    }
  }
#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("DeductTotal : %d", deduct / 100));
#endif

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
      mul = 1.125;
    else
      mul = 0.75;
  } else if (mechspec & ICE_TECH || ((mech)->ud.type) == CLASS_VEH_GROUND ||
             ((mech)->ud.type) == CLASS_VEH_NAVAL) {
    mul = 0.5;
  } else {
    mul = 1.5;
  }

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("InternMul : %.2f", mul));
#endif

  armor = (armor * (((mech)->ud.type) == CLASS_MECH ? 2 : 1));
  intern = intern * mul;
  mul = 1.00;

#ifdef DEBUG_BV
  btech_channel_send(
      mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
      tprintf("ArmorEnd : %d IntEnd : %d", armor / 100, intern / 100));
#endif

  maxspeed = mech_effective_maximum_speed(mech);
  if (mechspec & MASC_TECH || mechspec2 & SUPERCHARGER_TECH) {
    if (mechspec & MASC_TECH && mechspec2 & SUPERCHARGER_TECH)
      maxspeed = maxspeed * 2.5;
    else
      maxspeed = maxspeed * 1.5;
  }
  if (mechspec & TRIPLE_MYOMER_TECH)
    maxspeed = ((walking_speed(maxspeed) + MP1) * 1.5);

  if (maxspeed <= MP2) {
    mul = 1.0;
  } else if (maxspeed <= MP4) {
    mul = 1.1;
  } else if (maxspeed <= MP6) {
    mul = 1.2;
  } else if (maxspeed <= MP9) {
    mul = 1.3;
  } else if (maxspeed <= MP1 * 13) {
    mul = 1.4;
  } else if (maxspeed <= MP1 * 18) {
    mul = 1.5;
  } else if (maxspeed <= MP1 * 24) {
    mul = 1.6;
  } else {
    mul = 1.7;
  }

  if (mech_is_dropship(mech))
    mul = 1.0;
  else if (mech_is_aerospace_unit(mech))
    mul = 1.1;

  if (mechspec2 & (NULLSIGSYS_TECH | STEALTH_ARMOR_TECH))
    mul += 1.5;
  if (((mech)->rd.infantry_specials) & DC_KAGE_STEALTH_TECH)
    mul += .75;
  if (((mech)->rd.infantry_specials) & FWL_ACHILEUS_STEALTH_TECH)
    mul += 1.5;
  if (((mech)->rd.infantry_specials) & CS_PURIFIER_STEALTH_TECH)
    mul += 2.0;
  if (((mech)->rd.infantry_specials) & FC_INFILTRATOR_STEALTH_TECH)
    mul += .75;
  if (((mech)->rd.infantry_specials) & FC_INFILTRATOR_STEALTH_TECH)
    mul += 2.0;

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("DefBVMul : %.2f", mul));
#endif

  defbv = (armor + intern + (((mech)->ud.tons) * 100) + defweapbv);

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("DefBV Tonnage added : %d", ((mech)->ud.tons)));
#endif

  if ((defbv - deduct) < 1)
    defbv = 1;
  else
    defbv -= deduct;
  if (type != CLASS_MECH)
    defbv =
        ((defbv * (move == MOVE_TRACK   ? 0.8
                   : move == MOVE_WHEEL ? 0.7
                   : move == MOVE_HOVER ? 0.6
                   : move == MOVE_VTOL  ? 0.4
                   : move == MOVE_FOIL || move == MOVE_SUB || move == MOVE_HULL
                       ? 0.5
                       : 1.0)) -
         deduct);
  defbv = defbv * mul;

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("DefBV : %d", defbv / 100));
#endif

  if ((type == CLASS_MECH || mech_is_aerospace_unit(mech)) &&
      mostheat > (mech_active_heat_sinks(mech) * 100)) {
#ifdef DEBUG_BV
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("Pre-Heat OffWeapBV : %d", offweapbv / 100));
#endif
    i = (((mech_active_heat_sinks(mech) / 100) * offweapbv) / mostheat);
    ii = ((offweapbv - i) / 2);
    offweapbv = i + ii;

#ifdef DEBUG_BV
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("Post-Heat OffWeapBV : %d", offweapbv / 100));
#endif
  }
  /*
  mul = pow(((((mech_effective_maximum_speed(mech) / MP1) + (type == CLASS_AERO
  || type == CLASS_DS ? 0 : (((mech)->rd.jumpspeed) / MP1)) + (mechspec &
  MASC_TECH ? 1 : 0) + (mechspec & TRIPLE_MYOMER_TECH ? 1 : 0)+ (mechspec2 &
  SUPERCHARGER_TECH ? 1 : 0) - 5) / 10) + 1), 1.2);
  */
  mul = pow((((((mech_is_dropship(mech)
                     ? walking_speed(mech_effective_maximum_speed(mech))
                     : mech_effective_maximum_speed(mech)) /
                MP1) +
               (mechspec & MASC_TECH ? 1 : 0) +
               (mechspec & TRIPLE_MYOMER_TECH ? 1 : 0) +
               (mechspec2 & SUPERCHARGER_TECH ? 1 : 0) - 5) /
              10) +
             1),
            1.2);

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("DumbMul : %.2f", mul));
#endif

  if (mechspec2 & OMNIMECH_TECH)
    mul += .3;

  offweapbv = offweapbv * mul;
  if (type != CLASS_AERO && type != CLASS_DS && ((mech)->rd.jumpspeed) > 0)
    offweapbv +=
        ((((mech)->rd.jumpspeed) / MP1) * (100 * (((mech)->ud.tons) / 5)));
  offbv = offweapbv;

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("OffWeapBVAfter : %d", offweapbv / 100));
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("DefBV : %d OffBV : %d TotalBV : %d", defbv / 100,
                             offbv / 100, (offbv + defbv) / 100));
#endif

  mul = (skillmul[battle_value_skill_index(gunskl)]
                 [battle_value_skill_index(pilskl)]);

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("SkillMul : %.2f (%d/%d)", mul, gunskl, pilskl));
#endif
  return ((offbv + defbv) / 100) * mul;
}
#endif
