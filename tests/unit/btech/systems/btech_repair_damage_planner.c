#include "btech/context.h"
#include "equipment_types.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_damages_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"
#include "repair_job.h"

#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#pragma clang diagnostic ignored "-Wunsafe-buffer-usage"

typedef struct SectionState {
  int armor;
  int original_armor;
  int rear_armor;
  int original_rear_armor;
  int internal;
  int original_internal;
  bool destroyed;
  bool flooded;
} SectionState;

typedef struct CriticalState {
  int part_type;
  int data;
  int full_ammunition;
  int damage_flags;
  bool destroyed;
  bool nonfunctional;
  bool temporary_failure;
  bool damaged;
  bool broken;
} CriticalState;

static SectionState sections[NUM_SECTIONS];
static CriticalState criticals[NUM_SECTIONS][NUM_CRITICALS];
static UnitClass unit_class;
static bool fixable;
static bool scrapping_part;
static char dispatched[8][32];
static size_t dispatch_count;
static char notification[128];

static void reset_state(void) {
  memset(sections, 0, sizeof(sections));
  memset(criticals, 0, sizeof(criticals));
  memset(dispatched, 0, sizeof(dispatched));
  memset(notification, 0, sizeof(notification));
  dispatch_count = 0;
  unit_class = CLASS_MECH;
  fixable = true;
  scrapping_part = false;
  for (int section = 0; section < NUM_SECTIONS; section++) {
    sections[section].internal = 1;
    sections[section].original_internal = 1;
  }
}

static void record_dispatch(const char *command, const char *argument) {
  if (dispatch_count >= sizeof(dispatched) / sizeof(*dispatched))
    return;
  (void)snprintf(dispatched[dispatch_count], sizeof(dispatched[0]), "%s:%s",
                 command, argument);
  dispatch_count++;
}

UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return unit_class; }
MechMovementType mech_movement_type(const Mech *mech [[maybe_unused]]) {
  return MOVE_BIPED;
}
int mech_section_armor(const Mech *mech [[maybe_unused]], int section) {
  return sections[section].armor;
}
int mech_section_original_armor(const Mech *mech [[maybe_unused]],
                                int section) {
  return sections[section].original_armor;
}
int mech_section_rear_armor(const Mech *mech [[maybe_unused]], int section) {
  return sections[section].rear_armor;
}
int mech_section_original_rear_armor(const Mech *mech [[maybe_unused]],
                                     int section) {
  return sections[section].original_rear_armor;
}
int mech_section_internal(const Mech *mech [[maybe_unused]], int section) {
  return sections[section].internal;
}
int mech_section_original_internal(const Mech *mech [[maybe_unused]],
                                   int section) {
  return sections[section].original_internal;
}
bool mech_section_is_destroyed(const Mech *mech [[maybe_unused]], int section) {
  return sections[section].destroyed;
}
bool mech_section_is_flooded(const Mech *mech [[maybe_unused]], int section) {
  return sections[section].flooded;
}
int mech_critical_part_type(const Mech *mech [[maybe_unused]], int section,
                            int critical) {
  return criticals[section][critical].part_type;
}
int mech_critical_data(const Mech *mech [[maybe_unused]], int section,
                       int critical) {
  return criticals[section][critical].data;
}
int mech_critical_full_ammunition(const Mech *mech [[maybe_unused]],
                                  int section, int critical) {
  return criticals[section][critical].full_ammunition;
}
int full_ammo(const Mech *mech [[maybe_unused]], int section, int critical) {
  return criticals[section][critical].full_ammunition;
}
int mech_critical_damage_flags(const Mech *mech [[maybe_unused]], int section,
                               int critical) {
  return criticals[section][critical].damage_flags;
}
bool mech_critical_is_destroyed(const Mech *mech [[maybe_unused]], int section,
                                int critical) {
  return criticals[section][critical].destroyed;
}
bool mech_critical_is_nonfunctional(const Mech *mech [[maybe_unused]],
                                    int section, int critical) {
  const CriticalState state = criticals[section][critical];
  return state.nonfunctional || state.broken || state.destroyed;
}
int mech_critical_temporary_failure(const Mech *mech [[maybe_unused]],
                                    int section, int critical) {
  return criticals[section][critical].temporary_failure;
}
bool mech_critical_is_damaged(const Mech *mech [[maybe_unused]], int section,
                              int critical) {
  return criticals[section][critical].damaged;
}
bool mech_critical_is_broken(const Mech *mech [[maybe_unused]], int section,
                             int critical) {
  return criticals[section][critical].broken;
}
bool mech_part_is_structural_placeholder(int part_type [[maybe_unused]]) {
  return false;
}
int get_weapon_crits(Mech *mech [[maybe_unused]], int weapon [[maybe_unused]]) {
  return 1;
}
ArmorSectionAbbreviation
armor_section_abbreviation(const ArmorSectionReference *section) {
  ArmorSectionAbbreviation result = {0};
  (void)snprintf(result.text, sizeof(result.text), "S%d", section->location);
  return result;
}
PartDisplayName pos_part_name(Mech *mech [[maybe_unused]], int section,
                              int critical) {
  PartDisplayName result = {.valid = true};
  (void)snprintf(result.text, sizeof(result.text), "P%d-%d", section, critical);
  return result;
}
bool unit_is_fixable(Mech *mech [[maybe_unused]]) { return fixable; }
bool someone_repairing(Mech *mech [[maybe_unused]],
                       int section [[maybe_unused]],
                       int critical [[maybe_unused]]) {
  return false;
}
int someone_attaching(Mech *mech [[maybe_unused]],
                      int section [[maybe_unused]]) {
  return 0;
}
int someone_resealing(Mech *mech [[maybe_unused]],
                      int section [[maybe_unused]]) {
  return 0;
}
int someone_replacing_suit(Mech *mech [[maybe_unused]],
                           int section [[maybe_unused]]) {
  return 0;
}
bool someone_fixing(Mech *mech [[maybe_unused]], int section [[maybe_unused]]) {
  return false;
}
int someone_scrapping_loc(Mech *mech [[maybe_unused]],
                          int section [[maybe_unused]]) {
  return 0;
}
bool someone_scrapping_part(Mech *mech [[maybe_unused]],
                            int section [[maybe_unused]],
                            int critical [[maybe_unused]]) {
  return scrapping_part;
}
bool invalid_scrap_path(Mech *mech [[maybe_unused]],
                        int section [[maybe_unused]]) {
  return false;
}
RepairCommandStatus repair_command_context_initialize(
    DbRef player [[maybe_unused]], void *mech_data,
    RepairStallPolicy policy [[maybe_unused]], RepairCommandContext *context) {
  context->mech = mech_data;
  context->evaluation = (EvaluationContext *)1;
  return REPAIR_COMMAND_READY;
}
const char *repair_command_status_message(RepairCommandStatus status
                                          [[maybe_unused]]) {
  return "unavailable";
}
BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return (BtechContext *)1;
}
EvaluationContext *btech_context_evaluation(BtechContext *context
                                            [[maybe_unused]]) {
  return (EvaluationContext *)1;
}
void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]], const char *message) {
  (void)snprintf(notification, sizeof(notification), "%s", message);
}

void tech_repairgun(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                    char *argument) {
  record_dispatch("tech_repairgun", argument);
}
void tech_replacegun(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                     char *argument) {
  record_dispatch("tech_replacegun", argument);
}
void tech_replacepart(DbRef player [[maybe_unused]],
                      Mech *mech [[maybe_unused]], char *argument) {
  record_dispatch("tech_replacepart", argument);
}
void tech_reload(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                 char *argument) {
  record_dispatch("tech_reload", argument);
}
void tech_reattach(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                   char *argument) {
  record_dispatch("tech_reattach", argument);
}
void tech_reseal(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                 char *argument) {
  record_dispatch("tech_reseal", argument);
}
void tech_fixarmor(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                   char *argument) {
  record_dispatch("tech_fixarmor", argument);
}
void tech_fixinternal(DbRef player [[maybe_unused]],
                      Mech *mech [[maybe_unused]], char *argument) {
  record_dispatch("tech_fixinternal", argument);
}
void tech_removesection(DbRef player [[maybe_unused]],
                        Mech *mech [[maybe_unused]], char *argument) {
  record_dispatch("tech_removesection", argument);
}
void tech_removepart(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                     char *argument) {
  record_dispatch("tech_removepart", argument);
}
void tech_removegun(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                    char *argument) {
  record_dispatch("tech_removegun", argument);
}
void tech_unload(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                 char *argument) {
  record_dispatch("tech_unload", argument);
}
void tech_replacesuit(DbRef player [[maybe_unused]],
                      Mech *mech [[maybe_unused]], char *argument) {
  record_dispatch("tech_replacesuit", argument);
}
void tech_fixenhcrit(DbRef player [[maybe_unused]], void *mech [[maybe_unused]],
                     char *argument) {
  record_dispatch("tech_fixenhcrit", argument);
}

static bool test_damage_ordering_and_details(void) {
  Mech *const mech = (Mech *)1;
  char jobs[2048];
  reset_state();
  sections[CTORSO].armor = 3;
  sections[CTORSO].original_armor = 5;
  sections[CTORSO].rear_armor = 1;
  sections[CTORSO].original_rear_armor = 2;
  criticals[CTORSO][0] =
      (CriticalState){.part_type = ammunition_equipment_index(3),
                      .data = 2,
                      .full_ammunition = 6};
  criticals[CTORSO][1] = (CriticalState){.part_type = weapon_equipment_index(4),
                                         .damaged = true,
                                         .damage_flags = WEAP_DAM_EN_FOCUS};
  criticals[LTORSO][0] = (CriticalState){.part_type = weapon_equipment_index(5),
                                         .temporary_failure = true};
  criticals[RTORSO][0] =
      (CriticalState){.part_type = 600, .destroyed = true, .broken = true};
  sections[LLEG].internal = 2;
  sections[LLEG].original_internal = 3;
  sections[LLEG].armor = 1;
  sections[LLEG].original_armor = 3;

  mech_repair_jobs_format(mech, jobs, sizeof(jobs));
  if (mech_repair_job_count(mech) != 7 ||
      strcmp(jobs, "1|S4|12|2|0,2|S4|13|1|0,3|S4|11|P4-0:4|0,"
                   "4|S4|4|P4-1|0,5|S2|2|P2-0|0,6|S3|1|P3-0|0,"
                   "7|S5|14|1|0") != 0) {
    fprintf(stderr, "ordering count=%zu jobs=%s\n", mech_repair_job_count(mech),
            jobs);
    return false;
  }
  return true;
}

static bool test_section_gates_and_capacity(void) {
  Mech *const mech = (Mech *)1;
  char small[1] = {'x'};
  reset_state();
  sections[CTORSO].destroyed = true;
  sections[LTORSO].armor = 0;
  sections[LTORSO].original_armor = 4;
  if (mech_repair_job_count(mech) != 1) {
    fprintf(stderr, "gate count=%zu\n", mech_repair_job_count(mech));
    return false;
  }

  reset_state();
  unit_class = CLASS_VEH_GROUND;
  for (int section = 0; section < NUM_SECTIONS; section++) {
    sections[section].armor = 0;
    sections[section].original_armor = 1;
    sections[section].rear_armor = 0;
    sections[section].original_rear_armor = 1;
    for (int critical = 0; critical < NUM_CRITICALS; critical++) {
      criticals[section][critical] =
          (CriticalState){.part_type = ammunition_equipment_index(1),
                          .data = 0,
                          .full_ammunition = 1,
                          .temporary_failure = true};
    }
  }
  size_t count = mech_repair_job_count(mech);
  mech_repair_jobs_format(mech, small, sizeof(small));
  if (count != (size_t)(NUM_SECTIONS * (2 + 2 * NUM_CRITICALS)) ||
      small[0] != '\0') {
    fprintf(stderr, "capacity count=%zu small=%d\n", count, small[0]);
    return false;
  }
  return true;
}

static bool test_fix_dispatch_and_errors(void) {
  Mech *const mech = (Mech *)1;
  reset_state();
  unit_class = CLASS_VEH_GROUND;
  sections[0].armor = 0;
  sections[0].original_armor = 2;
  sections[0].rear_armor = 0;
  sections[0].original_rear_armor = 2;
  char range[] = "1-2";
  tech_fix(42, mech, range);
  if (dispatch_count != 2 || strcmp(dispatched[0], "tech_fixarmor:S0") != 0 ||
      strcmp(dispatched[1], "tech_fixarmor:S0 r") != 0) {
    fprintf(stderr, "dispatch %zu %s %s\n", dispatch_count, dispatched[0],
            dispatched[1]);
    return false;
  }

  char invalid[] = "3";
  tech_fix(42, mech, invalid);
  if (dispatch_count != 2 || strcmp(notification, "Invalid #!") != 0) {
    fprintf(stderr, "invalid %zu %s\n", dispatch_count, notification);
    return false;
  }

  reset_state();
  unit_class = CLASS_VEH_GROUND;
  fixable = false;
  scrapping_part = true;
  criticals[0][0].part_type = 600;
  char scrap_range[] = "1-2";
  tech_fix(42, mech, scrap_range);
  if (dispatch_count != 2 ||
      strcmp(dispatched[0], "tech_removepart:S0 1") != 0 ||
      strcmp(dispatched[1], "tech_removesection:S0") != 0) {
    fprintf(stderr, "scrap dispatch %zu %s %s\n", dispatch_count, dispatched[0],
            dispatched[1]);
    return false;
  }
  return true;
}

int main(void) {
  if (!test_damage_ordering_and_details() ||
      !test_section_gates_and_capacity() || !test_fix_dispatch_and_errors()) {
    fprintf(stderr, "repair damage planner test failed\n");
    return 1;
  }
  return 0;
}
