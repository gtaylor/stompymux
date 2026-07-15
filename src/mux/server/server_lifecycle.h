/* Server lifecycle entry points for startup, event-loop control, and shutdown.
 */

#pragma once

struct event_base;

int server_lifecycle_initialize(void);
struct event_base *server_lifecycle_event_base(void);
void server_lifecycle_prepare(void);
int server_lifecycle_boot(int mindb);
void server_lifecycle_run(int port);
void server_lifecycle_stop(void);
void server_lifecycle_shutdown(void);
