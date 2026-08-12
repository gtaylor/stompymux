#include "mech_weight_descriptions.h"

#include "equipment_types.h"
#include "mech_internal.h"
#include "mech_status_types.h"

const char *mech_weight_mech_engine_name(const Mech *mech) {
  if (mech->rd.specials & XL_TECH)
    return "Engine (XL)";
  if (mech->rd.specials & XXL_TECH)
    return "Engine (XXL)";
  if (mech->rd.specials & CE_TECH)
    return "Engine (Compact)";
  if (mech->rd.specials & LE_TECH)
    return "Engine (Light)";
  return "Engine";
}

const char *mech_weight_vehicle_engine_name(const Mech *mech) {
  if (mech->rd.specials & LE_TECH)
    return "Engine (Light)";
  if (mech->rd.specials & CE_TECH)
    return "Engine (Compact)";
  if (mech->rd.specials & XXL_TECH)
    return "Engine (XXL)";
  if (mech->rd.specials & XL_TECH)
    return "Engine (XL)";
  if (mech->rd.specials & ICE_TECH)
    return "Engine (ICE)";
  return "Engine";
}

const char *mech_weight_internal_structure_name(const Mech *mech) {
  if (mech->rd.specials & REINFI_TECH)
    return "Internals (Reinforced)";
  if (mech->rd.specials & COMPI_TECH)
    return "Internals (Composite)";
  if (mech->rd.specials & ES_TECH)
    return "Internals (ES)";
  return "Internals";
}

const char *mech_weight_armor_name(const Mech *mech) {
  if (mech->rd.specials2 & STEALTH_ARMOR_TECH)
    return "Armor (Stealth)";
  if (mech->rd.specials2 & HVY_FF_ARMOR_TECH)
    return "Armor (Hvy FF)";
  if (mech->rd.specials2 & LT_FF_ARMOR_TECH)
    return "Armor (Lt FF)";
  if (mech->rd.specials & HARDA_TECH)
    return "Armor (Hardened)";
  if (mech->rd.specials & FF_TECH)
    return "Armor (FF)";
  return "Armor";
}
