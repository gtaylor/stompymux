/* Contract tests for armor_string_from_index and the section-name tables it
 * renders from. The function writes into a caller-provided buffer whose
 * minimum size is fixed by UNIT_SECTION_NAME_CAPACITY, so the capacity check
 * below is what keeps a newly added section name from silently truncating at
 * every call site. */

#include "equipment_types.h"
#include "mech_utils_api.h"
#include "section_types.h"

#include <string.h>

static bool renders_as(int index, UnitClass type, MechMovementType movement,
                       const char *expected) {
  char buffer[UNIT_SECTION_NAME_CAPACITY];
  armor_string_from_index(index, buffer, type, movement);
  return strcmp(buffer, expected) == 0;
}

int main(void) {
  /* Valid indices render the section name for the unit's class. */
  if (!renders_as(0, CLASS_MECH, MOVE_BIPED, "Left Arm"))
    return 1;
  if (!renders_as(7, CLASS_MECH, MOVE_BIPED, "Head"))
    return 2;

  /* Movement type selects the quad table for the same class. */
  if (!renders_as(0, CLASS_MECH, MOVE_QUAD, "Front Left Leg"))
    return 3;
  if (!renders_as(6, CLASS_MECH, MOVE_QUAD, "Rear Right Leg"))
    return 4;

  /* The longest name in any table still round-trips intact, which is the
   * case the capacity constant exists to protect. */
  if (!renders_as(0, CLASS_SPHEROID_DS, MOVE_NONE, "Front Right Side"))
    return 5;

  /* Out-of-range indices fall back to the marker rather than reading past
   * the end of the table. */
  if (!renders_as(-1, CLASS_MECH, MOVE_BIPED, "Invalid!!"))
    return 6;
  if (!renders_as(NUM_SECTIONS, CLASS_MECH, MOVE_BIPED, "Invalid!!"))
    return 7;
  if (!renders_as(1000, CLASS_AERO, MOVE_FLY, "Invalid!!"))
    return 8;

  /* An index valid for a larger class is still rejected for a smaller one:
   * aerospace units have fewer sections than 'Mechs. */
  if (!renders_as(NUM_AERO_SECTIONS, CLASS_AERO, MOVE_FLY, "Invalid!!"))
    return 9;

  /* Capacity contract: every name every class can produce must fit, with its
   * terminator, inside the buffer callers are required to supply. */
  for (int type = CLASS_MECH; type <= CLASS_LAST; type++) {
    for (int movement = MOVE_BIPED; movement <= MOVENEMENT_LAST; movement++) {
      const UnitSectionCatalog CATALOG = {.unit_type = type,
                                          .movement_type = movement};
      const size_t COUNT = unit_section_name_count(&CATALOG);
      for (size_t i = 0; i < COUNT; i++) {
        const char *const NAME = unit_section_name(&CATALOG, i);
        if (NAME == nullptr)
          return 10;
        if (strlen(NAME) + 1 > UNIT_SECTION_NAME_CAPACITY)
          return 11;

        /* Rendering must reproduce the table entry exactly, never a
         * truncation of it. */
        char buffer[UNIT_SECTION_NAME_CAPACITY];
        armor_string_from_index((int)i, buffer, (UnitClass)type,
                                (MechMovementType)movement);
        if (strcmp(buffer, NAME) != 0)
          return 12;
      }

      /* One past the end is out of range for every class/movement pair. */
      if (unit_section_name(&CATALOG, COUNT) != nullptr)
        return 13;
    }
  }

  return 0;
}
