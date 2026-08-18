/** @file
 * Borrowed services for accepting client sockets.
 */
#pragma once

typedef struct AccessControlStore AccessControlStore;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct FileCache FileCache;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLog ServerLog;

typedef struct ConnectionRuntime ConnectionRuntime;
struct ConnectionRuntime {
  /* Every member is borrowed from MuxServer. */
  const ServerConfiguration *configuration;
  RuntimeClock *clock;
  DescriptorRegistry *descriptors;
  ServerLog *log;
  AccessControlStore *access_control;
  /* Stable owner slot: content loading replaces the cache stored here. */
  FileCache **files_owner;
};

/** Initializes connection runtime. @param[out] runtime Runtime services.
 * @param[in] configuration Server configuration. @param[in] clock Clock.
 * @param[in] descriptors Descriptors. @param[in] log Server log. @param[in]
 * access_control Access control. @param[in,out] files_owner Files owner. */

static inline void connection_runtime_initialize(
    ConnectionRuntime *runtime, const ServerConfiguration *configuration,
    RuntimeClock *clock, DescriptorRegistry *descriptors, ServerLog *log,
    AccessControlStore *access_control, FileCache **files_owner) {
  *runtime = (ConnectionRuntime){
      .configuration = configuration,
      .clock = clock,
      .descriptors = descriptors,
      .log = log,
      .access_control = access_control,
      .files_owner = files_owner,
  };
}
