/* timer.h - Timed server-maintenance lifecycle and idle-check interface. */

#pragma once

void init_timer(void);
void timer_shutdown(void);
void check_idle(void);
