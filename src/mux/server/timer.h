/** @file
 * Timed server-maintenance lifecycle and idle-check interface.
 */
#pragma once

#include "mux/network/mux_event.h"
#include "mux/server/maintenance.h"

typedef struct uv_loop_s UvLoopT;
typedef struct MaintenanceContext MaintenanceContext;
typedef struct ServerTimer ServerTimer;

/** Creates server timer. @param[in] loop Event loop. @param[in] maintenance
 * Maintenance. */

ServerTimer *server_timer_create(UvLoopT *loop,
                                 MaintenanceContext *maintenance);
/** Destroys server timer. @param[in,out] timer Timer instance. */

void server_timer_destroy(ServerTimer *timer);
