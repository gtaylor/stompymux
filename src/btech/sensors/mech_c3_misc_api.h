
/*
   p.mech.c3.misc.h
*/

#pragma once

typedef struct BtechContext BtechContext;

#include "mux/server/platform.h"

Mech *getMechInTempNetwork(BtechContext *context, int wIdx, DbRef *myNetwork,
                           int networkSize);
Mech *getOtherMechInNetwork(Mech *mech, int wIdx, int tCheckECM,
                            int tCheckStarted, int tCheckUncon, int tIsC3);
void buildTempNetwork(Mech *mech, DbRef *myNetwork, int *networkSize,
                      int tCheckECM, int tCheckStarted, int tCheckUncon,
                      int tIsC3);
void sendNetworkMessage(DbRef player, Mech *mech, char *msg, int tIsC3);
void showNetworkTargets(DbRef player, Mech *mech, int tIsC3);
void showNetworkData(DbRef player, Mech *mech, int tIsC3);
int mechSeenByNetwork(Mech *mech, Mech *mechTarget, int isC3);
float findC3Range(Mech *mech, Mech *mechTarget, float realRange, DbRef *c3Ref,
                  int tIsC3);
float findC3RangeWithNetwork(Mech *mech, Mech *mechTarget, float realRange,
                             DbRef *myNetwork, int networkSize, DbRef *c3Ref);
void debugC3(BtechContext *context, char *msg);
