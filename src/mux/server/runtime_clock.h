/* Shared wall-clock and resource-sampling state. */

#pragma once

#include <time.h>

typedef struct RuntimeClock RuntimeClock;
struct RuntimeClock {
  time_t now;
  time_t dump_deadline;
  time_t check_deadline;
  time_t idle_deadline;
  time_t metrics_deadline;
  bool tick_pending;
  int shared_memory[2];
  int private_memory[2];
  int stack_memory[2];
  int sample_time[2];
  int current_sample;
};
