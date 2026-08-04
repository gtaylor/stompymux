#pragma once

#include "mech_api_types.h"

typedef struct BtechContext BtechContext;

BtechContext *mech_context(const Mech *mech);
DbRef mech_dbref(const Mech *mech);
DbRef mech_map_dbref(const Mech *mech);
int mech_map_slot(const Mech *mech);
int mech_brief_mode(const Mech *mech);
void mech_map_dbref_set(Mech *mech, DbRef map_dbref);
void mech_brief_mode_set(Mech *mech, int mode);
void mech_unit_id_set(Mech *mech, char first, char second);
void mech_identity_initialize(Mech *mech, DbRef dbref);
