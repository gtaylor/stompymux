/* runtime_clock.h - Process time, maintenance deadlines, and usage samples. */

#pragma once

#include <stdbool.h>
#include <time.h>

typedef struct RuntimeClock RuntimeClock;
struct RuntimeClock {
  time_t now;
  time_t dump_deadline;
  time_t check_deadline;
  time_t idle_deadline;
  time_t metrics_deadline;
  time_t events_deadline;
  int events_run;
  int events_last_hour;
  bool tick_pending;
  int shared_memory[2];
  int private_memory[2];
  int stack_memory[2];
  int sample_time[2];
  int current_sample;
};

void runtime_clock_initialize(RuntimeClock *clock);

