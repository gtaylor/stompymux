/* Selects sensors and scan policies for autopilots. */

#include "autopilot.h"
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

/* Function to determine if there are any slites affecting the AI */
int SearchLightInRange(Mech *mech, BattleMap *map) {

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
        mech_condition_summary(mech).searchlight_destroyed)
      continue;

    /* Is the mech close enough to be affected by the slite */
    if (mech_range_to(target, mech) < LITE_RANGE) {

      /* Returning true, but let's differentiate also between being in-arc. */
      if (mech_searchlight_active(target) &&
          InWeaponArc(target, mech_position_real_x(mech),
                      mech_position_real_y(mech)) &
              FORWARDARC) {

        /* Make sure its in los */
        if (!battle_map_unit_los_is_blocked(map, target, mech))

          /* Slite on and, arced, and LoS to you */
          return 3;
        else
          /* Slite on, arced, but LoS blocked */
          return 4;

      } else if (!mech_searchlight_active(target) &&
                 InWeaponArc(target, mech_position_real_x(mech),
                             mech_position_real_y(mech)) &
                     FORWARDARC) {

        if (!battle_map_unit_los_is_blocked(map, target, mech))

          /* Slite off, arced, and LoS to you */
          return 5;

        else
          /* Slite off, arced, and LoS blocked */
          return 6;
      }

      /* Slite is in range of you, but apparently not arced on you.
       * Return tells wether on or off */
      return (mech_searchlight_active(target) ? 1 : 2);
    }
  }
  return 0;
}

/* Function to determine if the AI should use V or L sensor */
int PrefVisSens(Mech *mech, BattleMap *map, int slite, Mech *target) {

  /* No map or mech so use default till we get put somewhere */
  if (!mech || !map)
    return SENSOR_VIS;

  /* Ok the AI is lit or using slite so use V */
  if (mech_searchlight_active(mech) || mech_condition_summary(mech).illuminated)
    return SENSOR_VIS;

  /* The target is lit so use V */
  if (target && mech_condition_summary(target).illuminated)
    return SENSOR_VIS;

  /* Ok if its night/dawn/dusk and theres no slite use L */
  if (battle_map_light(map) <= 1 && slite != 3 && slite != 5)
    return SENSOR_LA;

  /* Default sensor */
  return SENSOR_VIS;
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
  int set = 0;

  if (!is_good_obj(autopilot->xcode.context->database, autopilot->mymechnum)) {
    dprintk("mymechnum is bad!");
    return;
  }
  if (!is_good_obj(autopilot->xcode.context->database, autopilot->mynum)) {
    dprintk("mynum is bad!");
    return;
  }

  Mech *mech = (Mech *)autopilot->mymech;

  /* Make sure its a MECH Xcode Object and the AI is
   * an AUTOPILOT Xcode Object */
  /* Basic checks */
  if (!mech) {
    dprintk("mech is bad!");
    return;
  }
  if (!autopilot) {
    dprintk("ai is bad!");
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
  slite = (visibility != 2 ? SearchLightInRange(mech, map) : 0);
  rvis = (battle_map_light(map) ? visibility : (visibility * (slite ? 1 : 3)));
  prefvis = PrefVisSens(mech, map, slite, target);

  /* Is there a target */
  if (target) {

    /* Range to target */
    trng = mech_range_to(mech, target);

    /* Actually not gonna bother with this */
    /* If the target is running hot and is close switch to IR */
    if (!set && HeatFactor(target) > 35 && (int)trng < 15) {
      // wanted_s[0] = SENSOR_IR;
      // wanted_s[1] = ((mech_tonnage(target) >= 60) ? SENSOR_EM : prefvis);
      // set++;
    }

    /* If the target is BIG and close enough, use EM */
    if (!set && mech_tonnage(target) >= 60 && (int)trng <= 20) {
      wanted_s[0] = SENSOR_EM;
      wanted_s[1] = SENSOR_IR;
      set++;
    }

    /* If the target is flying switch to Radar */
    if (!set && !mech_is_landed(target) && mech_is_flying_type(target)) {
      wanted_s[0] = SENSOR_RA;
      wanted_s[1] = prefvis;
      set++;
    }

    /* If the target is really close and the unit has BAP, use it */
    if (!set && (int)trng <= 4 && mech_has_operational_beagle_probe(mech)) {
      wanted_s[0] = SENSOR_BAP;
      wanted_s[1] = SENSOR_BAP;
      set++;
    }

    /* If the target is really close and the unit has Bloodhound, use it */
    if (!set && (int)trng <= 8 && mech_has_operational_bloodhound_probe(mech)) {
      wanted_s[0] = SENSOR_BHAP;
      wanted_s[1] = SENSOR_BHAP;
      set++;
    }

    /* Didn't stop at any of the others so use selected visual sensors */
    if (!set) {
      wanted_s[0] = prefvis;
      wanted_s[1] = (rvis <= 15 ? SENSOR_EM : prefvis);
      set++;
    }
  }

  /* Ok no target and no sensors set yet so lets go for defaults */
  if (!set) {
    if (rvis <= 15) {
      /* Vis is less then or equal to 15 so go to E I for longer range */
      wanted_s[0] = SENSOR_EM;
      wanted_s[1] = SENSOR_IR;
    } else {
      /* Ok lets go with default visual sensors */
      wanted_s[0] = prefvis;
      wanted_s[1] = prefvis;
    }
  }

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
    MarkForLOSUpdate(mech);
  }
}

/*
 * Create a weapon_list node
 */
