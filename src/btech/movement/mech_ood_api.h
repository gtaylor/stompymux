#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct MuxEvent MuxEvent;

void mech_ood_damage(Mech *wounded, Mech *attacker, int damage);
void mech_ood_event(MuxEvent *e);
void mech_ood_initiate(DbRef player, Mech *mech, char *buffer);
