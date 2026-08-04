#include "mech_utils_internal.h"

int MechFullNoRecycle(Mech *mech, int num) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (num & CHECK_WEAPS && SectHasBusyWeap(mech, i))
      return 1;
    if (num & CHECK_PHYS && MechSections(mech)[i].recycle > 0)
      return 2;
  }
  return 0;
}

#ifdef BT_COMPLEXREPAIRS
int GetPartMod(Mech *mech, int t) {
  int val, div, bound;

  div = (t && t == Special(GYRO) ? 100 : t && t == Special(ENGINE) ? 20 : 10);
  bound = (t && t == Special(GYRO) ? 3 : t && t == Special(ENGINE) ? 19 : 9);
  val =
      (t && (t == Special(GYRO) || t == Special(ENGINE)) ? MechEngineSize(mech)
                                                         : MechTons(mech));

  if (val % div != 0)
    val = val + (div - (val % div));

  return BOUNDED(0, (val / div) - 1, bound);
}

int ProperArmor(Mech *mech) {
  /* For now they all use the same basic cargo parts. */
  return Cargo(MechSpecials(mech) & FF_TECH               ? FF_ARMOR
               : MechSpecials(mech) & HARDA_TECH          ? HD_ARMOR
               : MechSpecials2(mech) & HVY_FF_ARMOR_TECH  ? HVY_FF_ARMOR
               : MechSpecials2(mech) & LT_FF_ARMOR_TECH   ? LT_FF_ARMOR
               : MechSpecials2(mech) & STEALTH_ARMOR_TECH ? STH_ARMOR
                                                          : S_ARMOR);
}

int ProperInternal(Mech *mech) {
  int part = 0;

  if (mech->xcode.context->configuration->btech_complexrepair) {
    part = (MechSpecials(mech) & ES_TECH       ? TON_ESINTERNAL_FIRST
            : MechSpecials(mech) & REINFI_TECH ? TON_REINTERNAL_FIRST
            : MechSpecials(mech) & COMPI_TECH  ? TON_COINTERNAL_FIRST
                                               : TON_INTERNAL_FIRST);
    part += GetPartMod(mech, 0);
  } else {
    part = (MechSpecials(mech) & ES_TECH       ? ES_INTERNAL
            : MechSpecials(mech) & REINFI_TECH ? RE_INTERNAL
            : MechSpecials(mech) & COMPI_TECH  ? CO_INTERNAL
                                               : S_INTERNAL);
  }
  return Cargo(part);
}

int alias_part(Mech *mech, int t, int loc) {
  int part = 0;

  if (!IsSpecial(t))
    return t;

  if (mech->xcode.context->configuration->btech_complexrepair) {
    int tonmod = GetPartMod(mech, t);
    int locmod;
    if (MechIsQuad(mech))
      locmod =
          (loc == RARM || loc == LARM || loc == RLEG || loc == LLEG ? 2 : 0);
    else
      locmod = (loc == RARM || loc == LARM   ? 1
                : loc == LLEG || loc == RLEG ? 2
                                             : 0);

    part = (locmod && (t == Special(SHOULDER_OR_HIP) ||
                       t == Special(UPPER_ACTUATOR))
                ? (locmod == 1 ? Cargo(TON_ARMUPPER_FIRST + tonmod)
                               : Cargo(TON_LEGUPPER_FIRST + tonmod))
            : locmod && t == Special(LOWER_ACTUATOR)
                ? (locmod == 1 ? Cargo(TON_ARMLOWER_FIRST + tonmod)
                               : Cargo(TON_LEGLOWER_FIRST + tonmod))
            : locmod && t == Special(HAND_OR_FOOT_ACTUATOR)
                ? (locmod == 1 ? Cargo(TON_ARMHAND_FIRST + tonmod)
                               : Cargo(TON_LEGFOOT_FIRST + tonmod))
            : t == Special(ENGINE) && MechSpecials(mech) & XL_TECH
                ? Cargo(TON_ENGINE_XL_FIRST + tonmod)
            : t == Special(ENGINE) && MechSpecials(mech) & ICE_TECH
                ? Cargo(TON_ENGINE_ICE_FIRST + tonmod)
            : t == Special(ENGINE) && MechSpecials(mech) & CE_TECH
                ? Cargo(TON_ENGINE_COMP_FIRST + tonmod)
            : t == Special(ENGINE) && MechSpecials(mech) & XXL_TECH
                ? Cargo(TON_ENGINE_XXL_FIRST + tonmod)
            : t == Special(ENGINE) && MechSpecials(mech) & LE_TECH
                ? Cargo(TON_ENGINE_LIGHT_FIRST + tonmod)
            : t == Special(ENGINE) ? Cargo(TON_ENGINE_FIRST + tonmod)
            : t == Special(HEAT_SINK) &&
                    MechSpecials(mech) & (DOUBLE_HEAT_TECH | CLAN_TECH)
                ? Cargo(DOUBLE_HEAT_SINK)
            : t == Special(HEAT_SINK) && MechSpecials2(mech) & COMPACT_HS_TECH
                ? Cargo(COMPACT_HEAT_SINK)
            : t == Special(GYRO) && MechSpecials2(mech) & XLGYRO_TECH
                ? Cargo(TON_XLGYRO_FIRST + tonmod)
            : t == Special(GYRO) && MechSpecials2(mech) & HDGYRO_TECH
                ? Cargo(TON_HDGYRO_FIRST + tonmod)
            : t == Special(GYRO) && MechSpecials2(mech) & CGYRO_TECH
                ? Cargo(TON_CGYRO_FIRST + tonmod)
            : t == Special(GYRO)     ? Cargo(TON_GYRO_FIRST + tonmod)
            : t == Special(SENSORS)  ? Cargo(TON_SENSORS_FIRST + tonmod)
            : t == Special(JUMP_JET) ? Cargo(TON_JUMPJET_FIRST + tonmod)
                                     : t);
  } else {
    part = (IsActuator(t) ? Cargo(S_ACTUATOR)
            : t == Special(ENGINE) && MechSpecials(mech) & XL_TECH
                ? Cargo(XL_ENGINE)
            : t == Special(ENGINE) && MechSpecials(mech) & ICE_TECH
                ? Cargo(IC_ENGINE)
            : t == Special(ENGINE) && MechSpecials(mech) & CE_TECH
                ? Cargo(COMP_ENGINE)
            : t == Special(ENGINE) && MechSpecials(mech) & XXL_TECH
                ? Cargo(XXL_ENGINE)
            : t == Special(ENGINE) && MechSpecials(mech) & LE_TECH
                ? Cargo(LIGHT_ENGINE)
            : t == Special(HEAT_SINK) &&
                    MechSpecials(mech) & (DOUBLE_HEAT_TECH | CLAN_TECH)
                ? Cargo(DOUBLE_HEAT_SINK)
            : t == Special(HEAT_SINK) && MechSpecials2(mech) & COMPACT_HS_TECH
                ? Cargo(COMPACT_HEAT_SINK)
            : t == Special(GYRO) && MechSpecials2(mech) & XLGYRO_TECH
                ? Cargo(XL_GYRO)
            : t == Special(GYRO) && MechSpecials2(mech) & HDGYRO_TECH
                ? Cargo(HD_GYRO)
            : t == Special(GYRO) && MechSpecials2(mech) & CGYRO_TECH
                ? Cargo(COMP_GYRO)
                : t);
  }
  return part;
}

int ProperMyomer(Mech *mech) {
  int part;

  part = (MechSpecials(mech) & TRIPLE_MYOMER_TECH ? TON_TRIPLEMYOMER_FIRST
                                                  : TON_MYOMER_FIRST);
  part += GetPartMod(mech, 0);

  return Cargo(part);
}
#endif

/* Function to return a value of how much heat a unit is putting out*/
/* TODO: Double check how Stealth Armor and Null Sig are coded */
int HeatFactor(Mech *mech) {

  int factor = 0;
  char buf[LBUF_SIZE];

  if (MechType(mech) != CLASS_MECH) {
    factor = (((MechSpecials(mech) & ICE_TECH)) ? -1 : 21);
    return factor;
  } else {
    factor =
        (MechPlusHeat(mech) + (2 * (MechPlusHeat(mech) - MechMinusHeat(mech))));
    return ((NullSigSysActive(mech) || HasWorkingECMSuite(mech) ||
             StealthArmorActive(mech))
                ? -1
                : factor);
  }
  snprintf(buf, LBUF_SIZE,
           "HeatFactor : Invalid heat factor calculation on #%ld.",
           mech->mynum);
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s", buf);
}

/* Function to determine if a weapon is functional or not
   Returns 0 if fully functional.
   Returns 1 if non functional.
   Returns 2 if fully damaged.
   Returns -(# of crits) if partially damaged.
   remember that values 3 means the weapon IS NOT destroyed.  */
int WeaponIsNonfunctional(Mech *mech, int section, int crit, int numcrits) {
  int disabled = 0, dested = 0;
  int count = 0, nloc, ncrit, stype;
  int i;

  if (numcrits <= 0)
    numcrits = GetWeaponCrits(mech, Weapon2I(GetPartType(mech, section, crit)));

  for (i = crit; i < MIN(NUM_CRITICALS, crit + numcrits); i++) {
    if (PartIsDestroyed(mech, section, i))
      dested++;
    else if (PartIsDisabled(mech, section, i))
      disabled++;
    count++;
  }

  if (count < numcrits && MechType(mech) == CLASS_MECH) {
    if (GetSplitData(mech, section, crit, &nloc, &ncrit, &stype)) {
      for (i = ncrit; i < (numcrits - count); i++) {
        if (PartIsDestroyed(mech, nloc, i))
          dested++;
        else if (PartIsDisabled(mech, nloc, i))
          disabled++;
      }
    }
  }

  if (disabled > 0)
    return 1;

  if ((numcrits == 1 && (dested || disabled)) ||
      (numcrits > 1 && (dested + disabled) >= numcrits / 2))
    return 2;

  if (dested)
    return 0 - (dested + disabled);

  return 0;
}

/* Parts needed for a Unit.  Basic premise is to scan a template
 * Grab Weapon crits...Tally Up Armor, Internals, crits and special crits
 * Very Similiar to BV Calcs, without the Calcs
 * Arguments:
 * [0] Mech_Template
 * [1] Mode
 * 	0 = Parts On Unit
 * 	1 = Parts Need to Fix Unit
 */

void unit_parts_list(Mech *mech, char buffer[static LBUF_SIZE]) {
  char *bp = buffer;

  buffer[0] = '\0';

  safe_str(tprintf("%s:%d|",
                   MechSpecials2(mech) & STEALTH_ARMOR_TECH  ? "ST_ARMOR"
                   : MechSpecials2(mech) & HVY_FF_ARMOR_TECH ? "HVY_FF_ARMOR"
                   : MechSpecials2(mech) & LT_FF_ARMOR_TECH  ? "LT_FF_ARMOR"
                   : MechSpecials2(mech) & HARDA_TECH        ? "HD_ARMOR"
                   : MechSpecials(mech) & FF_TECH            ? "FF_ARMOR"
                                                             : "ARMOR",
                   mech_armorpoints(mech)),
           buffer, &bp);

  safe_str(tprintf("%s:%d|",
                   MechSpecials(mech) & REINFI_TECH  ? "RE_INTERNALS"
                   : MechSpecials(mech) & COMPI_TECH ? "CO_INTERNALS"
                   : MechSpecials(mech) & ES_TECH    ? "ES_INTERNAL"
                                                     : "INTERNAL",
                   mech_intpoints(mech)),
           buffer, &bp);

  safe_str(tprintf("%s|", payloadlist_func(mech, (char[MBUF_SIZE]){0})), buffer,
           &bp);

  safe_str(tprintf("%s", partlist_func(mech, (char[LBUF_SIZE]){0})), buffer,
           &bp);
  *bp = '\0';
}
