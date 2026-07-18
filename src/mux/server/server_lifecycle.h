/* Server lifecycle entry points for startup, event-loop control, and shutdown.
 */

#pragma once

#include <stdbool.h>

typedef struct uv_loop_s uv_loop_t;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct CommandQueue CommandQueue;
typedef struct MaintenanceContext MaintenanceContext;

ServerLifecycle *server_lifecycle_create(MaintenanceContext *maintenance);
void server_lifecycle_destroy(ServerLifecycle *lifecycle);
uv_loop_t *server_lifecycle_loop(ServerLifecycle *lifecycle);
void server_lifecycle_prepare(ServerLifecycle *lifecycle);
void server_lifecycle_unbind_signals(ServerLifecycle *lifecycle);
int server_lifecycle_boot(ServerLifecycle *lifecycle, int mindb);
void server_lifecycle_run(ServerLifecycle *lifecycle, int port);
void server_lifecycle_stop(ServerLifecycle *lifecycle);
void server_lifecycle_release_sockets(ServerLifecycle *lifecycle);
void server_lifecycle_close_connections(ServerLifecycle *lifecycle,
                                        bool emergency, const char *message);
int server_lifecycle_eradicate_fd(ServerLifecycle *lifecycle, int fd);
void server_lifecycle_shutdown(ServerLifecycle *lifecycle);
