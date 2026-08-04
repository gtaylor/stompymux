
/*
   p.mech.tag.h
*/

/* static void tag_recycle_event(MuxEvent * e); */
#pragma once

#include "mux/server/platform.h"

void mech_tag(DbRef player, void *data, char *buffer);
int isTAGDestroyed(Mech *mech);
void stopTAG(Mech *mech);
void checkTAG(Mech *mech);
