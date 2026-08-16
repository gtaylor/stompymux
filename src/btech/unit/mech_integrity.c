#include "ai_api.h"
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

/* Function to return a value of how much heat a unit is putting out*/
/* TODO: Double check how Stealth Armor and Null Sig are coded */
int heat_factor(Mech *mech) {

  int factor = 0;

  if (((mech)->ud.type) != CLASS_MECH) {
    factor = (((((mech)->rd.specials) & ICE_TECH)) ? -1 : 21);
    return factor;
  }
  factor = clamp_float_to_int(
      mech->rd.plus_heat + (2.0F * (mech->rd.plus_heat - mech->rd.minus_heat)));
  return ((mech_condition_summary(mech).null_signature_active ||
           mech_has_working_ecm_suite(mech) ||
           mech_condition_summary(mech).stealth_armor_active)
              ? -1
              : factor);
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
  const char *armor_name = "ARMOR";
  const char *internal_name = "INTERNAL";

  if (((mech)->rd.specials2) & STEALTH_ARMOR_TECH)
    armor_name = "ST_ARMOR";
  else if (((mech)->rd.specials2) & HVY_FF_ARMOR_TECH)
    armor_name = "HVY_FF_ARMOR";
  else if (((mech)->rd.specials2) & LT_FF_ARMOR_TECH)
    armor_name = "LT_FF_ARMOR";
  else if (((mech)->rd.specials2) & HARDA_TECH)
    armor_name = "HD_ARMOR";
  else if (((mech)->rd.specials) & FF_TECH)
    armor_name = "FF_ARMOR";

  if (((mech)->rd.specials) & REINFI_TECH)
    internal_name = "RE_INTERNALS";
  else if (((mech)->rd.specials) & COMPI_TECH)
    internal_name = "CO_INTERNALS";
  else if (((mech)->rd.specials) & ES_TECH)
    internal_name = "ES_INTERNAL";

  buffer[0] = '\0';

  safe_tprintf_str(buffer, &bp, "%s:%d|", armor_name, mech_armorpoints(mech));

  safe_tprintf_str(buffer, &bp, "%s:%d|", internal_name, mech_intpoints(mech));

  safe_tprintf_str(buffer, &bp, "%s|",
                   payloadlist_func(mech, (char[MBUF_SIZE]){0}, MBUF_SIZE));

  safe_str(partlist_func(mech, (char[LBUF_SIZE]){0}, LBUF_SIZE), buffer, &bp);
  *bp = '\0';
}
