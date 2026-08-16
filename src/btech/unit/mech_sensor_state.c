#include "mech_sensor_state_api.h"

#include <stdlib.h>

#include "checked_conversion.h"
#include "mech_internal.h"
#include "mech_status_types.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"

int mech_sensor_index(const Mech *mech, int slot) {
  if (slot < 0)
    abort();
  const char *sensor = checked_storage_at_const(
      mech->rd.sensor, 2, sizeof(*mech->rd.sensor), (size_t)slot);
  return *sensor;
}

void mech_sensors_set(Mech *mech, int primary, int secondary) {
  mech->rd.sensor[0] = clamp_int_to_char(primary);
  mech->rd.sensor[1] = clamp_int_to_char(secondary);
}

bool mech_is_fallen(const Mech *mech) {
  return mech_status_has(mech->rd.status, MECH_STATUS_FALLEN);
}

bool mech_is_jellied(const Mech *mech) {
  return mech_crit_status_has(mech->rd.critstatus, MECH_CRIT_STATUS_JELLIED);
}

void mech_jellied_set(Mech *mech, bool jellied) {
  if (jellied)
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_JELLIED);
  else
    mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_JELLIED);
}

void mech_sensor_visibility_modifier_set(Mech *mech, int modifier) {
  mech->rd.vis_mod = clamp_int_to_char(modifier);
}

bool mech_searchlight_active(const Mech *mech) {
  return mech_status2_has(mech->rd.status2, MECH_STATUS2_SLITE_ON);
}

bool mech_has_searchlight(const Mech *mech) {
  return (mech->rd.specials & SLITE_TECH) != 0;
}

bool mech_has_operational_beagle_probe(const Mech *mech) {
  return ((mech->rd.specials & BEAGLE_PROBE_TECH) &&
          !mech_crit_status_has(mech->rd.critstatus,
                                MECH_CRIT_STATUS_BEAGLE_DESTROYED)) != 0;
}

bool mech_has_operational_bloodhound_probe(const Mech *mech) {
  return ((mech->rd.specials2 & BLOODHOUND_PROBE_TECH) &&
          !mech_crit_status_has(mech->rd.critstatus,
                                MECH_CRIT_STATUS_BLOODHOUND_DESTROYED)) != 0;
}

bool mech_is_clairvoyant(const Mech *mech) {
  return mech_crit_status_has(mech->rd.critstatus,
                              MECH_CRIT_STATUS_CLAIRVOYANT);
}

bool mech_is_ecm_disturbed(const Mech *mech) {
  return mech_status2_has(mech->rd.status2, MECH_STATUS2_ECM_DISTURBANCE);
}

bool mech_is_any_ecm_disturbed(const Mech *mech) {
  return mech_status2_has(mech->rd.status2,
                          (MechStatus2)(MECH_STATUS2_ECM_DISTURBANCE |
                                        MECH_STATUS2_ANGEL_ECM_DISTURBED));
}

bool mech_electronic_warfare_is_enabled(const Mech *mech) {
  return mech_status2_has(mech->rd.status2,
                          (MechStatus2)(MECH_STATUS2_ECM_ENABLED |
                                        MECH_STATUS2_ECCM_ENABLED |
                                        MECH_STATUS2_ANGEL_ECM_ENABLED |
                                        MECH_STATUS2_ANGEL_ECCM_ENABLED));
}

bool mech_is_stealth_infantry(const Mech *mech) {
  return (mech->rd.infantry_specials & STEALTH_TECH) != 0;
}

bool mech_is_purifier_infantry(const Mech *mech) {
  return (mech->rd.infantry_specials & CS_PURIFIER_STEALTH_TECH) != 0;
}

int mech_sensor_visibility_modifier(const Mech *mech) {
  return mech->rd.vis_mod;
}

bool mech_has_tag_system(const Mech *mech) {
  return ((mech->rd.specials2 & TAG_TECH) ||
          (mech->rd.specials & C3_MASTER_TECH)) != 0;
}

bool mech_tag_system_is_destroyed(const Mech *mech) {
  return (((mech->rd.specials2 & TAG_TECH) &&
           mech_crit_status_has(mech->rd.critstatus,
                                MECH_CRIT_STATUS_TAG_DESTROYED)) ||
          ((mech->rd.specials & C3_MASTER_TECH) &&
           mech_crit_status_has(mech->rd.critstatus,
                                MECH_CRIT_STATUS_C3_DESTROYED))) != 0;
}

bool mech_has_working_ecm_suite(const Mech *mech) {
  return (((mech->rd.specials & ECM_TECH) &&
           !mech_crit_status_has(mech->rd.critstatus,
                                 MECH_CRIT_STATUS_ECM_DESTROYED)) ||
          ((mech->rd.specials2 & ANGEL_ECM_TECH) &&
           !mech_crit_status_has(mech->rd.critstatus,
                                 MECH_CRIT_STATUS_ANGEL_ECM_DESTROYED)) ||
          (mech->rd.infantry_specials & FC_INFILTRATORII_STEALTH_TECH)) != 0;
}

bool mech_supports_sensor_requirement(const SensorCapabilityRequest *request) {
  const int CAPABILITY = abs(request->signed_capability);
  const bool EQUIPPED =
      (request->capability_set == 1
           ? (request->mech->rd.specials & CAPABILITY) != 0
           : (request->mech->rd.specials2 & CAPABILITY) != 0) != 0;
  return (request->signed_capability > 0 ? EQUIPPED : !EQUIPPED) != 0;
}

bool mech_searchlight_warning_enabled(const Mech *mech) {
  return (mech->rd.mech_prefs & MECHPREF_SLWARN) != 0;
}

void mech_illumination_set(Mech *mech, bool illuminated) {
  if (illuminated)
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_SLITE_LIT);
  else
    mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_SLITE_LIT);
}

static void mech_status2_flag_set(Mech *mech, MechStatus2 flag, bool enabled) {
  if (enabled)
    mech_status2_set(&mech->rd.status2, flag);
  else
    mech_status2_clear(&mech->rd.status2, flag);
}

void mech_searchlight_active_set(Mech *mech, bool active) {
  mech_status2_flag_set(mech, MECH_STATUS2_SLITE_ON, active);
}

void mech_ecm_countered_set(Mech *mech, bool countered) {
  mech_status2_flag_set(mech, MECH_STATUS2_ECM_COUNTERED, countered);
}

void mech_ecm_protected_set(Mech *mech, bool protected) {
  mech_status2_flag_set(mech, MECH_STATUS2_ECM_PROTECTED, protected);
}

void mech_angel_ecm_protected_set(Mech *mech, bool protected) {
  mech_status2_flag_set(mech, MECH_STATUS2_ANGEL_ECM_PROTECTED, protected);
}

void mech_ecm_disturbed_set(Mech *mech, bool disturbed) {
  mech_status2_flag_set(mech, MECH_STATUS2_ECM_DISTURBANCE, disturbed);
}

void mech_angel_ecm_disturbed_set(Mech *mech, bool disturbed) {
  mech_status2_flag_set(mech, MECH_STATUS2_ANGEL_ECM_DISTURBED, disturbed);
}
