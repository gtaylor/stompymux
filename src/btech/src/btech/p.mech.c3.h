
/* p.mech.c3.h */

#pragma once

int getC3MasterSize(MECH *mech);
int isPartOfWorkingC3Master(MECH *mech, int section, int slot);
int countWorkingC3MastersOnMech(MECH *mech);
int countTotalC3MastersOnMech(MECH *mech);
int countMaxC3Units(MECH *mech, DbRef *myTempNetwork, int tempNetworkSize,
                    MECH *targMech);
int trimC3Network(MECH *mech, DbRef *myTempNetwork, int tempNetworkSize);
int getFreeC3NetworkPos(MECH *mech, MECH *mechToAdd);
void replicateC3Network(MECH *mechSrc, MECH *mechDest);
void addMechToC3Network(MECH *mech, MECH *mechToAdd);
void clearMechFromC3Network(DbRef refToClear, MECH *mech);
void clearC3Network(MECH *mech, int tClearFromOthers);
void validateC3Network(MECH *mech);
void mech_c3_join_leave(DbRef player, void *data, char *buffer);
void mech_c3_message(DbRef player, MECH *mech, char *buffer);
void mech_c3_targets(DbRef player, MECH *mech, char *buffer);
void mech_c3_network(DbRef player, MECH *mech, char *buffer);
