
/*
   p.mech.sensor.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 10:40:21 CET 1999 from mech.sensor.c */

#pragma once

#include "mux/server/platform.h"
#include "mux/support/alloc.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

typedef struct SensorFlagText {
  char text[MBUF_SIZE];
} SensorFlagText;

/* mech.sensor.c */
int mech_sensor_to_hit_bonus(Mech *mech, Mech *target, int flag, int maplight,
                             float range, int ammunition_mode);
int mech_sensor_can_see(Mech *mech, Mech *target, int *flag, int arc,
                        float range, int mapvis, int maplight, int cloudbase);
int mech_sensor_arc_base_chance(int type, int arc);
int mech_sensor_driver_base_chance(Mech *mech);
int mech_sensor_detects(Mech *mech, Mech *target, int f, int arc, float range,
                        int snum, int chance_divisor, int mapvis, int maplight);
int mech_sensor_detects_now(Mech *mech, Mech *target, int f, int arc,
                            float range, int mapvis, int maplight);
SensorFlagText sensor_flag_text(int flags);
void mech_sensor_visibility_update(Mech *mech, unsigned short *fl, float range,
                                   int x, int y, Mech *target, int mapvis,
                                   int maplight, int cloudbase, int seeanew,
                                   int wlf);
void mech_sensor_map_los_update(DbRef obj, BattleMap *map);
void mech_sensor_description_append(char *buf, int size, Mech *mech, int sn,
                                    int verbose);
char *mech_sensor_info(Mech *mech, char buffer[static LBUF_SIZE]);
int mech_sensor_can_change_to(Mech *mech, int sensor);
void mech_sensors_disable_requiring(Mech *mech, int technology);
void sensor_light_availability_check(Mech *mech);
void mech_sensor(DbRef player, void *data, char *buffer);
void mech_sensor_visibility_refresh(Mech *mech);
void mech_sensors_scramble_infrared_and_liteamp(Mech *mech, int time,
                                                int chance,
                                                const char *inframsg,
                                                const char *liteampmsg);
