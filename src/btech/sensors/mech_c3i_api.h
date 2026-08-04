
/*
   p.mech.c3i.h
*/

#pragma once

#include "mux/server/platform.h"

void mech_c3i_join_leave(DbRef player, void *data, char *buffer);
void mech_c3i_message(DbRef player, Mech *mech, char *buffer);
void mech_c3i_targets(DbRef player, Mech *mech, char *buffer);
void mech_c3i_network(DbRef player, Mech *mech, char *buffer);
int getFreeC3iNetworkPos(Mech *mech, Mech *mechToAdd);
void replicateC3iNetwork(Mech *mechSrc, Mech *mechDest);
void validateC3iNetwork(Mech *mech);
Mech *getOtherC3iMech(Mech *mech, int wIdx, int tCheckECM, int tCheckStarted,
                      int tCheckUncon);
void clearC3iNetwork(Mech *mech, int tClearFromOthers);
void clearMechFromC3iNetwork(DbRef refToClear, Mech *mech);
void addMechToC3iNetwork(Mech *mech, Mech *mechToAdd);
