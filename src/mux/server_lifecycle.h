/* Server lifecycle entry points for startup, event-loop control, and shutdown.
 */

#pragma once

void server_lifecycle_prepare(void);
int server_lifecycle_boot(int restarting, int mindb);
void server_lifecycle_run(int port);
void server_lifecycle_stop(void);
void server_lifecycle_shutdown(void);
