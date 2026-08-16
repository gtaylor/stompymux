#include <stdbool.h>
#include <stdio.h>

#include "command_handlers_api.h"
#include "mech_classification_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_utils_api.h"
#include "section_types.h"

static Mech fixture_mech;
static int checked_section;
static int checked_critical;

int min(int left, int right) { return left < right ? left : right; }

UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return CLASS_MECH; }

bool mech_critical_is_damaged(const Mech *mech [[maybe_unused]], int section,
                              int critical) {
  checked_section = section;
  checked_critical = critical;
  return section == RTORSO && critical == 3;
}

SplitCriticalLookup split_critical_find(Mech *mech [[maybe_unused]],
                                        CriticalSlotReference source
                                        [[maybe_unused]]) {
  return (SplitCriticalLookup){
      .found = true,
      .slot = {.section = RTORSO, .critical = 3},
  };
}

static int test_split_critical_range(void) {
  checked_section = -1;
  checked_critical = -1;
  const int damaged =
      mech_weapon_damaged_slot_count(&fixture_mech, LARM, NUM_CRITICALS - 1, 2);
  if (damaged != 1 || checked_section != RTORSO || checked_critical != 3) {
    fprintf(stderr, "split critical slot range was not traversed\n");
    return 1;
  }
  return 0;
}

int main(void) { return test_split_critical_range(); }
