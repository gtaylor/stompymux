
/*
   p.mech.c3i.h
*/

#pragma once

void mech_c3i_join_leave(DbRef player, void *data, char *buffer);
void mech_c3i_message(DbRef player, MECH *mech, char *buffer);
void mech_c3i_targets(DbRef player, MECH *mech, char *buffer);
void mech_c3i_network(DbRef player, MECH *mech, char *buffer);
int getFreeC3iNetworkPos(MECH *mech, MECH *mechToAdd);
void replicateC3iNetwork(MECH *mechSrc, MECH *mechDest);
void validateC3iNetwork(MECH *mech);
MECH *getOtherC3iMech(MECH *mech, int wIdx, int tCheckECM, int tCheckStarted,
                      int tCheckUncon);
void clearC3iNetwork(MECH *mech, int tClearFromOthers);
void clearMechFromC3iNetwork(DbRef refToClear, MECH *mech);
void addMechToC3iNetwork(MECH *mech, MECH *mechToAdd);
