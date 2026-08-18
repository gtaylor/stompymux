/** @file
 * Player notification and descriptor output queues.
 */
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

/** Sends raw notify raw. @param[in] notification Notification. */

void raw_notify_raw(const RawNotification *notification);
/** Sends raw notify. @param[in,out] evaluation Expression evaluation context.
 * @param[in] player Player object. @param[in] msg Msg. */

void raw_notify(EvaluationContext *evaluation, DbRef player, const char *msg);
/** Sends notify printf. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. @param[in] format Format. */

void notify_printf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...)
    __attribute__((format(printf, 3, 4)));
/** Sends raw notify newline. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. */

void raw_notify_newline(EvaluationContext *evaluation, DbRef player);
/** Executes raw broadcast. @param[in,out] descriptors Descriptors. @param[in]
 * flags Flags. @param[in] template Template. */

void raw_broadcast(DescriptorRegistry *descriptors, int flags,
                   const char *template, ...)
    __attribute__((format(printf, 3, 4)));
/** Executes descriptor queue write. @param[in,out] descriptor Network
 * descriptor. @param[in] buffer Caller-owned output storage. @param[in] n N. */

void descriptor_queue_write(Descriptor *descriptor, const char *buffer, int n);
/** Executes descriptor queue string. @param[in,out] descriptor Network
 * descriptor. @param[in] string String to process. */

void descriptor_queue_string(Descriptor *descriptor, const char *string);
