typedef enum : int { STATUS_NONE = 0, STATUS_ACTIVE = 1 } MechStatus;
typedef enum : int { STATUS2_NONE = 0, STATUS2_ACTIVE = 1 } MechStatus2;
typedef enum : int { SPECIAL_NONE = 0, SPECIAL_ACTIVE = 1 } MechSpecialsStatus;
typedef enum : int { CRIT_NONE = 0, CRIT_ACTIVE = 1 } MechCritStatus;
typedef enum : int { TANK_NONE = 0, TANK_ACTIVE = 1 } MechTankCritStatus;
typedef enum : int { CRIT2_NONE = 0, CRIT2_ACTIVE = 1 } MechCritStatus2;
typedef MechStatus UnitStatus;

bool status_is_active(MechStatus status) {
  return (status & STATUS_ACTIVE) != 0;
}

bool status2_is_active(MechStatus2 status) { return (status & 1) != 0; }
bool special_is_active(MechSpecialsStatus status) { return (status & 1) != 0; }
bool crit_is_active(MechCritStatus status) { return (status & 1) != 0; }
bool tank_is_active(MechTankCritStatus status) { return (status & 1) != 0; }
bool crit2_is_active(MechCritStatus2 status) { return (status & 1) != 0; }
bool alias_is_active(UnitStatus status) { return (status & 1) != 0; }
