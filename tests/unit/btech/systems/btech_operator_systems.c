#include "mech_crew_api.h"
#include "mech_electronics_api.h"
#include "mech_network_api.h"

#include "btconfig.h"
#include "btech/context.h"
#include "btech/core/context_internal.h"
#include "mech_internal.h"

static Mech *next_unit(Mech *unit, int *evaluations) {
  ++*evaluations;
  return unit;
}

int main(void) {
  BtechContext context = {0};
  Mech unit = {.xcode.context = &context};
  int evaluations = 0;

  mech_pilot_dbref_set(next_unit(&unit, &evaluations), 9);
  if (evaluations != 1 || mech_gunner_dbref(&unit) != 9)
    return 1;
  context.combat_overrides.pilot = 17;
  if (mech_gunner_dbref(&unit) != 17)
    return 2;

  mech_pilot_status_set(&unit, 2);
  mech_pilot_skill_modifier_set(&unit, 3);
  mech_base_to_hit_modifier_set(&unit, 4);
  mech_perception_target_set(&unit, 5);
  if (mech_pilot_status(&unit) != 2 || mech_pilot_skill_modifier(&unit) != 3 ||
      mech_base_to_hit_modifier(&unit) != 4 ||
      mech_perception_target(&unit) != 5)
    return 3;

  mech_computer_quality_set(&unit, 4);
  mech_radio_quality_set(&unit, 5);
  mech_radio_configuration_set(&unit, 23);
  if (mech_default_scanner_range(&unit) != (int)(DEFAULT_SCANRANGE * 1.5F) ||
      mech_default_long_range_sensor_range(&unit) !=
          (int)(DEFAULT_LRSRANGE * 1.5F) ||
      mech_default_tactical_range(&unit) != (int)(DEFAULT_TACRANGE * 1.5F) ||
      mech_default_radio_range(&unit) != (int)(DEFAULT_RADIORANGE * 1.75F) ||
      mech_radio_configuration(&unit) != 23)
    return 4;

  mech_c3_total_masters_set(&unit, 6);
  mech_c3_working_masters_set(&unit, 4);
  mech_c3i_network_size_set(&unit, 3);
  return mech_c3_total_masters(&unit) == 6 &&
                 mech_c3_working_masters(&unit) == 4 &&
                 mech_c3i_network_size(&unit) == 3
             ? 0
             : 5;
}
