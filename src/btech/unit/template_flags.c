#include "template_internal.h"

#include "checked_conversion.h"
#include "mux/support/checked_storage.h"
#include <stddef.h>

int count_special_items() {
  return clamp_size_to_int(template_internal_name_count());
}

static const char *const SECTION_CONFIGURATION_NAMES[] = {"Case", "Destroyed",
                                                          nullptr};

static const char *const MOVEMENT_TYPE_NAMES[] = {
    "Biped", "Track", "Wheel", "Hover", "VTOL", "Hull",
    "Foil",  "Fly",   "Quad",  "Sub",   "None", nullptr};

static const char *const UNIT_CLASS_NAMES[] = {
    "Mech",        "Vehicle",           "VTOL",
    "Naval",       "Spheroid_DropShip", "AeroFighter",
    "Mechwarrior", "Aerodyne_DropShip", "Battlesuit",
    nullptr};

static const char *const CRITICAL_FIRE_MODE_NAMES[] = {
    "Destroyed", "Disabled",       "Broken",          "Damaged",
    "OnTC",      "RearMount",      "Hotload",         "Halfton",
    "OneShot",   "OneShot_Used",   "UltraMode",       "RapidFire",
    "Gattling",  "Rotary_TwoShot", "Rotary_FourShot", "Rotary_SixShot",
    "Heat",      "BackPack",       "Jettisoned",      "OmniBase",
    NULL};

static const char *const CRITICAL_AMMO_MODE_NAMES[] = {"LBX/Cluster",
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
static const char *const PRIMARY_TECHNOLOGY_NAMES[] = {
    "TripleMyomerTech",
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

static const char *const PRIMARY_TECHNOLOGY_ABBREVIATIONS[] = {
    "TSM",  "CLAMS", "ISAMS", "DHS",  "MASC", "CLTECH", "FA",  "C3M",
    "C3S",  "AIV",   "ECM",   "BAP",  "SAL",  "CAR",    "SL",  "LBAP",
    "AA",   "NOSEN", "SS",    "FF",   "ES",   "XL",     "ICE", "SHS",
    "LENG", "XXL",   "CENG",  "RINT", "CINT", "HARM",   "CP",  NULL};
/* 'specials' is *full* */

static const char *const SECONDARY_TECHNOLOGY_NAMES[] = {
    "StealthArmor_Tech",  "HvyFerroFibrous_Tech", "LaserRefArmor_Tech",
    "ReactiveArmor_Tech", "NullSigSys_Tech",      "C3I_Tech",
    "SuperCharger_Tech",  "ImprovedJJ_Tech",      "MechanicalJJ_Tech",
    "CompactHS",          "LaserHS_Tech",         "BloodhoundProbe_Tech",
    "AngelECM_Tech",      "WatchDog_Tech",        "LtFerroFibrous_Tech",
    "TAG_Tech",           "OmniMech_Tech",        "ArtemisV_Tech",
    "Camo_Tech",          "Carrier_Tech",         "Waterproof_Tech",
    "XLGyro_Tech",        "HDGyro_Tech",          "CompactGyro_Tech",
    "TargComp_Tech",      "SmallCockpit_Tech",    NULL};

static const char *const SECONDARY_TECHNOLOGY_ABBREVIATIONS[] = {
    "STHA",   "HFF",    "LRARM", "REACTARM", "NULL",   "C3I",  "SCHARGE",
    "IJJ",    "MJJ",    "CHS",   "LHS",      "BLP",    "AECM", "WDOG",
    "LFF",    "TAG",    "OMNI",  "AV",       "CAMO",   "CART", "WPRF",
    "XLGYRO", "HDGYRO", "CGYRO", "TCOMP",    "SMCPIT", NULL};

static const char *const INFANTRY_TECHNOLOGY_NAMES[] = {
    "Swarm_Attack_Tech",
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

static const char *const INFANTRY_TECHNOLOGY_ABBREVIATIONS[] = {
    "SWARM",    "MFRIEND",  "ALEG",   "PSTEALTH", "KSTEALTH", "ASTEALTH",
    "ISTEALTH", "2STEALTH", "MJPACK", "CJPACK",   NULL};

size_t template_section_configuration_count(void) {
  return (sizeof(SECTION_CONFIGURATION_NAMES) /
          sizeof(*SECTION_CONFIGURATION_NAMES)) -
         1;
}

size_t template_unit_class_count(void) {
  return (sizeof(UNIT_CLASS_NAMES) / sizeof(*UNIT_CLASS_NAMES)) - 1;
}

size_t template_movement_type_count(void) {
  return (sizeof(MOVEMENT_TYPE_NAMES) / sizeof(*MOVEMENT_TYPE_NAMES)) - 1;
}

size_t template_critical_fire_mode_count(void) {
  return (sizeof(CRITICAL_FIRE_MODE_NAMES) /
          sizeof(*CRITICAL_FIRE_MODE_NAMES)) -
         1;
}

size_t template_critical_ammo_mode_count(void) {
  return (sizeof(CRITICAL_AMMO_MODE_NAMES) /
          sizeof(*CRITICAL_AMMO_MODE_NAMES)) -
         1;
}

size_t primary_technology_name_count(void) {
  return (sizeof(PRIMARY_TECHNOLOGY_NAMES) /
          sizeof(*PRIMARY_TECHNOLOGY_NAMES)) -
         1;
}

const char *primary_technology_name(size_t index) {
  return *(const char *const *)checked_storage_at_const(
      (const void *)PRIMARY_TECHNOLOGY_NAMES, primary_technology_name_count(),
      sizeof(*PRIMARY_TECHNOLOGY_NAMES), index);
}

size_t secondary_technology_name_count(void) {
  return (sizeof(SECONDARY_TECHNOLOGY_NAMES) /
          sizeof(*SECONDARY_TECHNOLOGY_NAMES)) -
         1;
}

const char *secondary_technology_name(size_t index) {
  return *(const char *const *)checked_storage_at_const(
      (const void *)SECONDARY_TECHNOLOGY_NAMES,
      secondary_technology_name_count(), sizeof(*SECONDARY_TECHNOLOGY_NAMES),
      index);
}

size_t infantry_technology_name_count(void) {
  return (sizeof(INFANTRY_TECHNOLOGY_NAMES) /
          sizeof(*INFANTRY_TECHNOLOGY_NAMES)) -
         1;
}

const char *infantry_technology_name(size_t index) {
  return *(const char *const *)checked_storage_at_const(
      (const void *)INFANTRY_TECHNOLOGY_NAMES, infantry_technology_name_count(),
      sizeof(*INFANTRY_TECHNOLOGY_NAMES), index);
}

const char *template_unit_class_name(size_t index) {
  return *(const char *const *)checked_storage_at_const(
      (const void *)UNIT_CLASS_NAMES, template_unit_class_count(),
      sizeof(*UNIT_CLASS_NAMES), index);
}

const char *template_movement_type_name(size_t index) {
  return *(const char *const *)checked_storage_at_const(
      (const void *)MOVEMENT_TYPE_NAMES, template_movement_type_count(),
      sizeof(*MOVEMENT_TYPE_NAMES), index);
}

const char *const *template_section_configuration_names(void) {
  return SECTION_CONFIGURATION_NAMES;
}

const char *const *template_movement_type_names(void) {
  return MOVEMENT_TYPE_NAMES;
}

const char *const *template_unit_class_names(void) { return UNIT_CLASS_NAMES; }

const char *const *template_critical_fire_mode_names(void) {
  return CRITICAL_FIRE_MODE_NAMES;
}

const char *const *template_critical_ammo_mode_names(void) {
  return CRITICAL_AMMO_MODE_NAMES;
}

const char *const *primary_technology_names(void) {
  return PRIMARY_TECHNOLOGY_NAMES;
}

const char *const *primary_technology_abbreviations(void) {
  return PRIMARY_TECHNOLOGY_ABBREVIATIONS;
}

const char *const *secondary_technology_names(void) {
  return SECONDARY_TECHNOLOGY_NAMES;
}

const char *const *secondary_technology_abbreviations(void) {
  return SECONDARY_TECHNOLOGY_ABBREVIATIONS;
}

const char *const *infantry_technology_names(void) {
  return INFANTRY_TECHNOLOGY_NAMES;
}

const char *const *infantry_technology_abbreviations(void) {
  return INFANTRY_TECHNOLOGY_ABBREVIATIONS;
}
