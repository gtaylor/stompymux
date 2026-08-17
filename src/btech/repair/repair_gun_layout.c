/* Implements shared validation and coordinates for weapon critical footprints.
 */

#include "repair_gun_layout.h"

#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_tech_commands_api.h"
#include "mech_utils_api.h"
#include "section_types.h"

static bool repair_gun_layout_section_valid(int section) {
  return (section >= 0 && section < NUM_SECTIONS) != 0;
}

typedef struct RepairGunLayoutSpan {
  int section;
  int first;
  int count;
  int part_type;
  int first_position;
  bool split;
} RepairGunLayoutSpan;

static bool repair_gun_layout_contiguous(Mech *mech,
                                         const RepairGunLayoutSpan *span) {
  for (int critical = span->first; critical < span->first + span->count;
       critical++) {
    if (mech_critical_part_type(mech, span->section, critical) !=
        span->part_type)
      return false;
    if (span->split && mech_critical_data(mech, span->section, critical) !=
                           span->first_position)
      return false;
  }
  return true;
}

bool repair_gun_layout_find(Mech *mech, int location, int position,
                            unsigned int requirements,
                            RepairGunLayout *layout) {
  *layout = (RepairGunLayout){0};
  if (!repair_gun_layout_section_valid(location))
    return false;
  int critical_count = mech_section_critical_count(mech, location);
  if (position < 0 || position >= critical_count)
    return false;
  int part_type = mech_critical_part_type(mech, location, position);
  if ((requirements & REPAIR_GUN_LAYOUT_REQUIRE_WEAPON) != 0 &&
      !equipment_is_weapon(part_type))
    return false;
  if ((requirements & REPAIR_GUN_LAYOUT_REQUIRE_GUN_START) != 0 &&
      !valid_gun_pos(&(RepairCriticalSelection){
          .mech = mech, .location = location, .position = position}))
    return false;
  int size = get_weapon_crits(mech, weapon_from_equipment_index(part_type));
  if (size <= 0)
    return false;
  int available = critical_count - position;
  int local_count = size < available ? size : available;
  if ((requirements & REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS) != 0 &&
      !repair_gun_layout_contiguous(
          mech, &(RepairGunLayoutSpan){.section = location,
                                       .first = position,
                                       .count = local_count,
                                       .part_type = part_type,
                                       .first_position = position,
                                       .split = false}))
    return false;
  if ((requirements & REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS) != 0 &&
      (mech_section_is_destroyed(mech, location) ||
       mech_section_is_flooded(mech, location)))
    return false;

  layout->size = size;
  layout->local_count = local_count;
  if (local_count >= size)
    return true;
  if (mech_class(mech) != CLASS_MECH)
    return false;
  layout->split = split_critical_find(
      mech, (CriticalSlotReference){.section = location, .critical = position});
  int split_count;
  if (!layout->split.found ||
      !repair_gun_layout_section_valid(layout->split.slot.section) ||
      layout->split.slot.critical < 0)
    return false;
  split_count = mech_section_critical_count(mech, layout->split.slot.section);
  if (layout->split.slot.critical > split_count - (size - local_count))
    return false;
  if ((requirements & REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS) != 0 &&
      !repair_gun_layout_contiguous(
          mech, &(RepairGunLayoutSpan){.section = layout->split.slot.section,
                                       .first = layout->split.slot.critical,
                                       .count = size - local_count,
                                       .part_type = layout->split.part_type,
                                       .first_position = position,
                                       .split = true}))
    return false;
  if ((requirements & REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS) != 0 &&
      (mech_section_is_destroyed(mech, layout->split.slot.section) ||
       mech_section_is_flooded(mech, layout->split.slot.section)))
    return false;
  return true;
}
