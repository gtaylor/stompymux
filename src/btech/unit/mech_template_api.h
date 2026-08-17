#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

typedef struct BtechContext BtechContext;

void mech_template_clear(Mech *mech, bool clear_communications);
bool mech_template_load(DbRef player, Mech *mech, const char *id);
char *mech_template_resolve_path(BtechContext *context, const char *mech_path,
                                 const char *id);
void mech_template_registry_clear(BtechContext *context);
void mech_template_registry_destroy(BtechContext *context);
