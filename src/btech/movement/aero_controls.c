#include <stdlib.h>

#include "aero_move_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

enum { MIN_TAKEOFF_SPEED = 3 };

void aero_thrust(DbRef player, void *data, char *arg) {
  Mech *mech = (Mech *)data;
  char *args[1];
  float newspeed;
  float maxspeed;

  if (mech_is_landed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're landed!");
    return;
  }
  if (mech_is_aerospace_unit(mech) && mech_condition_summary(mech).spinning &&
      !mech_is_landed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are unable to control your craft at the moment.");
    return;
  }
  if (mech_parseattributes(arg, args, 1) != 1) {
    notify_printf(btech_context_evaluation(mech_context(mech)), player,
                  "Your current thrust is %.2f.",
                  (double)mech_desired_speed(mech));
    return;
  }
  newspeed = strtof(args[0], nullptr);
  if ((mech_class(mech) == CLASS_AERO || mech_class(mech) == CLASS_DS))
    if (newspeed < (MP1 * (float)MIN_TAKEOFF_SPEED / (float)ACCEL_MOD)) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          tprintf("Minimum thrust you stay in air with is %.1f kph.",
                  (double)(MP1 * (float)MIN_TAKEOFF_SPEED / (float)ACCEL_MOD)));
      return;
    }
  maxspeed = mech_effective_maximum_speed(mech);
  if (!(maxspeed > 0.0F))
    maxspeed = 0.0F;
  if (mech_condition_summary(mech).fallen) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your engine's dead, no way to thrust!");
    return;
  }
  if (newspeed < 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Doh, thrust backwards.. where's your sense of adventure?");
    return;
  }
  if (newspeed > maxspeed) {
    notify_printf(btech_context_evaluation(mech_context(mech)), player,
                  "Maximum thrust: %.2f (%.2f kb/sec2)", (double)maxspeed,
                  (double)(maxspeed / 10.0F));
    return;
  }
  mech_desired_speed_set(mech, newspeed);
  mech_printf(mech, MECHALL, "Thrust set to %.2f.", (double)newspeed);
  mech_maybe_move(mech);
}

void aero_vheading(DbRef player, void *data, char *arg, int flag) {
  char *args[1];
  int i = 0;
  Mech *mech = (Mech *)data;

  if (mech_parseattributes(arg, args, 1) != 1) {
    notify_printf(btech_context_evaluation(mech_context(mech)), player,
                  "Present angle: %d degrees.", mech_desired_angle(mech));
    return;
  }
  if (!parse_int_checked(args[0], &i)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid angle!");
    return;
  }
  i *= flag;
  if (abs(i) > 90)
    i = 90 * flag;
  if (abs(i) != 90 && mech_position_z(mech) < ATMO_Z &&
      (mech_class(mech) == CLASS_SPHEROID_DS)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 tprintf("You can go only up / down at <%d z!", ATMO_Z));
    return;
  }
  if (i >= 0)
    mech_printf(mech, MECHALL, "Climbing angle set to %d degrees.", i);
  else
    mech_printf(mech, MECHALL, "Diving angle set to %d degrees.", 0 - i);
  mech_desired_angle_set(mech, i);
}

void aero_climb(DbRef player, Mech *mech, char *arg) {
  aero_vheading(player, mech, arg, 1);
}

void aero_dive(DbRef player, Mech *mech, char *arg) {
  aero_vheading(player, mech, arg, -1);
}
