
/*
   p.mech.tag.h
*/

/* static void tag_recycle_event(MuxEvent * e); */
#pragma once
void mech_tag(DbRef player, void *data, char *buffer);
int isTAGDestroyed(MECH *mech);
void stopTAG(MECH *mech);
void checkTAG(MECH *mech);
