#include "mech_sensor_state_api.h"

#include <stdlib.h>

#include "checked_conversion.h"
#include "mech_internal.h"
#include "mech_status_types.h"

int mech_sensor_index(const Mech *mech, int slot) {
  return mech->rd.sensor[slot];
}

void mech_sensors_set(Mech *mech, int primary, int secondary) {
  mech->rd.sensor[0] = clamp_int_to_char(primary);
  mech->rd.sensor[1] = clamp_int_to_char(secondary);
}

bool mech_is_fallen(const Mech *mech) { return mech->rd.status & FALLEN; }

bool mech_is_jellied(const Mech *mech) { return mech->rd.critstatus & JELLIED; }

void mech_jellied_set(Mech *mech, bool jellied) {
  if (jellied)
    mech->rd.critstatus |= JELLIED;
  else
    mech->rd.critstatus &= ~JELLIED;
}

void mech_sensor_visibility_modifier_set(Mech *mech, int modifier) {
  mech->rd.vis_mod = clamp_int_to_char(modifier);
}

bool mech_searchlight_active(const Mech *mech) {
  return mech->rd.status2 & SLITE_ON;
}

bool mech_has_searchlight(const Mech *mech) {
  return mech->rd.specials & SLITE_TECH;
}

bool mech_has_operational_beagle_probe(const Mech *mech) {
  return (mech->rd.specials & BEAGLE_PROBE_TECH) &&
         !(mech->rd.critstatus & BEAGLE_DESTROYED);
}

bool mech_has_operational_bloodhound_probe(const Mech *mech) {
  return (mech->rd.specials2 & BLOODHOUND_PROBE_TECH) &&
         !(mech->rd.critstatus & BLOODHOUND_DESTROYED);
}

bool mech_is_clairvoyant(const Mech *mech) {
  return mech->rd.critstatus & CLAIRVOYANT;
}

bool mech_is_ecm_disturbed(const Mech *mech) {
  return mech->rd.status2 & ECM_DISTURBANCE;
}

bool mech_is_any_ecm_disturbed(const Mech *mech) {
  return mech->rd.status2 & (ECM_DISTURBANCE | ANGEL_ECM_DISTURBED);
}

bool mech_electronic_warfare_is_enabled(const Mech *mech) {
  return mech->rd.status2 &
         (ECM_ENABLED | ECCM_ENABLED | ANGEL_ECM_ENABLED | ANGEL_ECCM_ENABLED);
}

bool mech_is_stealth_infantry(const Mech *mech) {
  return mech->rd.infantry_specials & STEALTH_TECH;
}

bool mech_is_purifier_infantry(const Mech *mech) {
  return mech->rd.infantry_specials & CS_PURIFIER_STEALTH_TECH;
}

int mech_sensor_visibility_modifier(const Mech *mech) {
  return mech->rd.vis_mod;
}

bool mech_has_tag_system(const Mech *mech) {
  return (mech->rd.specials2 & TAG_TECH) ||
         (mech->rd.specials & C3_MASTER_TECH);
}

bool mech_tag_system_is_destroyed(const Mech *mech) {
  return ((mech->rd.specials2 & TAG_TECH) &&
          (mech->rd.critstatus & TAG_DESTROYED)) ||
         ((mech->rd.specials & C3_MASTER_TECH) &&
          (mech->rd.critstatus & C3_DESTROYED));
}

bool mech_has_working_ecm_suite(const Mech *mech) {
  return ((mech->rd.specials & ECM_TECH) &&
          !(mech->rd.critstatus & ECM_DESTROYED)) ||
         ((mech->rd.specials2 & ANGEL_ECM_TECH) &&
          !(mech->rd.critstatus & ANGEL_ECM_DESTROYED)) ||
         (mech->rd.infantry_specials & FC_INFILTRATORII_STEALTH_TECH);
}

bool mech_supports_sensor_requirement(const Mech *mech, int capability_set,
                                      int signed_capability) {
  const int capability = abs(signed_capability);
  const bool equipped = capability_set == 1
                            ? (mech->rd.specials & capability) != 0
                            : (mech->rd.specials2 & capability) != 0;
  return signed_capability > 0 ? equipped : !equipped;
}

bool mech_searchlight_warning_enabled(const Mech *mech) {
  return mech->rd.mech_prefs & MECHPREF_SLWARN;
}

void mech_illumination_set(Mech *mech, bool illuminated) {
  if (illuminated)
    mech->rd.critstatus |= SLITE_LIT;
  else
    mech->rd.critstatus &= ~SLITE_LIT;
}

static void mech_status2_flag_set(Mech *mech, int flag, bool enabled) {
  if (enabled)
    mech->rd.status2 |= flag;
  else
    mech->rd.status2 &= ~flag;
}

void mech_searchlight_active_set(Mech *mech, bool active) {
  mech_status2_flag_set(mech, SLITE_ON, active);
}

void mech_ecm_countered_set(Mech *mech, bool countered) {
  mech_status2_flag_set(mech, ECM_COUNTERED, countered);
}

void mech_ecm_protected_set(Mech *mech, bool protected) {
  mech_status2_flag_set(mech, ECM_PROTECTED, protected);
}

void mech_angel_ecm_protected_set(Mech *mech, bool protected) {
  mech_status2_flag_set(mech, ANGEL_ECM_PROTECTED, protected);
}

void mech_ecm_disturbed_set(Mech *mech, bool disturbed) {
  mech_status2_flag_set(mech, ECM_DISTURBANCE, disturbed);
}

void mech_angel_ecm_disturbed_set(Mech *mech, bool disturbed) {
  mech_status2_flag_set(mech, ANGEL_ECM_DISTURBED, disturbed);
}
