
/* Defines unit repair job types and operations. */

#include <stdbool.h>
#include <stdint.h>

#include "btech/context.h"
#include "mech_api_types.h"
#include "mux/network/mux_event.h"

#pragma once

typedef enum RepairStallPolicy {
  REPAIR_STALL_REQUIRED,
  REPAIR_STALL_CONFIGURED,
} RepairStallPolicy;

typedef enum RepairCommandStatus {
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

typedef enum RepairParseStatus {
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

typedef enum RepairJobResult {
  REPAIR_JOB_REJECTED,
  REPAIR_JOB_CALLBACK_ABORTED,
  REPAIR_JOB_SCHEDULED_SUCCESS,
  REPAIR_JOB_SCHEDULED_FAILURE,
} RepairJobResult;

typedef int (*RepairPartOperation)(DbRef, Mech *, int, int);
typedef int (*RepairPartAmountOperation)(DbRef, Mech *, int, int, int *);
typedef int (*RepairSectionOperation)(DbRef, Mech *, int);
typedef int (*RepairSectionAmountOperation)(DbRef, Mech *, int, int *);

typedef struct RepairPartJob {
  int difficulty;
  int time;
  intptr_t event_data;
  int event_type;
  MuxEventCallback event_callback;
  const char *message;
  bool weapon_roll;
  RepairPartOperation resource;
  RepairPartOperation failure;
  RepairPartOperation success;
} RepairPartJob;

typedef struct RepairPartAmountJob {
  int difficulty;
  int time;
  int event_type;
  MuxEventCallback event_callback;
  const char *message;
  RepairPartAmountOperation resource;
  RepairPartAmountOperation failure;
  RepairPartAmountOperation success;
} RepairPartAmountJob;

typedef struct RepairSectionJob {
  int difficulty;
  int time;
  intptr_t event_data;
  int event_type;
  MuxEventCallback event_callback;
  const char *message;
  RepairSectionOperation resource;
  RepairSectionOperation failure;
  RepairSectionOperation success;
} RepairSectionJob;

typedef struct RepairSectionAmountJob {
  int difficulty;
  int failure_time;
  int unit_time;
  int failure_event_type;
  int event_type;
  MuxEventCallback event_callback;
  const char *message;
  RepairSectionAmountOperation resource;
  RepairSectionAmountOperation failure;
  RepairSectionAmountOperation success;
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
void repair_event_schedule(Mech *mech, int delay, int event_type,
                           MuxEventCallback callback,
                           RepairEventPayload payload);
void repair_event_schedule_minutes(Mech *mech, int minutes, int event_type,
                                   MuxEventCallback callback,
                                   RepairEventPayload payload);
void repair_event_schedule_with_techtime(RepairCommandContext *command,
                                         int work_time, int multiplier,
                                         int event_type,
                                         MuxEventCallback callback,
                                         RepairEventPayload payload);
void repair_event_schedule_amount(RepairCommandContext *command, int work_time,
                                  int multiplier, int amount, int event_type,
                                  MuxEventCallback callback,
                                  RepairEventPayload payload);

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
#if 1
constexpr int TECH_TICK = 60;
constexpr char TECH_UNIT[] = "minute";
#else
constexpr int TECH_TICK = 1;
constexpr char TECH_UNIT[] = "second";
#endif

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
