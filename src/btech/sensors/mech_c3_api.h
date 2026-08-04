
/* p.mech.c3.h */

#pragma once

#include "mux/server/platform.h"

int getC3MasterSize(Mech *mech);
int isPartOfWorkingC3Master(Mech *mech, int section, int slot);
int countWorkingC3MastersOnMech(Mech *mech);
int countTotalC3MastersOnMech(Mech *mech);
int countMaxC3Units(Mech *mech, DbRef *myTempNetwork, int tempNetworkSize,
                    Mech *targMech);
int trimC3Network(Mech *mech, DbRef *myTempNetwork, int tempNetworkSize);
int getFreeC3NetworkPos(Mech *mech, Mech *mechToAdd);
void replicateC3Network(Mech *mechSrc, Mech *mechDest);
void addMechToC3Network(Mech *mech, Mech *mechToAdd);
void clearMechFromC3Network(DbRef refToClear, Mech *mech);
void clearC3Network(Mech *mech, int tClearFromOthers);
void validateC3Network(Mech *mech);
void mech_c3_join_leave(DbRef player, void *data, char *buffer);
void mech_c3_message(DbRef player, Mech *mech, char *buffer);
void mech_c3_targets(DbRef player, Mech *mech, char *buffer);
void mech_c3_network(DbRef player, Mech *mech, char *buffer);
