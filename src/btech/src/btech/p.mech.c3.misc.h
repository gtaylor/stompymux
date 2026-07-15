
/*
   p.mech.c3.misc.h
*/

#pragma once

MECH *getMechInTempNetwork(int wIdx, DbRef *myNetwork, int networkSize);
MECH *getOtherMechInNetwork(MECH *mech, int wIdx, int tCheckECM,
                            int tCheckStarted, int tCheckUncon, int tIsC3);
void buildTempNetwork(MECH *mech, DbRef *myNetwork, int *networkSize,
                      int tCheckECM, int tCheckStarted, int tCheckUncon,
                      int tIsC3);
void sendNetworkMessage(DbRef player, MECH *mech, char *msg, int tIsC3);
void showNetworkTargets(DbRef player, MECH *mech, int tIsC3);
void showNetworkData(DbRef player, MECH *mech, int tIsC3);
int mechSeenByNetwork(MECH *mech, MECH *mechTarget, int isC3);
float findC3Range(MECH *mech, MECH *mechTarget, float realRange, DbRef *c3Ref,
                  int tIsC3);
float findC3RangeWithNetwork(MECH *mech, MECH *mechTarget, float realRange,
                             DbRef *myNetwork, int networkSize, DbRef *c3Ref);
void debugC3(char *msg);
