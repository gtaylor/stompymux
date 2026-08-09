#include "template_internal.h"

#include "checked_conversion.h"
#include "mux/support/checked_storage.h"
#include <stddef.h>

int count_special_items() {
  return clamp_size_to_int(template_internal_name_count());
}

const char *section_configs[] = {"Case", "Destroyed", NULL};

const char *move_types[] = {"Biped", "Track", "Wheel", "Hover", "VTOL", "Hull",
                            "Foil",  "Fly",   "Quad",  "Sub",   "None", NULL};

const char *mech_types[] = {"Mech",        "Vehicle",           "VTOL",
                            "Naval",       "Spheroid_DropShip", "AeroFighter",
                            "Mechwarrior", "Aerodyne_DropShip", "Battlesuit",
                            NULL};

const char *crit_fire_modes[] = {
    "Destroyed", "Disabled",       "Broken",          "Damaged",
    "OnTC",      "RearMount",      "Hotload",         "Halfton",
    "OneShot",   "OneShot_Used",   "UltraMode",       "RapidFire",
    "Gattling",  "Rotary_TwoShot", "Rotary_FourShot", "Rotary_SixShot",
    "Heat",      "BackPack",       "Jettisoned",      "OmniBase",
    NULL};

const char *crit_ammo_modes[] = {"LBX/Cluster",
                                 "Artemis/Mine",
                                 "Narc/Smoke",
                                 "Cluster",
                                 "Mine",
                                 "Smoke",
                                 "Inferno",
                                 "Swarm",
                                 "Swarm1",
                                 "iNarc_Explosive",
                                 "iNarc_Haywire",
                                 "iNarc_ECM",
                                 "iNarc_Nemesis",
                                 "AP",
                                 "Flechette",
                                 "Incendiary",
                                 "Precision",
                                 "Stinger",
                                 "Caseless",
                                 "Sguided",
                                 "ExtendedRange",
                                 "HighExplosive",
                                 NULL};

/* 'specials' is *full* */
const char *specials[] = {"TripleMyomerTech",
                          "CL_AMS",
                          "IS_AMS",
                          "DoubleHS",
                          "Masc",
                          "Clan",
                          "FlipArms",
                          "C3MasterTech",
                          "C3SlaveTech",
                          "ArtemisIV",
                          "ECM",
                          "BeagleProbe",
                          "SalvageTech",
                          "CargoTech",
                          "SearchLight",
                          "LightBAP",
                          "AntiAircraft",
                          "NoSensors",
                          "SS_Ability",
                          "FerroFibrous_Tech",
                          "EndoSteel_Tech",
                          "XLEngine_Tech",
                          "ICEEngine_Tech",
                          "ForceSingleHS",
                          "LightEngine_Tech",
                          "XXL_Tech",
                          "CompactEngine_Tech",
                          "ReinforcedInternal_Tech",
                          "CompositeInternal_Tech",
                          "HardenedArmor_Tech",
                          "CritProof_Tech",
                          NULL};

const char *specialsabrev[] = {
    "TSM",  "CLAMS", "ISAMS", "DHS",  "MASC", "CLTECH", "FA",  "C3M",
    "C3S",  "AIV",   "ECM",   "BAP",  "SAL",  "CAR",    "SL",  "LBAP",
    "AA",   "NOSEN", "SS",    "FF",   "ES",   "XL",     "ICE", "SHS",
    "LENG", "XXL",   "CENG",  "RINT", "CINT", "HARM",   "CP",  NULL};
/* 'specials' is *full* */

const char *specials2[] = {
    "StealthArmor_Tech",  "HvyFerroFibrous_Tech", "LaserRefArmor_Tech",
    "ReactiveArmor_Tech", "NullSigSys_Tech",      "C3I_Tech",
    "SuperCharger_Tech",  "ImprovedJJ_Tech",      "MechanicalJJ_Tech",
    "CompactHS",          "LaserHS_Tech",         "BloodhoundProbe_Tech",
    "AngelECM_Tech",      "WatchDog_Tech",        "LtFerroFibrous_Tech",
    "TAG_Tech",           "OmniMech_Tech",        "ArtemisV_Tech",
    "Camo_Tech",          "Carrier_Tech",         "Waterproof_Tech",
    "XLGyro_Tech",        "HDGyro_Tech",          "CompactGyro_Tech",
    "TargComp_Tech",      "SmallCockpit_Tech",    NULL};

const char *specialsabrev2[] = {
    "STHA",   "HFF",    "LRARM", "REACTARM", "NULL",   "C3I",  "SCHARGE",
    "IJJ",    "MJJ",    "CHS",   "LHS",      "BLP",    "AECM", "WDOG",
    "LFF",    "TAG",    "OMNI",  "AV",       "CAMO",   "CART", "WPRF",
    "XLGYRO", "HDGYRO", "CGYRO", "TCOMP",    "SMCPIT", NULL};

const char *infantry_specials[] = {"Swarm_Attack_Tech",
                                   "Mount_Friends_Tech",
                                   "AntiLeg_Attack_Tech",
                                   "CS_Purifier_Stealth_Tech",
                                   "DC_Kage_Stealth_Tech",
                                   "FWL_Achileus_Stealth_Tech",
                                   "FC_Infiltrator_Stealth_Tech",
                                   "FC_InfiltratorII_Stealth_Tech",
                                   "Must_Jettison_Pack_Tech",
                                   "Can_Jettison_Pack_Tech",
                                   NULL};

const char *infspecialsabrev[] = {
    "SWARM",    "MFRIEND",  "ALEG",   "PSTEALTH", "KSTEALTH", "ASTEALTH",
    "ISTEALTH", "2STEALTH", "MJPACK", "CJPACK",   NULL};

size_t template_section_configuration_count(void) {
  return sizeof(section_configs) / sizeof(*section_configs) - 1;
}

size_t template_unit_class_count(void) {
  return sizeof(mech_types) / sizeof(*mech_types) - 1;
}

size_t template_movement_type_count(void) {
  return sizeof(move_types) / sizeof(*move_types) - 1;
}

size_t template_critical_fire_mode_count(void) {
  return sizeof(crit_fire_modes) / sizeof(*crit_fire_modes) - 1;
}

size_t template_critical_ammo_mode_count(void) {
  return sizeof(crit_ammo_modes) / sizeof(*crit_ammo_modes) - 1;
}

size_t primary_technology_name_count(void) {
  return sizeof(specials) / sizeof(*specials) - 1;
}

const char *primary_technology_name(size_t index) {
  return *(const char *const *)checked_storage_at_const(
      specials, primary_technology_name_count(), sizeof(*specials), index);
}

size_t secondary_technology_name_count(void) {
  return sizeof(specials2) / sizeof(*specials2) - 1;
}

const char *secondary_technology_name(size_t index) {
  return *(const char *const *)checked_storage_at_const(
      specials2, secondary_technology_name_count(), sizeof(*specials2), index);
}

size_t infantry_technology_name_count(void) {
  return sizeof(infantry_specials) / sizeof(*infantry_specials) - 1;
}

const char *infantry_technology_name(size_t index) {
  return *(const char *const *)checked_storage_at_const(
      infantry_specials, infantry_technology_name_count(),
      sizeof(*infantry_specials), index);
}

char *template_unit_class_name(size_t index) {
  return *(char *const *)checked_storage_at_const(
      mech_types, sizeof(mech_types) / sizeof(*mech_types) - 1,
      sizeof(*mech_types), index);
}

char *template_movement_type_name(size_t index) {
  return *(char *const *)checked_storage_at_const(
      move_types, sizeof(move_types) / sizeof(*move_types) - 1,
      sizeof(*move_types), index);
}
