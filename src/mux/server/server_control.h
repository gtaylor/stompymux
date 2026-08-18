/** @file
 * Borrowed process-control and persistence services.
 */
#pragma once

typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct BtechContext BtechContext;
typedef struct CommandContext CommandContext;
typedef struct GameDatabase GameDatabase;
typedef struct PersistenceContext PersistenceContext;
typedef struct PlayerCache PlayerCache;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct ServerLog ServerLog;

typedef struct ServerControl ServerControl;
struct ServerControl {
  /* Every member is borrowed from MuxServer. */
  ServerConfiguration *configuration;
  GameDatabase *database;
  ServerLog *log;
  DescriptorRegistry *descriptors;
  PlayerCache *players;
  PersistenceContext *persistence;
  ServerLifecycle *lifecycle;
  CommandContext *command;
  BtechContext *btech;
};

/** Initializes server control. @param[out] control Control. @param[in]
 * configuration Server configuration. @param[in] database Game database.
 * @param[in] log Server log. @param[in] descriptors Descriptors. @param[in]
 * players Players. @param[in] persistence Persistence. @param[in] command
 * Command text or descriptor. @param[in] btech Btech. */

static inline void server_control_initialize(
    ServerControl *control, ServerConfiguration *configuration,
    GameDatabase *database, ServerLog *log, DescriptorRegistry *descriptors,
    PlayerCache *players, PersistenceContext *persistence,
    CommandContext *command, BtechContext *btech) {
  *control = (ServerControl){
      .configuration = configuration,
      .database = database,
      .log = log,
      .descriptors = descriptors,
      .players = players,
      .persistence = persistence,
      .command = command,
      .btech = btech,
  };
}
