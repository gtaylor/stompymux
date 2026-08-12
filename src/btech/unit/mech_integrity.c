#include "ai_api.h"
#include "btech_channel.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_sensor_state_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mech_utils_internal.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "section_types.h"
#include "template_api.h"
#include <stdio.h>

int mech_recycling_state(Mech *mech, int num) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (num & CHECK_WEAPS && sect_has_busy_weap(mech, i))
      return 1;
    if (num & CHECK_PHYS && mech_section_recycle_ticks(mech, i) > 0)
      return 2;
  }
  return 0;
}

#ifdef BT_COMPLEXREPAIRS
int GetPartMod(const Mech *mech, int t) {
  int val, div, bound;

  div = (t && t == special_equipment_index(GYRO)     ? 100
         : t && t == special_equipment_index(ENGINE) ? 20
                                                     : 10);
  bound = (t && t == special_equipment_index(GYRO)     ? 3
           : t && t == special_equipment_index(ENGINE) ? 19
                                                       : 9);
  val = (t && (t == special_equipment_index(GYRO) ||
               t == special_equipment_index(ENGINE))
             ? mech_engine_rating(mech)
             : ((mech)->ud.tons));

  if (val % div != 0)
    val = val + (div - (val % div));

  return BOUNDED(0, (val / div) - 1, bound);
}

int ProperArmor(const Mech *mech) {
  /* For now they all use the same basic cargo parts. */
  return cargo_equipment_index(
      ((mech)->rd.specials) & FF_TECH               ? FF_ARMOR
      : ((mech)->rd.specials) & HARDA_TECH          ? HD_ARMOR
      : ((mech)->rd.specials2) & HVY_FF_ARMOR_TECH  ? HVY_FF_ARMOR
      : ((mech)->rd.specials2) & LT_FF_ARMOR_TECH   ? LT_FF_ARMOR
      : ((mech)->rd.specials2) & STEALTH_ARMOR_TECH ? STH_ARMOR
                                                    : S_ARMOR);
}

int ProperInternal(const Mech *mech) {
  int part = 0;

  if (mech->xcode.context->configuration->btech_complexrepair) {
    part = (((mech)->rd.specials) & ES_TECH       ? TON_ESINTERNAL_FIRST
            : ((mech)->rd.specials) & REINFI_TECH ? TON_REINTERNAL_FIRST
            : ((mech)->rd.specials) & COMPI_TECH  ? TON_COINTERNAL_FIRST
                                                  : TON_INTERNAL_FIRST);
    part += GetPartMod(mech, 0);
  } else {
    part = (((mech)->rd.specials) & ES_TECH       ? ES_INTERNAL
            : ((mech)->rd.specials) & REINFI_TECH ? RE_INTERNAL
            : ((mech)->rd.specials) & COMPI_TECH  ? CO_INTERNAL
                                                  : S_INTERNAL);
  }
  return cargo_equipment_index(part);
}

int alias_part(Mech *mech, int t, int loc) {
  int part = 0;

  if (!equipment_is_special(t))
    return t;

  if (mech->xcode.context->configuration->btech_complexrepair) {
    int tonmod = GetPartMod(mech, t);
    int locmod;
    if (mech_is_quad(mech))
      locmod =
          (loc == RARM || loc == LARM || loc == RLEG || loc == LLEG ? 2 : 0);
    else
      locmod = (loc == RARM || loc == LARM   ? 1
                : loc == LLEG || loc == RLEG ? 2
                                             : 0);

    part =
        (locmod && (t == special_equipment_index(SHOULDER_OR_HIP) ||
                    t == special_equipment_index(UPPER_ACTUATOR))
             ? (locmod == 1
                    ? cargo_equipment_index(TON_ARMUPPER_FIRST + tonmod)
                    : cargo_equipment_index(TON_LEGUPPER_FIRST + tonmod))
         : locmod && t == special_equipment_index(LOWER_ACTUATOR)
             ? (locmod == 1
                    ? cargo_equipment_index(TON_ARMLOWER_FIRST + tonmod)
                    : cargo_equipment_index(TON_LEGLOWER_FIRST + tonmod))
         : locmod && t == special_equipment_index(HAND_OR_FOOT_ACTUATOR)
             ? (locmod == 1 ? cargo_equipment_index(TON_ARMHAND_FIRST + tonmod)
                            : cargo_equipment_index(TON_LEGFOOT_FIRST + tonmod))
         : t == special_equipment_index(ENGINE) &&
                 ((mech)->rd.specials) & XL_TECH
             ? cargo_equipment_index(TON_ENGINE_XL_FIRST + tonmod)
         : t == special_equipment_index(ENGINE) &&
                 ((mech)->rd.specials) & ICE_TECH
             ? cargo_equipment_index(TON_ENGINE_ICE_FIRST + tonmod)
         : t == special_equipment_index(ENGINE) &&
                 ((mech)->rd.specials) & CE_TECH
             ? cargo_equipment_index(TON_ENGINE_COMP_FIRST + tonmod)
         : t == special_equipment_index(ENGINE) &&
                 ((mech)->rd.specials) & XXL_TECH
             ? cargo_equipment_index(TON_ENGINE_XXL_FIRST + tonmod)
         : t == special_equipment_index(ENGINE) &&
                 ((mech)->rd.specials) & LE_TECH
             ? cargo_equipment_index(TON_ENGINE_LIGHT_FIRST + tonmod)
         : t == special_equipment_index(ENGINE)
             ? cargo_equipment_index(TON_ENGINE_FIRST + tonmod)
         : t == special_equipment_index(HEAT_SINK) &&
                 ((mech)->rd.specials) & (DOUBLE_HEAT_TECH | CLAN_TECH)
             ? cargo_equipment_index(DOUBLE_HEAT_SINK)
         : t == special_equipment_index(HEAT_SINK) &&
                 ((mech)->rd.specials2) & COMPACT_HS_TECH
             ? cargo_equipment_index(COMPACT_HEAT_SINK)
         : t == special_equipment_index(GYRO) &&
                 ((mech)->rd.specials2) & XLGYRO_TECH
             ? cargo_equipment_index(TON_XLGYRO_FIRST + tonmod)
         : t == special_equipment_index(GYRO) &&
                 ((mech)->rd.specials2) & HDGYRO_TECH
             ? cargo_equipment_index(TON_HDGYRO_FIRST + tonmod)
         : t == special_equipment_index(GYRO) &&
                 ((mech)->rd.specials2) & CGYRO_TECH
             ? cargo_equipment_index(TON_CGYRO_FIRST + tonmod)
         : t == special_equipment_index(GYRO)
             ? cargo_equipment_index(TON_GYRO_FIRST + tonmod)
         : t == special_equipment_index(SENSORS)
             ? cargo_equipment_index(TON_SENSORS_FIRST + tonmod)
         : t == special_equipment_index(JUMP_JET)
             ? cargo_equipment_index(TON_JUMPJET_FIRST + tonmod)
             : t);
  } else {
    part = (equipment_is_actuator(t) ? cargo_equipment_index(S_ACTUATOR)
            : t == special_equipment_index(ENGINE) &&
                    ((mech)->rd.specials) & XL_TECH
                ? cargo_equipment_index(XL_ENGINE)
            : t == special_equipment_index(ENGINE) &&
                    ((mech)->rd.specials) & ICE_TECH
                ? cargo_equipment_index(IC_ENGINE)
            : t == special_equipment_index(ENGINE) &&
                    ((mech)->rd.specials) & CE_TECH
                ? cargo_equipment_index(COMP_ENGINE)
            : t == special_equipment_index(ENGINE) &&
                    ((mech)->rd.specials) & XXL_TECH
                ? cargo_equipment_index(XXL_ENGINE)
            : t == special_equipment_index(ENGINE) &&
                    ((mech)->rd.specials) & LE_TECH
                ? cargo_equipment_index(LIGHT_ENGINE)
            : t == special_equipment_index(HEAT_SINK) &&
                    ((mech)->rd.specials) & (DOUBLE_HEAT_TECH | CLAN_TECH)
                ? cargo_equipment_index(DOUBLE_HEAT_SINK)
            : t == special_equipment_index(HEAT_SINK) &&
                    ((mech)->rd.specials2) & COMPACT_HS_TECH
                ? cargo_equipment_index(COMPACT_HEAT_SINK)
            : t == special_equipment_index(GYRO) &&
                    ((mech)->rd.specials2) & XLGYRO_TECH
                ? cargo_equipment_index(XL_GYRO)
            : t == special_equipment_index(GYRO) &&
                    ((mech)->rd.specials2) & HDGYRO_TECH
                ? cargo_equipment_index(HD_GYRO)
            : t == special_equipment_index(GYRO) &&
                    ((mech)->rd.specials2) & CGYRO_TECH
                ? cargo_equipment_index(COMP_GYRO)
                : t);
  }
  return part;
}

int ProperMyomer(Mech *mech) {
  int part;

  part = (((mech)->rd.specials) & TRIPLE_MYOMER_TECH ? TON_TRIPLEMYOMER_FIRST
                                                     : TON_MYOMER_FIRST);
  part += GetPartMod(mech, 0);

  return cargo_equipment_index(part);
}
#endif

/* Function to return a value of how much heat a unit is putting out*/
/* TODO: Double check how Stealth Armor and Null Sig are coded */
int heat_factor(Mech *mech) {

  int factor = 0;
  char buf[LBUF_SIZE];

  if (((mech)->ud.type) != CLASS_MECH) {
    factor = (((((mech)->rd.specials) & ICE_TECH)) ? -1 : 21);
    return factor;
  }
  factor = clamp_float_to_int(
      mech->rd.plus_heat + 2.0F * (mech->rd.plus_heat - mech->rd.minus_heat));
  return ((mech_condition_summary(mech).null_signature_active ||
           mech_has_working_ecm_suite(mech) ||
           mech_condition_summary(mech).stealth_armor_active)
              ? -1
              : factor);

  (void)snprintf(buf, LBUF_SIZE,
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
int weapon_is_nonfunctional(Mech *mech, int section, int crit, int numcrits) {
  int disabled = 0;
  int dested = 0;
  int count = 0;
  int nloc;
  int ncrit;
  int i;

  if (numcrits <= 0)
    numcrits = get_weapon_crits(
        mech, weapon_from_equipment_index(
                  mech_critical_part_type(mech, section, crit)));

  for (i = crit; i < min(NUM_CRITICALS, crit + numcrits); i++) {
    if (mech_critical_is_destroyed(mech, section, i))
      dested++;
    else if (mech_critical_is_disabled(mech, section, i))
      disabled++;
    count++;
  }

  if (count < numcrits && ((mech)->ud.type) == CLASS_MECH) {
    SplitCriticalLookup split_lookup =
        split_critical_find(mech, (CriticalSlotReference){section, crit});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (numcrits - count); i++) {
        if (mech_critical_is_destroyed(mech, nloc, i))
          dested++;
        else if (mech_critical_is_disabled(mech, nloc, i))
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
                   ((mech)->rd.specials2) & STEALTH_ARMOR_TECH  ? "ST_ARMOR"
                   : ((mech)->rd.specials2) & HVY_FF_ARMOR_TECH ? "HVY_FF_ARMOR"
                   : ((mech)->rd.specials2) & LT_FF_ARMOR_TECH  ? "LT_FF_ARMOR"
                   : ((mech)->rd.specials2) & HARDA_TECH        ? "HD_ARMOR"
                   : ((mech)->rd.specials) & FF_TECH            ? "FF_ARMOR"
                                                                : "ARMOR",
                   mech_armorpoints(mech)),
           buffer, &bp);

  safe_str(tprintf("%s:%d|",
                   ((mech)->rd.specials) & REINFI_TECH  ? "RE_INTERNALS"
                   : ((mech)->rd.specials) & COMPI_TECH ? "CO_INTERNALS"
                   : ((mech)->rd.specials) & ES_TECH    ? "ES_INTERNAL"
                                                        : "INTERNAL",
                   mech_intpoints(mech)),
           buffer, &bp);

  safe_str(tprintf("%s|", payloadlist_func(mech, (char[MBUF_SIZE]){0})), buffer,
           &bp);

  safe_str(tprintf("%s", partlist_func(mech, (char[LBUF_SIZE]){0})), buffer,
           &bp);
  *bp = '\0';
}
