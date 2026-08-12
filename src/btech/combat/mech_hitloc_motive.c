/* Implements BattleTech combat mechanics for unit hitloc motive. */

#include <math.h>
#include <string.h>

#include "crit_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "section_types.h"

/* Do L3 FASA motive system crits */
void mech_motive_system_hit(Mech *mech, int w_roll_mod) {
  int w_roll;
  const char MAX_LEN = 64;
  char str_vhl_type_name[64];

  w_roll = btech_random_roll(mech_context(mech)) + w_roll_mod;

  switch (mech_movement_type(mech)) {
  case MOVE_TRACK:
    strcpy(str_vhl_type_name, "tank");
    break;
  case MOVE_WHEEL:
    strcpy(str_vhl_type_name, "vehicle");
    w_roll += 2;
    break;
  case MOVE_HOVER:
    strcpy(str_vhl_type_name, "hovercraft");
    w_roll += 4;
    break;
  case MOVE_HULL:
    strcpy(str_vhl_type_name, "ship");
    break;
  case MOVE_FOIL:
    strcpy(str_vhl_type_name, "hydrofoil");
    w_roll += 4;
    break;
  case MOVE_SUB:
    strncpy(str_vhl_type_name, "submarine", MAX_LEN);
    break;
  case MOVE_BIPED:
  case MOVE_VTOL:
  case MOVE_FLY:
  case MOVE_QUAD:
  case MOVE_NONE:
  default:
    strncpy(str_vhl_type_name, "weird unidentifiable toy (warn a wizard!)",
            MAX_LEN);
    break;
  }

  if (w_roll < 8) /* no effect */
    return;

  mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");

  if (w_roll < 10) { /* minor effect */
    mech_pilot_skill_modifier_add(mech, 1);

    if (mech_condition_summary(mech).fallen) {
      mech_notify(mech, MECHALL,
                  "[fg=red bold]Your destroyed motive system takes another "
                  "hit![reset]");
    } else {
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]Your motive system takes a minor hit, making it "
          "harder to control your %s![reset]",
          str_vhl_type_name);
    }

    if (fabsf(mech_current_speed(mech)) > 0.0F)
      mech_los_broadcast(mech, "wobbles slightly.");
  } else if (w_roll < 12) { /* moderate effect */
    mech_pilot_skill_modifier_add(mech, 2);

    if (mech_condition_summary(mech).fallen) {
      mech_notify(mech, MECHALL,
                  "[fg=red bold]Your destroyed motive system takes another "
                  "hit![reset]");
    } else {
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]Your motive system takes a moderate hit, slowing "
          "you down and making it harder to control your %s![reset]",
          str_vhl_type_name);
    }

    if (fabsf(mech_current_speed(mech)) > 0.0F)
      mech_los_broadcast(mech, "wobbles violently.");

    mech_max_speed_lower(mech, MP1);
    mech_speed_correct(mech);
  } else {
    if (mech_condition_summary(mech).fallen) {
      mech_notify(mech, MECHALL,
                  "[fg=red bold]Your destroyed motive system takes another "
                  "hit![reset]");
    } else {
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]Your motive system is destroyed! Your %s can no "
          "longer move![reset]",
          str_vhl_type_name);
    }

    if (mech_current_speed(mech) > 0)
      mech_los_broadcast(mech, "shakes violently then begins to slow down.");

    mech_max_speed_set(mech, 0.0);
    mech_make_fall(mech);
    mech_speed_correct(mech);
  }
}
