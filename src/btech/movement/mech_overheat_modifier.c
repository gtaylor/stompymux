#include "mech_update_api.h"

#include "mech_heat_api.h"

int mech_overheat_to_hit_modifier(const Mech *mech) {
  float heat = mech_excess_heat(mech);
  if (heat >= 24.)
    return 4;
  if (heat >= 17.)
    return 3;
  if (heat >= 13.)
    return 2;
  if (heat >= 8.)
    return 1;
  return 0;
}
