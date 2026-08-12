#pragma once

#include "mech_api_types.h"

#include <stdbool.h>
#include <stddef.h>

typedef struct EvaluationContext EvaluationContext;

void append_status(char *buffer, size_t size, const char *fmt, ...)
    __attribute__((format(printf, 3, 4)));
void print_weapon_status(EvaluationContext *evaluation, Mech *mech,
                         DbRef player, bool compact, char *compact_buffer,
                         size_t compact_buffer_size);
