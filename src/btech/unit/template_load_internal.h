/* Private helpers for loading unit templates. */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"
#include <stdio.h>

bool template_load_error(FILE *file, Mech *mech, DbRef player, bool condition,
                         bool global, const char *format, ...)
    __attribute__((format(printf, 6, 7)));
bool template_read_int(FILE *file, Mech *mech, DbRef player, char *text,
                       int *value);
bool template_read_float(FILE *file, Mech *mech, DbRef player, char *text,
                         float *value);
bool template_parse_critical_range(char *command, int *first, int *last);
void template_load_finalize(Mech *mech, bool clan_equipment);
