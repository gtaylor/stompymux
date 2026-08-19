#include "autopilot_sensor_policy_api.h"
#include "mech_sensor.h"

static bool selection_is(AutopilotSensorSituation situation, int primary,
                         int secondary) {
  AutopilotSensorSelection selection = autopilot_sensor_select(&situation);
  return selection.primary == primary && selection.secondary == secondary;
}

int main(void) {
  if (autopilot_searchlight_classify(true, true, false) != 3 ||
      autopilot_searchlight_classify(true, true, true) != 4 ||
      autopilot_searchlight_classify(false, true, false) != 5 ||
      autopilot_searchlight_classify(false, true, true) != 6 ||
      autopilot_searchlight_classify(true, false, false) != 1 ||
      autopilot_searchlight_classify(false, false, false) != 2)
    return 1;
  if (autopilot_visual_sensor_select(false, false, 1, 0) != SENSOR_LA ||
      autopilot_visual_sensor_select(false, false, 1, 3) != SENSOR_VIS ||
      autopilot_visual_sensor_select(false, false, 2, 5) != SENSOR_VIS ||
      autopilot_visual_sensor_select(true, false, 0, 0) != SENSOR_VIS ||
      autopilot_visual_sensor_select(false, true, 0, 0) != SENSOR_VIS)
    return 2;
  AutopilotSensorSituation situation = {.preferred_visual_sensor = SENSOR_VIS,
                                        .effective_visibility = 30};
  if (!selection_is(situation, SENSOR_VIS, SENSOR_VIS))
    return 3;
  situation.effective_visibility = 15;
  if (!selection_is(situation, SENSOR_EM, SENSOR_IR))
    return 4;
  situation = (AutopilotSensorSituation){.has_target = true,
                                         .target_range = 20,
                                         .target_tonnage = 60,
                                         .preferred_visual_sensor = SENSOR_LA,
                                         .effective_visibility = 30};
  if (!selection_is(situation, SENSOR_EM, SENSOR_IR))
    return 5;
  situation.target_tonnage = 40;
  situation.target_flying = true;
  if (!selection_is(situation, SENSOR_RA, SENSOR_LA))
    return 6;
  situation.target_flying = false;
  situation.target_range = 4;
  situation.has_beagle_probe = true;
  if (!selection_is(situation, SENSOR_BAP, SENSOR_BAP))
    return 7;
  situation.has_beagle_probe = false;
  situation.has_bloodhound_probe = true;
  situation.target_range = 8;
  if (!selection_is(situation, SENSOR_BHAP, SENSOR_BHAP))
    return 8;
  situation.has_bloodhound_probe = false;
  situation.effective_visibility = 15;
  if (!selection_is(situation, SENSOR_LA, SENSOR_EM))
    return 9;
  situation.effective_visibility = 16;
  if (!selection_is(situation, SENSOR_LA, SENSOR_LA))
    return 10;
  situation.target_landed = true;
  if (!selection_is(situation, SENSOR_LA, SENSOR_LA))
    return 11;
  situation.target_landed = false;
  situation.target_range = 3;
  situation.has_beagle_probe = true;
  situation.has_bloodhound_probe = true;
  return !selection_is(situation, SENSOR_BAP, SENSOR_BAP);
}
