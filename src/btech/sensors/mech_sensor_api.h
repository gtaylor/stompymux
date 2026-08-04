
/*
   p.mech.sensor.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 10:40:21 CET 1999 from mech.sensor.c */

#pragma once

#include "mux/server/platform.h"

typedef struct SensorFlagText {
  char text[MBUF_SIZE];
} SensorFlagText;

/* mech.sensor.c */
int Sensor_ToHitBonus(Mech *mech, Mech *target, int flag, int maplight,
                      float range, int wAmmoMode);
int Sensor_CanSee(Mech *mech, Mech *target, int *flag, int arc, float range,
                  int mapvis, int maplight, int cloudbase);
int Sensor_ArcBaseChance(int type, int arc);
int Sensor_DriverBaseChance(Mech *mech);
int Sensor_Sees(Mech *mech, Mech *target, int f, int arc, float range, int snum,
                int chance_divisor, int mapvis, int maplight);
int Sensor_SeesNow(Mech *mech, Mech *target, int f, int arc, float range,
                   int mapvis, int maplight);
SensorFlagText sensor_flag_text(int flags);
void Sensor_DoWeSeeNow(Mech *mech, unsigned short *fl, float range, int x,
                       int y, Mech *target, int mapvis, int maplight,
                       int cloudbase, int seeanew, int wlf);
void update_LOSinfo(DbRef obj, BattleMap *map);
void add_sensor_info(char *buf, int size, Mech *mech, int sn, int verbose);
char *mechSensorInfo(Mech *mech, char buffer[static LBUF_SIZE]);
int CanChangeTo(Mech *mech, int s);
void sensor_light_availability_check(Mech *mech);
void mech_sensor(DbRef player, void *data, char *buffer);
void possibly_see_mech(Mech *mech);
void ScrambleInfraAndLiteAmp(Mech *mech, int time, int chance, char *inframsg,
                             char *liteampmsg);
