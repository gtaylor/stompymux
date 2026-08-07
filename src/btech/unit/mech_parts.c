/* Parts inventory operations used by BTech repairs. */

#include "mech_parts.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"

#include "btech/context.h"
#include "econ_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_status_api.h"
#include "mech_tech_commands_api.h"
#include "mech_utils_api.h" // IWYU pragma: keep
#include "mux/objects/db.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

DbRef mech_parts_store_dbref(const Mech *mech) {
  if (mech_is_dropship(mech)) {
    return ((mech)->pd.bay[0]);
  }
  return game_object_location(mech->xcode.context->database, mech->mynum);
}

int mech_parts_alias(Mech *mech, int location, int part) {
#ifdef BT_COMPLEXREPAIRS
  return alias_part(mech, part, location);
#else
  (void)location;
  if (equipment_is_actuator(part)) {
    return cargo_equipment_index(S_ACTUATOR);
  }
  if (part == special_equipment_index(ENGINE)) {
    if (((mech)->rd.specials) & XL_TECH) {
      return cargo_equipment_index(XL_ENGINE);
    }
    if (((mech)->rd.specials) & ICE_TECH) {
      return cargo_equipment_index(IC_ENGINE);
    }
    if (((mech)->rd.specials) & XXL_TECH) {
      return cargo_equipment_index(XXL_ENGINE);
    }
    if (((mech)->rd.specials) & CE_TECH) {
      return cargo_equipment_index(COMP_ENGINE);
    }
    if (((mech)->rd.specials) & LE_TECH) {
      return cargo_equipment_index(LIGHT_ENGINE);
    }
  }
  if (part == special_equipment_index(HEAT_SINK) &&
      mech_has_double_heat_sinks(mech)) {
    return cargo_equipment_index(DOUBLE_HEAT_SINK);
  }
  return part;
#endif
}

bool mech_parts_available(Mech *mech, int part, int brand, int count) {
  return econ_find_items(mech->xcode.context, mech_parts_store_dbref(mech),
                         part, brand) >= count;
}

void mech_parts_take(Mech *mech, int part, int brand, int count) {
  econ_change_items(mech->xcode.context, mech_parts_store_dbref(mech), part,
                    brand, -count);
}

void mech_parts_add(Mech *mech, int location, int part, int brand, int count) {
  econ_change_items(mech->xcode.context, mech_parts_store_dbref(mech),
                    mech_parts_alias(mech, location, part), brand, count);
}

bool mech_parts_consume(Mech *mech, DbRef player,
                        const MechPartRequirement requirements[],
                        size_t count) {
  for (size_t index = 0; index < count; ++index) {
    const MechPartRequirement *requirement = &requirements[index];
    if (!mech_parts_available(mech, requirement->part, requirement->brand,
                              requirement->count)) {
      mecha_notify(
          btech_context_evaluation(mech->xcode.context), player,
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

bool mech_section_armor_repairing(Mech *mech, int section) {
  return SomeoneFixingA(mech, section);
}

bool mech_section_rear_armor_repairing(Mech *mech, int section) {
  return SomeoneFixingA(mech, section + 8);
}

bool mech_section_internals_repairing(Mech *mech, int section) {
  return SomeoneFixingI(mech, section);
}
