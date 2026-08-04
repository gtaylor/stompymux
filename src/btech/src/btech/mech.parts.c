/* Parts inventory operations used by BTech repairs. */

#include "mech.parts.h"

#include "btech_context.h"
#include "btmacros.h"
#include "macros.h"
#include "mech.h"
#include "mech.lifecycle.h"
#include "mux/objects/db.h"
#include "mux/support/formatting.h"
#include "p.econ.h"
#include "p.mech.status.h"
#include "p.mech.tech.commands.h"
#include "p.mech.utils.h" // IWYU pragma: keep

static DbRef mech_parts_store(const MECH *mech) {
  if (IsDS(mech)) {
    return AeroBay(mech, 0);
  }
  return game_object_location(mech->xcode.context->database, mech->mynum);
}

int mech_parts_alias(MECH *mech, int location, int part) {
#ifdef BT_COMPLEXREPAIRS
  return alias_part(mech, part, location);
#else
  (void)location;
  if (IsActuator(part)) {
    return Cargo(S_ACTUATOR);
  }
  if (part == Special(ENGINE)) {
    if (MechSpecials(mech) & XL_TECH) {
      return Cargo(XL_ENGINE);
    }
    if (MechSpecials(mech) & ICE_TECH) {
      return Cargo(IC_ENGINE);
    }
    if (MechSpecials(mech) & XXL_TECH) {
      return Cargo(XXL_ENGINE);
    }
    if (MechSpecials(mech) & CE_TECH) {
      return Cargo(COMP_ENGINE);
    }
    if (MechSpecials(mech) & LE_TECH) {
      return Cargo(LIGHT_ENGINE);
    }
  }
  if (part == Special(HEAT_SINK) && MechHasDHS(mech)) {
    return Cargo(DOUBLE_HEAT_SINK);
  }
  return part;
#endif
}

bool mech_parts_available(MECH *mech, int part, int brand, int count) {
  return econ_find_items(mech->xcode.context, mech_parts_store(mech), part,
                         brand) >= count;
}

void mech_parts_take(MECH *mech, int part, int brand, int count) {
  econ_change_items(mech->xcode.context, mech_parts_store(mech), part, brand,
                    -count);
}

void mech_parts_add(MECH *mech, int location, int part, int brand, int count) {
  econ_change_items(mech->xcode.context, mech_parts_store(mech),
                    mech_parts_alias(mech, location, part), brand, count);
}

bool mech_parts_consume(MECH *mech, DbRef player,
                        const MechPartRequirement requirements[],
                        size_t count) {
  for (size_t index = 0; index < count; ++index) {
    const MechPartRequirement *requirement = &requirements[index];
    if (!mech_parts_available(mech, requirement->part, requirement->brand,
                              requirement->count)) {
      notify(btech_context_evaluation(mech->xcode.context), player,
             tprintf("Not enough units of %s in store! You need to have at "
                     "least %d.",
                     part_name(mech->xcode.context, requirement->part,
                               requirement->brand)
                         .text,
                     requirement->count));
      return false;
    }
  }
  for (size_t index = 0; index < count; ++index) {
    const MechPartRequirement *requirement = &requirements[index];
    mech_parts_take(mech, requirement->part, requirement->brand,
                    requirement->count);
  }
  return true;
}

bool mech_section_armor_repairing(MECH *mech, int section) {
  return SomeoneFixingA(mech, section);
}

bool mech_section_rear_armor_repairing(MECH *mech, int section) {
  return SomeoneFixingA(mech, section + 8);
}

bool mech_section_internals_repairing(MECH *mech, int section) {
  return SomeoneFixingI(mech, section);
}
