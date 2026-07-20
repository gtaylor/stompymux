/* command_context.h - State scoped to one command execution. */

#pragma once

#include <stdbool.h>

#include "mux/server/platform.h"
#include "mux/support/alloc.h"

typedef struct Descriptor Descriptor;
typedef struct BtechContext BtechContext;
typedef struct CommandRuntime CommandRuntime;
typedef struct ServerLog ServerLog;
typedef struct WorldContext WorldContext;
typedef struct CommandContext CommandContext;
typedef struct EvaluationContext EvaluationContext;

typedef struct MatchContext MatchContext;
struct MatchContext {
  /* The evaluation and input string are borrowed for one match operation. */
  EvaluationContext *evaluation;
  int confidence;
  int count;
  int pref_type;
  bool check_keys;
  DbRef absolute_form;
  DbRef match;
  DbRef player;
  char *string;
  char normalized[LBUF_SIZE];
};

struct EvaluationContext {
  /* Borrowed runtime services and world view. */
  CommandRuntime *runtime;
  BtechContext *btech;
  ServerLog *log;
  WorldContext *world;
  CommandContext *command;
  int function_nesting;
  int function_invocations;
  int notification_nesting;
  void *trace_head;
  int trace_count;
  bool trace_top;
  char *registers[MAX_GLOBAL_REGS];
  bool is_piping;
  char *pipe_output;
  char *pipe_next;
  char *pipe_cursor;
  DbRef pipe_object;
};

struct CommandContext {
  /* Borrowed runtime services and world view. */
  CommandRuntime *runtime;
  BtechContext *btech;
  ServerLog *log;
  WorldContext *world;
  DbRef player;
  DbRef enactor;
  Descriptor *descriptor;
  bool interactive;
  const char *debug_command;
  MatchContext match;
  EvaluationContext evaluation;
};

bool command_context_initialize(CommandContext *context,
                                CommandRuntime *runtime, BtechContext *btech,
                                ServerLog *log, DbRef player, DbRef enactor,
                                Descriptor *descriptor, bool interactive);
void command_context_destroy(CommandContext *context);
void command_context_reset_limits(CommandContext *context);
