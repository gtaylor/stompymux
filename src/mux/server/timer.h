/* timer.h - Timed server-maintenance lifecycle and idle-check interface. */

#pragma once

typedef struct uv_loop_s uv_loop_t;
typedef struct MaintenanceContext MaintenanceContext;
typedef struct ServerTimer ServerTimer;

ServerTimer *server_timer_create(uv_loop_t *loop,
                                 MaintenanceContext *maintenance);
void server_timer_destroy(ServerTimer *timer);
