/* Selects sensors and scan policies for autopilots. */

#include "autopilot.h"
#include "autopilot_sensor_policy_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_los_api.h"
#include "map_units_api.h"
#include "mech_condition_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/diagnostics.h"
#include "registry_api.h"

static int sensor_index_clamp(int sensor) {
  if (sensor < SENSOR_VIS)
    return SENSOR_VIS;
  if (sensor > SENSOR_BHAP)
    return SENSOR_BHAP;
  return sensor;
}

int autopilot_searchlight_classify(bool active, bool in_arc,
                                   bool line_of_sight_blocked) {
  if (!in_arc)
    return active ? 1 : 2;
  if (active)
    return line_of_sight_blocked ? 4 : 3;
  return line_of_sight_blocked ? 6 : 5;
}

int autopilot_visual_sensor_select(bool observer_lit, bool target_lit,
                                   int map_light,
                                   int searchlight_classification) {
  if (observer_lit || target_lit)
    return SENSOR_VIS;
  if (map_light <= 1 && searchlight_classification != 3 &&
      searchlight_classification != 5)
    return SENSOR_LA;
  return SENSOR_VIS;
}

AutopilotSensorSelection
autopilot_sensor_select(const AutopilotSensorSituation *situation) {
  if (!situation->has_target) {
    if (situation->effective_visibility <= 15)
      return (AutopilotSensorSelection){.primary = SENSOR_EM,
                                        .secondary = SENSOR_IR};
    return (AutopilotSensorSelection){
        .primary = situation->preferred_visual_sensor,
        .secondary = situation->preferred_visual_sensor};
  }
  if (situation->target_tonnage >= 60 && situation->target_range <= 20)
    return (AutopilotSensorSelection){.primary = SENSOR_EM,
                                      .secondary = SENSOR_IR};
  if (situation->target_flying && !situation->target_landed)
    return (AutopilotSensorSelection){
        .primary = SENSOR_RA, .secondary = situation->preferred_visual_sensor};
  if (situation->target_range <= 4 && situation->has_beagle_probe)
    return (AutopilotSensorSelection){.primary = SENSOR_BAP,
                                      .secondary = SENSOR_BAP};
  if (situation->target_range <= 8 && situation->has_bloodhound_probe)
    return (AutopilotSensorSelection){.primary = SENSOR_BHAP,
                                      .secondary = SENSOR_BHAP};
  return (AutopilotSensorSelection){
      .primary = situation->preferred_visual_sensor,
      .secondary = situation->effective_visibility <= 15
                       ? SENSOR_EM
                       : situation->preferred_visual_sensor};
}

/* Function to determine if there are any slites affecting the AI */
int search_light_in_range(Mech *mech, BattleMap *map) {

  Mech *target;
  int i;

  /* Make sure theres a valid mech or map */
  if (!mech || !map)
    return 0;

  /* Loop through all the units on the map */
  for (i = 0; i < battle_map_unit_count(map); i++) {

    /* No units on the map */
    target = btech_context_find_object(mech_context(mech),
                                       battle_map_unit_dbref(map, i));
    if (!target)
      continue;

    /* The unit doesn't have slite on */
    if (!mech_has_searchlight(target) ||
        mech_condition_summary(target).searchlight_destroyed)
      continue;

    /* Is the mech close enough to be affected by the slite */
    if (mech_range_to(target, mech) < LITE_RANGE) {

      /* Returning true, but let's differentiate also between being in-arc. */
      const bool IN_ARC = (in_weapon_arc(target, mech_position_real_x(mech),
                                         mech_position_real_y(mech)) &
                           FORWARDARC) != 0;
      return autopilot_searchlight_classify(
          mech_searchlight_active(target), IN_ARC,
          IN_ARC && battle_map_unit_los_is_blocked(map, target, mech));
    }
  }
  return 0;
}

/* Function to determine if the AI should use V or L sensor */
int pref_vis_sens(Mech *mech, BattleMap *map, int slite, Mech *target) {

  /* No map or mech so use default till we get put somewhere */
  if (!mech || !map)
    return SENSOR_VIS;

  return autopilot_visual_sensor_select(
      mech_searchlight_active(mech) || mech_condition_summary(mech).illuminated,
      target != nullptr && mech_condition_summary(target).illuminated,
      battle_map_light(map), slite);
}

/*
 * AI event to let the AI decide what sensors to use for a given
 * target and situation
 */
/*! \todo {Improve this so it knows more about the terrain} */
void auto_sensor_event(Autopilot *autopilot) {
  Mech *target = nullptr;
  BattleMap *map;
  int wanted_s[2];
  int rvis;
  int slite, prefvis;
  float trng;

  if (!is_good_obj(autopilot->xcode.context->database, autopilot->mymechnum)) {
    DPRINTK("mymechnum is bad!");
    return;
  }
  if (!is_good_obj(autopilot->xcode.context->database, autopilot->mynum)) {
    DPRINTK("mynum is bad!");
    return;
  }

  Mech *mech = (Mech *)autopilot->mymech;

  /* Make sure its a MECH Xcode Object and the AI is
   * an AUTOPILOT Xcode Object */
  /* Basic checks */
  if (!mech) {
    DPRINTK("mech is bad!");
    return;
  }
  if (!autopilot) {
    DPRINTK("ai is bad!");
    return;
  }

  if (!btech_context_is_mech(mech_context(mech), mech_dbref(mech)) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Mech is dead so stop trying to shoot things */
  if (mech_is_destroyed(mech)) {
    autopilot_gunning_stop(autopilot);
    return;
  }

  /* Mech isn't started */
  if (!mech_is_started(mech)) {
    autopilot_gunning_suspend(autopilot);
    return;
  }

  /* The mech is using user defined sensors so don't try
   * and change them */
  if (autopilot->flags & AUTOPILOT_LSENS)
    return;

  /* Get the map */
  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map) {

    /* Bad Map */
    autopilot_gunning_suspend(autopilot);
    return;
  }

  /* Get the target if there is one */
  if (mech_target_dbref(mech) > 0)
    target =
        btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));

  /* Checks to see if there is slite, and what types of vis
   * and which visual sensor (V or L) to use */
  int visibility = battle_map_visibility(map);
  slite = (visibility != 2 ? search_light_in_range(mech, map) : 0);
  rvis = (battle_map_light(map) ? visibility : (visibility * (slite ? 1 : 3)));
  prefvis = pref_vis_sens(mech, map, slite, target);

  trng = target != nullptr ? mech_range_to(mech, target) : 0.0F;
  const AutopilotSensorSelection SELECTION =
      autopilot_sensor_select(&(AutopilotSensorSituation){
          .has_target = target != nullptr,
          .target_range = (int)trng,
          .target_tonnage = target != nullptr ? mech_tonnage(target) : 0,
          .target_flying = target != nullptr && mech_is_flying_type(target),
          .target_landed = target == nullptr || mech_is_landed(target),
          .has_beagle_probe = mech_has_operational_beagle_probe(mech),
          .has_bloodhound_probe = mech_has_operational_bloodhound_probe(mech),
          .preferred_visual_sensor = prefvis,
          .effective_visibility = rvis});
  wanted_s[0] = SELECTION.primary;
  wanted_s[1] = SELECTION.secondary;

  /* Check to make sure valid sensors are selected and then set them */
  if (wanted_s[0] >= SENSOR_VIS && wanted_s[0] <= SENSOR_BHAP &&
      wanted_s[1] >= SENSOR_VIS && wanted_s[1] <= SENSOR_BHAP &&
      (mech_sensor_index(mech, 0) != wanted_s[0] ||
       mech_sensor_index(mech, 1) != wanted_s[1])) {

    wanted_s[0] = sensor_index_clamp(wanted_s[0]);
    wanted_s[1] = sensor_index_clamp(wanted_s[1]);

    mech_sensors_set(mech, wanted_s[0], wanted_s[1]);
    mech_notify(mech, MECHALL, "As your sensors change, your lock clears.");
    mech_targeting_target_clear(mech);
    mark_for_los_update(mech);
  }
}

/*
 * Create a weapon_list node
 */
