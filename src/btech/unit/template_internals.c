#include "template_internal.h"

#include "mux/support/checked_storage.h"
#include <stddef.h>

static const char *const INTERNAL_NAMES[] = {"ShoulderOrHip",
                                             "UpperActuator",
                                             "LowerActuator",
                                             "HandOrFootActuator",
                                             "LifeSupport",
                                             "Sensors",
                                             "Cockpit",
                                             "Engine",
                                             "Gyro",
                                             "HeatSink",
                                             "JumpJet",
                                             "CASE",
                                             "FerroFibrous",
                                             "EndoSteel",
                                             "TripleStrengthMyomer",
                                             "TargetingComputer",
                                             "Masc",
                                             "C3Master",
                                             "C3Slave",
                                             "BeagleProbe",
                                             "ArtemisIV",
                                             "Ecm",
                                             "Axe",
                                             "Sword",
                                             "Mace",
                                             "Claw",
                                             "DSAeroDoor",
                                             "DSMechDoor",
                                             "Fuel_Tank",
                                             "TAG",
                                             "DSVehicleDoor",
                                             "DSCargoDoor",
                                             "LAM_Equipment",
                                             "CASE-II",
                                             "StealthArmor",
                                             "NullSig_Device",
                                             "C3i",
                                             "AngelEcm",
                                             "HvyFerroFibrous",
                                             "LtFerroFibrous",
                                             "BloodhoundProbe",
                                             "PurifierArmor",
                                             "KageStealthUnit",
                                             "AchileusStealthUnit",
                                             "InfiltratorStealthUnit",
                                             "InfiltratorIIStealthUnit",
                                             "SuperCharger",
                                             "Dual_Saw",
                                             "Light_BAP",
                                             "SplitCrit_Left",
                                             "SplitCrit_Right",
                                             "Hardpoint",
                                             nullptr};

size_t template_internal_name_count(void) {
  return (sizeof(INTERNAL_NAMES) / sizeof(*INTERNAL_NAMES)) - 1;
}

const char *const *template_internal_names(void) { return INTERNAL_NAMES; }

const char *template_internal_name(int index) {
  if (index < 0)
    return nullptr;
  const char *const *name = (const char *const *)checked_storage_at_const(
      (const void *)INTERNAL_NAMES, template_internal_name_count(),
      sizeof(*INTERNAL_NAMES), (size_t)index);
  return *name;
}

const int INTERNALSWEIGHT[] = {
    102,  /* ShoulderOrHip */
    102,  /* UpperActuator */
    102,  /* LowerActuator */
    102,  /* HandOrFootActuator */
    204,  /* LifeSupport */
    102,  /* Sensors */
    512,  /* Cockpit */
    1024, /* Engine */
    512,  /* Gyro */
    204,  /* HeatSink */
    204,  /* JumpJet */
    51,   /* Case */
    153,  /* FerroFibrous */
    153,  /* EndoSteel */
    51,   /* TripleStrengthMyomer */
    204,  /* TargetingComputer */
    51,   /* Masc */
    512,  /* C3Master */
    102,  /* C3Slave */
    102,  /* BeagleProbe */
    153,  /* ArtemisIV */
    204,  /* Ecm */
    51,   /* Axe */
    25,   /* Sword */
    102,  /* Mace */
    75,   /* Claw */
    1024, /* DSAeroDoor */
    1024, /* DSMechDoor */
    102,  /* Fuel_Tank */
    51,   /* TAG */
    1024, /* DSVehicleDoor */
    1024, /* DSCargoDoor */
    306,  /* LAM_Equipment */
    306,  /* CaseII */
    3,    /* StealthArmor */
    1024, /* NullSig_Device */
    512,  /* C3i */
    204,  /* AngelECM */
    7,    /* HvyFerroFibrous */
    1,    /* LtFerroFibrous */
    102,  /* BloodhoundProbe */
    2,    /* PurifierArmor */
    102,  /* KageStealthUnit */
    102,  /* AchileusStealthUnit */
    102,  /* InfiltratorStealthUnit */
    102,  /* InfiltratorIIStealthUnit */
    1024, /* Dual_Saw */
    512,  /* Light_BAP */
};

const int TEMPLATE_INTERNAL_COUNT =
    (sizeof(INTERNAL_NAMES) / sizeof(*INTERNAL_NAMES)) - 1;
