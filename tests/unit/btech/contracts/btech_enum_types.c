#include "autopilot.h"
#include "btech_event.h"
#include "failures.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_physical_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mine_api.h"
#include "registry_internal.h"
#include "template_internal.h"

static_assert(CLASS_MECH == 0 && CLASS_VTOL == 2 && CLASS_VEH_NAVAL == 3 &&
              CLASS_BSUIT == 8);
static_assert(MOVE_BIPED == 0 && MOVE_VTOL == 4 && MOVE_QUAD == 8 &&
              MOVE_SUB == 9 && MOVE_NONE == 10);
static_assert(EVENT_MOVE == 1 && EVENT_AMMOWARN == 22 &&
              EVENT_REPAIR_REPL == 43 && EVENT_REPAIR_REPENHCRIT == 58 &&
              EVENT_CHANGING_HULLDOWN == 74 && EVENT_SCHARGE_FAIL == 76 &&
              EVENT_SIDESLIP == 80);
static_assert(RANGE_SHORT == 0 && RANGE_NOWATER == 5);
static_assert(PA_PUNCH == 1 && PA_CLAW == 9);
static_assert(MINE_STEP == 1 && MINE_DROP == 4);
static_assert(ARMOR_LEVEL_GREAT == 0 && ARMOR_LEVEL_REPAIRING == 5);
static_assert(TARGCOMP_NORMAL == 0 && TARGCOMP_AA == 4);
static_assert(AUTO_CHASETARGET_ON == 1 && AUTO_CHASETARGET_SAVE == 4);
static_assert(VERIFY == 0 && SAVE == 1 && LOAD == 2);
static_assert(SPECIAL_FREE == 0 && SPECIAL_ALLOC == 1);
static_assert(MODE_UNKNOWN == 0 && MODE_NORMAL == 1);
static_assert(HEAT == 1 && CRAZY_MISSILES == 7);
static_assert(FAIL_NONE == 0 && FAIL_AMMOCRITJAMMED == 7);
static_assert(BROKEN_MODE == 0x00000004 && ROCKET_FIRED == 0x00100000);
static_assert(RAC_MODES == 0x0000E000 && FIRE_MODES == 0x0001FC40);
static_assert(ARTILLERY_MODES == 0x00000038 && INARC_MODES == 0x00001E00);
static_assert(AC_MODES == 0x0005E000 && ATM_MODES == 0x00300000);
static_assert(AMMO_MODES == 0x007FFFFF);
static_assert(WEAP_DAM_EN_FOCUS == 0x00000002 &&
              WEAP_DAM_MSL_AMMO == 0x00000040);
static_assert(SECTION_FLOODED == 0x08 && STABILIZERS_DESTROYED == 0x20);
static_assert(INARC_HOMING_ATTACHED == 0x00000002 &&
              INARC_NEMESIS_ATTACHED == 0x00000010);
static_assert(MECHPREF_AUTOFALL == 0x00000004 &&
              MECHPREF_BTHDEBUG == 0x00000200);
static_assert(sizeof(MechStatus) == sizeof(int) &&
              sizeof(MechStatus2) == sizeof(int) &&
              sizeof(MechSpecialsStatus) == sizeof(int) &&
              sizeof(MechCritStatus) == sizeof(int) &&
              sizeof(MechTankCritStatus) == sizeof(int) &&
              sizeof(MechCritStatus2) == sizeof(int));
static_assert(MECH_STATUS_CONDITIONS == 0x78000000 &&
              MECH_STATUS_LOCK_MODES == 0x000F8000 &&
              MECH_STATUS2_MOVE_MODES == 0x00070000 &&
              MECH_STATUS2_MOVE_MODES_LOCK == 0x00030000);
static_assert(_Generic(mech_status_mask(MECH_STATUS_STARTED,
                                        MECH_STATUS_STARTED),
                  MechStatus: 1,
                  default: 0));
static_assert(_Generic(mech_status2_mask(MECH_STATUS2_SPRINTING,
                                         MECH_STATUS2_SPRINTING),
                  MechStatus2: 1,
                  default: 0));
static_assert(_Generic(mech_crit_status_mask(MECH_CRIT_STATUS_HIDDEN,
                                             MECH_CRIT_STATUS_HIDDEN),
                  MechCritStatus: 1,
                  default: 0));
static_assert(_Generic(mech_tank_crit_status_mask(MECH_TANK_CRIT_STATUS_DUG_IN,
                                                  MECH_TANK_CRIT_STATUS_DUG_IN),
                  MechTankCritStatus: 1,
                  default: 0));
static_assert(_Generic(mech_crit_status2_mask(MECH_CRIT_STATUS2_HDGYRO_DAMAGED,
                                              MECH_CRIT_STATUS2_HDGYRO_DAMAGED),
                  MechCritStatus2: 1,
                  default: 0));

static_assert(_Generic(mech_class((const Mech *)nullptr),
                  UnitClass: 1,
                  default: 0));
static_assert(_Generic(mech_movement_type((const Mech *)nullptr),
                  MechMovementType: 1,
                  default: 0));
static_assert(_Generic(mech_targeting_computer_type((const Mech *)nullptr),
                  TargetingComputerType: 1,
                  default: 0));
static_assert(_Generic(armor_evaluate(&(ArmorEvaluationRequest){0}),
                  ArmorEvaluation: 1,
                  default: 0));
static_assert(_Generic(mech_range_to_hit_calculate(&(WeaponRangeToHitRequest){
                           0}),
                  WeaponRangeToHitResult: 1,
                  default: 0));
static_assert(_Generic(&mine_field_trigger,
                  void (*)(Mech *, MineTriggerReason): 1,
                  default: 0));
static_assert(_Generic(&physical_attack_verb,
                  const char *(*)(const PhysicalVerbRequest *): 1,
                  default: 0));
static_assert(_Generic(&mech_event_cancel,
                  void (*)(Mech *, MechEventType): 1,
                  default: 0));

int main(void) {
  MechStatus status = MECH_STATUS_NONE;
  mech_status_set(&status,
                  (MechStatus)(MECH_STATUS_STARTED | MECH_STATUS_LOCK_TARGET));
  if (!mech_status_has(status, MECH_STATUS_STARTED) ||
      !mech_status_has(status, MECH_STATUS_LOCK_TARGET) ||
      mech_status_has(status, MECH_STATUS_DESTROYED) ||
      mech_status_mask(status, MECH_STATUS_LOCK_MODES) !=
          MECH_STATUS_LOCK_TARGET)
    return 1;
  mech_status_clear(&status, MECH_STATUS_STARTED);
  if (mech_status_has(status, MECH_STATUS_STARTED) ||
      !mech_status_has(status, MECH_STATUS_LOCK_TARGET))
    return 2;

  MechStatus2 status2 = MECH_STATUS2_NONE;
  mech_status2_set(&status2, MECH_STATUS2_MOVE_MODES);
  if (!mech_status2_has(status2, MECH_STATUS2_SPRINTING) ||
      !mech_status2_has(status2, MECH_STATUS2_EVADING) ||
      !mech_status2_has(status2, MECH_STATUS2_DODGING))
    return 3;
  mech_status2_clear(&status2, MECH_STATUS2_MOVE_MODES_LOCK);
  if (mech_status2_has(status2, MECH_STATUS2_SPRINTING) ||
      mech_status2_has(status2, MECH_STATUS2_EVADING) ||
      !mech_status2_has(status2, MECH_STATUS2_DODGING))
    return 4;

  MechSpecialsStatus specials_status = MECH_SPECIALS_STATUS_NONE;
  if (mech_specials_status_has(specials_status, MECH_SPECIALS_STATUS_NONE) ||
      mech_specials_status_mask(specials_status, MECH_SPECIALS_STATUS_NONE) !=
          MECH_SPECIALS_STATUS_NONE)
    return 5;
  mech_specials_status_set(&specials_status, MECH_SPECIALS_STATUS_NONE);
  mech_specials_status_clear(&specials_status, MECH_SPECIALS_STATUS_NONE);

  MechCritStatus crit_status = MECH_CRIT_STATUS_NONE;
  mech_crit_status_set(&crit_status, MECH_CRIT_STATUS_HIDDEN);
  if (!mech_crit_status_has(crit_status, MECH_CRIT_STATUS_HIDDEN))
    return 6;
  mech_crit_status_clear(&crit_status, MECH_CRIT_STATUS_HIDDEN);
  if (mech_crit_status_has(crit_status, MECH_CRIT_STATUS_HIDDEN))
    return 7;

  MechTankCritStatus tank_crit_status = MECH_TANK_CRIT_STATUS_NONE;
  mech_tank_crit_status_set(&tank_crit_status,
                            MECH_TANK_CRIT_STATUS_TURRET_JAMMED);
  if (!mech_tank_crit_status_has(tank_crit_status,
                                 MECH_TANK_CRIT_STATUS_TURRET_JAMMED))
    return 8;
  mech_tank_crit_status_clear(&tank_crit_status,
                              MECH_TANK_CRIT_STATUS_TURRET_JAMMED);
  if (mech_tank_crit_status_has(tank_crit_status,
                                MECH_TANK_CRIT_STATUS_TURRET_JAMMED))
    return 9;

  MechCritStatus2 crit_status2 = MECH_CRIT_STATUS2_NONE;
  mech_crit_status2_set(&crit_status2, MECH_CRIT_STATUS2_HDGYRO_DAMAGED);
  if (!mech_crit_status2_has(crit_status2, MECH_CRIT_STATUS2_HDGYRO_DAMAGED))
    return 10;
  mech_crit_status2_clear(&crit_status2, MECH_CRIT_STATUS2_HDGYRO_DAMAGED);
  if (mech_crit_status2_has(crit_status2, MECH_CRIT_STATUS2_HDGYRO_DAMAGED))
    return 11;

  return 0;
}
