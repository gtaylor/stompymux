
/*
   p.mech.tag.h
*/

/* static void tag_recycle_event(MuxEvent * e); */
#pragma once

#include <stdbool.h>

#include "mech_api_types.h"
#include "mux/server/platform.h"

void mech_tag(DbRef player, void *data, char *buffer);
bool mech_tag_is_destroyed(const Mech *mech);
void mech_tag_stop(Mech *mech);
void mech_tag_check(Mech *mech);
