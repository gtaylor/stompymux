/* btech_context.h - Runtime dependencies for legacy BTech callbacks. */

#pragma once

#include <time.h>

typedef struct AccessControlStore AccessControlStore;
typedef struct CommandContext CommandContext;
typedef struct GameDatabase GameDatabase;
typedef struct MuxEventScheduler MuxEventScheduler;
typedef struct PersistenceContext PersistenceContext;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct ServerLog ServerLog;
typedef struct WorldIndexes WorldIndexes;

typedef struct BtechContext BtechContext;
struct BtechContext {
  ServerConfiguration *configuration;
  RuntimeClock *clock;
  CommandContext *command_context;
  GameDatabase *database;
  MuxEventScheduler *events;
  ServerLifecycle *lifecycle;
  ServerLog *log;
  PersistenceContext *persistence;
  WorldIndexes *world_indexes;
  AccessControlStore *access_control;
  time_t process_start_time;
};

void btech_context_initialize(
    BtechContext *context, ServerConfiguration *configuration,
    RuntimeClock *clock, CommandContext *command_context, GameDatabase *database,
    MuxEventScheduler *events, ServerLifecycle *lifecycle, ServerLog *log,
    PersistenceContext *persistence,
    WorldIndexes *world_indexes, AccessControlStore *access_control,
    time_t process_start_time);
void btech_context_activate(BtechContext *context);
BtechContext *btech_context_active(void);
CommandContext *btech_context_set_command(BtechContext *context,
                                          CommandContext *command_context);
