/** @file
 * Server lifecycle entry points for startup, event-loop control, and shutdown.
 */
#pragma once

// IWYU pragma: no_include "mux/commands/command_runtime.h"
// IWYU pragma: no_include "mux/server/maintenance.h"
// IWYU pragma: no_include "mux/server/server_config.h"
// IWYU pragma: no_include "mux/world/world_context.h"
// IWYU pragma: no_include "uv.h"

typedef struct uv_loop_s UvLoopT;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct CommandQueue CommandQueue;
typedef struct MaintenanceContext MaintenanceContext;

/** Creates server lifecycle. @param[in] maintenance Maintenance. */

ServerLifecycle *server_lifecycle_create(MaintenanceContext *maintenance);
/** Destroys server lifecycle. @param[in,out] lifecycle Lifecycle. */

void server_lifecycle_destroy(ServerLifecycle *lifecycle);
/** Executes server lifecycle loop. @param[in,out] lifecycle Lifecycle. */

UvLoopT *server_lifecycle_loop(ServerLifecycle *lifecycle);
/** Executes server lifecycle prepare. @param[in,out] lifecycle Lifecycle. */

void server_lifecycle_prepare(ServerLifecycle *lifecycle);
/** Executes server lifecycle unbind signals. @param[in,out] lifecycle
 * Lifecycle. */

void server_lifecycle_unbind_signals(ServerLifecycle *lifecycle);
/** Executes server lifecycle boot. @param[in,out] lifecycle Lifecycle. */

bool server_lifecycle_boot(ServerLifecycle *lifecycle);
/** Executes server lifecycle run. @param[in,out] lifecycle Lifecycle.
 * @param[in] port Port. */

void server_lifecycle_run(ServerLifecycle *lifecycle, int port);
/** Stops server lifecycle. @param[in,out] lifecycle Lifecycle. */

void server_lifecycle_stop(ServerLifecycle *lifecycle);
/** Executes server lifecycle release sockets. @param[in,out] lifecycle
 * Lifecycle. */

void server_lifecycle_release_sockets(ServerLifecycle *lifecycle);
/** Executes server lifecycle close connections. @param[in,out] lifecycle
 * Lifecycle. @param[in] emergency Emergency. @param[in] message Message. */

void server_lifecycle_close_connections(ServerLifecycle *lifecycle,
                                        bool emergency, const char *message);
/** Executes server lifecycle eradicate fd. @param[in,out] lifecycle Lifecycle.
 * @param[in] fd Fd. */

bool server_lifecycle_eradicate_fd(ServerLifecycle *lifecycle, int fd);
/** Executes server lifecycle shutdown. @param[in,out] lifecycle Lifecycle. */

void server_lifecycle_shutdown(ServerLifecycle *lifecycle);
