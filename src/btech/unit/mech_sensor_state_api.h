#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

int mech_sensor_index(const Mech *mech, int slot);
void mech_sensors_set(Mech *mech, int primary, int secondary);
bool mech_is_fallen(const Mech *mech);
bool mech_is_jellied(const Mech *mech);
void mech_jellied_set(Mech *mech, bool jellied);
bool mech_searchlight_active(const Mech *mech);
bool mech_has_searchlight(const Mech *mech);
bool mech_has_operational_beagle_probe(const Mech *mech);
bool mech_has_operational_bloodhound_probe(const Mech *mech);
bool mech_is_clairvoyant(const Mech *mech);
bool mech_is_ecm_disturbed(const Mech *mech);
bool mech_is_any_ecm_disturbed(const Mech *mech);
bool mech_electronic_warfare_is_enabled(const Mech *mech);
bool mech_is_stealth_infantry(const Mech *mech);
bool mech_is_purifier_infantry(const Mech *mech);
int mech_sensor_visibility_modifier(const Mech *mech);
void mech_sensor_visibility_modifier_set(Mech *mech, int modifier);
bool mech_has_tag_system(const Mech *mech);
bool mech_tag_system_is_destroyed(const Mech *mech);
bool mech_has_working_ecm_suite(const Mech *mech);
bool mech_supports_sensor_requirement(const Mech *mech, int capability_set,
                                      int signed_capability);
bool mech_searchlight_warning_enabled(const Mech *mech);
void mech_illumination_set(Mech *mech, bool illuminated);
void mech_searchlight_active_set(Mech *mech, bool active);
void mech_ecm_countered_set(Mech *mech, bool countered);
void mech_ecm_protected_set(Mech *mech, bool protected);
void mech_angel_ecm_protected_set(Mech *mech, bool protected);
void mech_ecm_disturbed_set(Mech *mech, bool disturbed);
void mech_angel_ecm_disturbed_set(Mech *mech, bool disturbed);
