/* context.h - Public ownership and command-scope interface for BTech. */

#pragma once

#include <stdbool.h>
#include <time.h>

#include "btech/ids.h"

typedef struct AccessControlStore AccessControlStore;
typedef struct BtechContext BtechContext;
typedef struct BtechCommandScope BtechCommandScope;
typedef struct CommandContext CommandContext;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct MuxEventScheduler MuxEventScheduler;
typedef struct PersistenceContext PersistenceContext;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct ServerLog ServerLog;
typedef struct WorldIndexes WorldIndexes;

typedef struct BtechDependencies {
  /* Every dependency is borrowed and must outlive the BTech context. */
  ServerConfiguration *configuration;
  RuntimeClock *clock;
  CommandContext *background_command;
  GameDatabase *database;
  MuxEventScheduler *events;
  ServerLifecycle *lifecycle;
  ServerLog *log;
  PersistenceContext *persistence;
  WorldIndexes *world_indexes;
  AccessControlStore *access_control;
  time_t process_start_time;
} BtechDependencies;

struct BtechCommandScope {
  /* Scope links are borrowed and valid only for the nested invocation. */
  BtechContext *context;
  CommandContext *command;
  BtechCommandScope *previous;
  bool active;
};

BtechContext *btech_context_create(const BtechDependencies *dependencies);
void btech_context_destroy(BtechContext *context);
void btech_context_set_lifecycle(BtechContext *context,
                                 ServerLifecycle *lifecycle);
void btech_context_set_process_start_time(BtechContext *context,
                                          time_t process_start_time);
CommandContext *btech_context_command(BtechContext *context);
EvaluationContext *btech_context_evaluation(BtechContext *context);
GameDatabase *btech_context_database(BtechContext *context);
bool btech_context_combat_arcs_enabled(const BtechContext *context);
int btech_context_movement_slowdown_mode(const BtechContext *context);
int btech_context_event_tick(const BtechContext *context);
time_t btech_context_now(const BtechContext *context);
void btech_command_scope_enter(BtechCommandScope *scope, BtechContext *context,
                               CommandContext *command);
void btech_command_scope_leave(BtechCommandScope *scope);

#ifdef BTECH_INTERNAL
#include "context_internal.h"
#endif
