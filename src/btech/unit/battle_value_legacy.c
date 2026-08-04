#include "mech_utils_internal.h"

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
    if (mech->xcode.context->events->tick - MechBVLast(mech) < 30)
      return MechBV(mech);
    else
      MechBVLast(mech) = mech->xcode.context->events->tick;
  }

  type = MechType(mech);
  move = MechMove(mech);
  mechspec = MechSpecials(mech);
  mechspec2 = MechSpecials2(mech);
  if (gunstat == 100)
    pilskl = FindPilotPiloting(mech);
  if (pilstat == 100)
    gunskl = FindAverageGunnery(mech);

  for (i = 0; i < NUM_SECTIONS; i++) {
    armor +=
        (debug1 = GetSectArmor(mech, i) * (mechspec & HARDA_TECH ? 200 : 100));
    if (type == CLASS_MECH && (i == CTORSO || i == LTORSO || i == RTORSO)) {
      armor += (debug2 = GetSectRArmor(mech, i) *
                         (mechspec & HARDA_TECH ? 200 : 100));
#if 0
/* NULLTODO : Port any of these techs ASAP */
					if(mechspec2 & TORSOCOCKPIT_TECH && i == CTORSO)
						armor += (debug4 =
								  (((GetSectArmor(mech, i) +
									 GetSectRArmor(mech,
												   i)) * 2) *
								   (mechspec & HARDA_TECH ? 200 : 100)));
#endif
    }
    if (!is_aero(mech))
      intern +=
          (debug3 = GetSectInt(mech, i) * (mechspec & COMPI_TECH    ? 50
                                           : mechspec & REINFI_TECH ? 200
                                                                    : 100));
    else
      intern = (debug3 = AeroSI(mech));
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
      if (IsWeapon(temp = GetPartType(mech, i, ii))) {
        weapindx = (Weapon2I(temp));
        if (PartIsNonfunctional(mech, i, ii)) {
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
                    (GetPartFireMode(mech, i, ii) & REAR_MOUNT ? 50 : 100)) *
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
          if (!(GetPartFireMode(mech, i, ii) & REAR_MOUNT)) {
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
      } else if (IsAmmo(temp)) {
        if (PartIsNonfunctional(mech, i, ii) || !GetPartData(mech, i, ii))
          continue;
#if 0
/* NULLTODO : Port any of these techs ASAP */
						mul =
							((temp2 =
							  GetPartAmmoMode(mech, i,
											  ii)) & AC_AP_MODE ? 4 : temp2 &
							 AC_PRECISION_MODE ? 6 : temp2 & (TRACER_MODE |
															  STINGER_MODE |
															  SWARM_MODE |
															  SWARM1_MODE |
															  SGUIDED_MODE) ?
							 1.5 : 1);
#else
        mul = ((temp2 = GetPartAmmoMode(mech, i, ii)) & AC_AP_MODE ? 4
               : temp2 & AC_PRECISION_MODE                         ? 6
               : temp2 & (SWARM_MODE | SWARM1_MODE | STINGER_MODE) ? 1.5
                                                                   : 1);
#endif
        mul = (mul * ((float)((float)GetPartData(mech, i, ii) /
                              (float)MechWeapons[weapindx = Ammo2WeaponI(temp)]
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
      if ((IsAmmo(temp) ||
           (IsWeapon(temp) && MechWeapons[(Weapon2I(temp))].special & GAUSS)) &&
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
            !(MechSections(mech)[i].config & CASE_TECH)) {

#ifdef DEBUG_BV
          btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                             "20 deduct added for ammo");
#endif

          deduct += 2000;
          continue;
        }
        if ((i == RARM || i == LARM) &&
            (!(MechSections(mech)[i].config & CASE_TECH) &&
             !(MechSections(mech)[(i == RARM ? RTORSO : LTORSO)].config &
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
    mostheat +=
        (MechJumpSpeed(mech) > 0 ? MAX((MechJumpSpeed(mech) / MP1) * 100, 300)
                                 : 200);
    if (mechspec2 & (NULLSIGSYS_TECH | STEALTH_ARMOR_TECH))
      mostheat += 1000;
    if ((temp = (mostheat - (MechActiveNumsinks(mech) * 100))) > 0) {
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
#if 0
/* NULLTODO : Port any of these techs ASAP */
			if(mechspec2 & HDGYRO_TECH)
				defweapbv += 3000;
#endif

  if (mechspec & (XL_TECH | XXL_TECH | LE_TECH)) {
    if (mechspec & (CLAN_TECH | LE_TECH))
      mul = 1.125;
    else
      mul = 0.75;
  } else if (mechspec & ICE_TECH || MechType(mech) == CLASS_VEH_GROUND ||
             MechType(mech) == CLASS_VEH_NAVAL) {
    mul = 0.5;
  } else {
    mul = 1.5;
  }

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("InternMul : %.2f", mul));
#endif

  armor = (armor * (MechType(mech) == CLASS_MECH ? 2 : 1));
  intern = intern * mul;
  mul = 1.00;

#ifdef DEBUG_BV
  btech_channel_send(
      mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
      tprintf("ArmorEnd : %d IntEnd : %d", armor / 100, intern / 100));
#endif

  maxspeed = MMaxSpeed(mech);
  if (mechspec & MASC_TECH || mechspec2 & SUPERCHARGER_TECH) {
    if (mechspec & MASC_TECH && mechspec2 & SUPERCHARGER_TECH)
      maxspeed = maxspeed * 2.5;
    else
      maxspeed = maxspeed * 1.5;
  }
  if (mechspec & TRIPLE_MYOMER_TECH)
    maxspeed = ((WalkingSpeed(maxspeed) + MP1) * 1.5);

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

  if (IsDS(mech))
    mul = 1.0;
  else if (is_aero(mech))
    mul = 1.1;

  if (mechspec2 & (NULLSIGSYS_TECH | STEALTH_ARMOR_TECH))
    mul += 1.5;
  if (MechInfantrySpecials(mech) & DC_KAGE_STEALTH_TECH)
    mul += .75;
  if (MechInfantrySpecials(mech) & FWL_ACHILEUS_STEALTH_TECH)
    mul += 1.5;
  if (MechInfantrySpecials(mech) & CS_PURIFIER_STEALTH_TECH)
    mul += 2.0;
  if (MechInfantrySpecials(mech) & FC_INFILTRATOR_STEALTH_TECH)
    mul += .75;
  if (MechInfantrySpecials(mech) & FC_INFILTRATOR_STEALTH_TECH)
    mul += 2.0;

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("DefBVMul : %.2f", mul));
#endif

  defbv = (armor + intern + (MechTons(mech) * 100) + defweapbv);

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("DefBV Tonnage added : %d", MechTons(mech)));
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

  if ((type == CLASS_MECH || is_aero(mech)) &&
      mostheat > (MechActiveNumsinks(mech) * 100)) {
#ifdef DEBUG_BV
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("Pre-Heat OffWeapBV : %d", offweapbv / 100));
#endif
    i = (((MechActiveNumsinks(mech) / 100) * offweapbv) / mostheat);
    ii = ((offweapbv - i) / 2);
    offweapbv = i + ii;

#ifdef DEBUG_BV
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("Post-Heat OffWeapBV : %d", offweapbv / 100));
#endif
  }
  /*
  mul = pow(((((MMaxSpeed(mech) / MP1) + (type == CLASS_AERO || type == CLASS_DS
  ? 0 : (MechJumpSpeed(mech) / MP1)) + (mechspec & MASC_TECH ? 1 : 0) +
  (mechspec & TRIPLE_MYOMER_TECH ? 1 : 0)+ (mechspec2 & SUPERCHARGER_TECH ? 1 :
  0) - 5) / 10) + 1), 1.2);
  */
  mul = pow((((((IsDS(mech) ? WalkingSpeed(MMaxSpeed(mech)) : MMaxSpeed(mech)) /
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
  if (type != CLASS_AERO && type != CLASS_DS && MechJumpSpeed(mech) > 0)
    offweapbv += ((MechJumpSpeed(mech) / MP1) * (100 * (MechTons(mech) / 5)));
  offbv = offweapbv;

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("OffWeapBVAfter : %d", offweapbv / 100));
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("DefBV : %d OffBV : %d TotalBV : %d", defbv / 100,
                             offbv / 100, (offbv + defbv) / 100));
#endif

  mul = (skillmul[LAZY_SKILLMUL(gunskl)][LAZY_SKILLMUL(pilskl)]);

#ifdef DEBUG_BV
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("SkillMul : %.2f (%d/%d)", mul, gunskl, pilskl));
#endif
  return ((offbv + defbv) / 100) * mul;
}
#endif
