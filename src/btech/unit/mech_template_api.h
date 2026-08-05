#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

typedef struct BtechContext BtechContext;

void mech_template_clear(Mech *mech, bool clear_communications);
int mech_template_load(DbRef player, Mech *mech, const char *reference);
char *mech_template_resolve_path(BtechContext *context,
                                 const char *template_path,
                                 const char *reference);
void mech_template_registry_clear(BtechContext *context);
void mech_template_registry_destroy(BtechContext *context);
