/* timer.h - Timed server-maintenance lifecycle and idle-check interface. */

#pragma once

#include "mux/network/mux_event.h"
#include "mux/server/maintenance.h"

typedef struct uv_loop_s UvLoopT;
typedef struct MaintenanceContext MaintenanceContext;
typedef struct ServerTimer ServerTimer;

ServerTimer *server_timer_create(UvLoopT *loop,
                                 MaintenanceContext *maintenance);
void server_timer_destroy(ServerTimer *timer);
