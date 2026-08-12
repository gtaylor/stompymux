/* Player notification and descriptor output queues. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

typedef struct RawNotification {
  EvaluationContext *evaluation;
  DbRef player;
  const char *message;
  const char *suffix;
} RawNotification;

void raw_notify_raw(const RawNotification *notification);
void raw_notify(EvaluationContext *evaluation, DbRef player, const char *msg);
void notify_printf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...)
    __attribute__((format(printf, 3, 4)));
void raw_notify_newline(EvaluationContext *evaluation, DbRef player);
void raw_broadcast(DescriptorRegistry *descriptors, int flags,
                   const char *template, ...)
    __attribute__((format(printf, 3, 4)));
void descriptor_queue_write(Descriptor *descriptor, const char *buffer, int n);
void descriptor_queue_string(Descriptor *descriptor, const char *string);
