
/* Defines unit repair job types and operations. */

#include <stdbool.h>
#include <stdint.h>

#include "btech/context.h"
#include "mech_api_types.h"
#include "mux/network/mux_event.h"

#pragma once

typedef enum RepairStallPolicy : int {
  REPAIR_STALL_REQUIRED,
  REPAIR_STALL_CONFIGURED,
} RepairStallPolicy;

typedef enum RepairCommandStatus : int {
  REPAIR_COMMAND_READY,
  REPAIR_COMMAND_MISSING_MECH,
  REPAIR_COMMAND_STARTING,
  REPAIR_COMMAND_STARTED,
  REPAIR_COMMAND_NO_STALL,
  REPAIR_COMMAND_UNAUTHORIZED,
  REPAIR_COMMAND_NO_TARGET,
  REPAIR_COMMAND_TARGET_UNALLOCATED,
} RepairCommandStatus;

typedef struct RepairCommandContext {
  DbRef player;
  BtechContext *context;
  EvaluationContext *evaluation;
  Mech *mech;
  bool is_dropship;
} RepairCommandContext;

typedef struct RepairFacility RepairFacility;
typedef struct RepairFacilityCommandContext {
  DbRef player;
  RepairFacility *facility;
  BtechContext *context;
  EvaluationContext *evaluation;
  Mech *mech;
} RepairFacilityCommandContext;

typedef enum RepairParseStatus : int {
  REPAIR_PARSE_OK,
  REPAIR_PARSE_INVALID_SECTION,
  REPAIR_PARSE_INVALID_PART,
  REPAIR_PARSE_INVALID_GUN,
  REPAIR_PARSE_INVALID_REPLACEMENT,
  REPAIR_PARSE_MISMATCHED_REPLACEMENT,
  REPAIR_PARSE_INVALID_GUN_LOCATION,
} RepairParseStatus;

typedef struct RepairSelection {
  int location;
  int part;
  int brand;
} RepairSelection;

typedef struct RepairEventPayload {
  int location;
  int position;
  int extra;
  DbRef player;
} RepairEventPayload;

typedef enum RepairJobResult : int {
  REPAIR_JOB_REJECTED,
  REPAIR_JOB_CALLBACK_ABORTED,
  REPAIR_JOB_SCHEDULED_SUCCESS,
  REPAIR_JOB_SCHEDULED_FAILURE,
} RepairJobResult;

typedef struct RepairOperationCall {
  DbRef player;
  Mech *mech;
  RepairSelection selection;
  int *amount;
} RepairOperationCall;

typedef int (*RepairOperation)(const RepairOperationCall *call);

typedef struct RepairPartJob {
  int difficulty;
  int time;
  intptr_t event_data;
  int event_type;
  MuxEventCallback event_callback;
  const char *message;
  bool weapon_roll;
  RepairOperation resource;
  RepairOperation failure;
  RepairOperation success;
} RepairPartJob;

typedef struct RepairPartAmountJob {
  int difficulty;
  int time;
  int event_type;
  MuxEventCallback event_callback;
  const char *message;
  RepairOperation resource;
  RepairOperation failure;
  RepairOperation success;
} RepairPartAmountJob;

typedef struct RepairSectionJob {
  int difficulty;
  int time;
  intptr_t event_data;
  int event_type;
  MuxEventCallback event_callback;
  const char *message;
  RepairOperation resource;
  RepairOperation failure;
  RepairOperation success;
} RepairSectionJob;

typedef struct RepairSectionAmountJob {
  int difficulty;
  int failure_time;
  int unit_time;
  int failure_event_type;
  int event_type;
  MuxEventCallback event_callback;
  const char *message;
  RepairOperation resource;
  RepairOperation failure;
  RepairOperation success;
} RepairSectionAmountJob;

RepairCommandStatus
repair_command_context_initialize(DbRef player, void *data,
                                  RepairStallPolicy stall_policy,
                                  RepairCommandContext *command);
const char *repair_command_status_message(RepairCommandStatus status);
RepairCommandStatus repair_facility_command_context_initialize(
    DbRef player, void *data, bool target_required,
    RepairFacilityCommandContext *command);
RepairParseStatus repair_selection_parse_part(Mech *mech, char *buffer,
                                              bool parse_position,
                                              bool parse_brand,
                                              RepairSelection *selection);
RepairParseStatus repair_selection_parse_gun(Mech *mech, char *buffer,
                                             bool parse_brand,
                                             RepairSelection *selection);
const char *repair_parse_status_message(RepairParseStatus status);
bool repair_command_parse_part(RepairCommandContext *command, char *buffer,
                               bool parse_position, bool parse_brand,
                               RepairSelection *selection);
bool repair_command_parse_gun(RepairCommandContext *command, char *buffer,
                              bool parse_brand, RepairSelection *selection);

intptr_t repair_event_payload_pack(RepairEventPayload payload);
RepairEventPayload repair_event_payload_unpack(intptr_t encoded);
int repair_fix_event_amount(RepairEventPayload payload);
bool repair_fix_event_payload_with_amount(RepairEventPayload *payload,
                                          int amount);
typedef struct RepairEventSchedule {
  Mech *mech;
  int delay;
  int event_type;
  MuxEventCallback callback;
  RepairEventPayload payload;
} RepairEventSchedule;

typedef struct RepairWorkSchedule {
  RepairCommandContext *command;
  int work_time;
  int multiplier;
  int amount;
  int event_type;
  MuxEventCallback callback;
  RepairEventPayload payload;
} RepairWorkSchedule;

void repair_event_schedule(const RepairEventSchedule *schedule);
void repair_event_schedule_minutes(const RepairEventSchedule *schedule);
void repair_event_schedule_with_techtime(const RepairWorkSchedule *schedule);

RepairJobResult repair_part_job_execute(RepairCommandContext *command,
                                        int location, int part,
                                        const RepairPartJob *job);
RepairJobResult repair_part_amount_job_execute(RepairCommandContext *command,
                                               int location, int part,
                                               int *amount,
                                               const RepairPartAmountJob *job);
RepairJobResult repair_section_job_execute(RepairCommandContext *command,
                                           int location,
                                           const RepairSectionJob *job);
RepairJobResult
repair_section_amount_job_execute(RepairCommandContext *command, int location,
                                  int *amount,
                                  const RepairSectionAmountJob *job);

int repair_part_type_difficulty(int part_type);
int repair_weapon_type_difficulty(int part_type);

/* In minutes */
constexpr int TECH_TICK = 60;
constexpr char TECH_UNIT[] = "minute";

/* Tech skill modifiers ; + = bad, - = good */
constexpr int REPAIR_DIFFICULTY = 0;
constexpr int REPLACE_DIFFICULTY = 1;
constexpr int RELOAD_DIFFICULTY = 1;
constexpr int FIXARMOR_DIFFICULTY = 1;
constexpr int FIXINTERNAL_DIFFICULTY = 2;
constexpr int REATTACH_DIFFICULTY = 3;
constexpr int REMOVEG_DIFFICULTY = 1;
constexpr int REMOVEP_DIFFICULTY = 0;
constexpr int REMOVES_DIFFICULTY = 2;
constexpr int RESEAL_DIFFICULTY = 0; /* Added 8/4/99. Kipsta. */
constexpr int REPLACESUIT_DIFFICULTY = 3;
constexpr int ENHCRIT_DIFFICULTY = 0;

/* Times are in minutes */
constexpr int MOUNT_BOMB_TIME = 5;
constexpr int UMOUNT_BOMB_TIME = 5;
constexpr int REPLACEGUN_TIME = 60;
constexpr int REPLACEPART_TIME = 45;
constexpr int REPAIRGUN_TIME = 20;
constexpr int REPAIRENHCRIT_TIME = 15;
constexpr int REPAIRPART_TIME = 15;
constexpr int RELOAD_TIME = 10;
constexpr int FIXARMOR_TIME = 3;
constexpr int FIXINTERNAL_TIME = 9;
constexpr int REATTACH_TIME = 240;
constexpr int REMOVEP_TIME = 40;
constexpr int REMOVEG_TIME = 40;
constexpr int REMOVES_TIME = 120;
constexpr int RESEAL_TIME = 60; /* Added 8/4/99. Kipsta. */
constexpr int REPLACESUIT_TIME = 120;

constexpr int LOCMAX = 16;
constexpr int POSMAX = 16;
constexpr int EXTMAX = 256;
constexpr int PLAYERPOS = LOCMAX * POSMAX * EXTMAX;
constexpr int REPAIR_FIX_AMOUNT_MAX = EXTMAX - 1;
