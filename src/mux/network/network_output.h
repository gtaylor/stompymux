/* Player notification and descriptor output queues. */

#pragma once

#include "mux/network/descriptor.h"
#include "mux/objects/db.h"

typedef struct EvaluationContext EvaluationContext;

void raw_notify_raw(EvaluationContext *evaluation, DbRef player,
                    const char *message, const char *append);
void raw_notify(EvaluationContext *evaluation, DbRef player,
                const char *message);
void notify_printf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...)
    __attribute__((format(printf, 3, 4)));
void raw_notify_newline(EvaluationContext *evaluation, DbRef player);
void raw_broadcast(DescriptorRegistry *descriptors, int flags,
                   const char *format, ...)
    __attribute__((format(printf, 3, 4)));
void descriptor_queue_write(Descriptor *descriptor, const char *buffer,
                            int size);
void descriptor_queue_string(Descriptor *descriptor, const char *string);
