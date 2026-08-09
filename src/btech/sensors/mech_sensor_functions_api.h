/* Declares the BattleTech unit sensor functions API. */

#pragma once

#include "mux/server/platform.h"

/* mech.sensor.functions.c */
int vislight_see(Mech *t, int num, float r, int c, int l);
int liteamp_see(Mech *t, int num, float r, int c, int l);
int infrared_see(Mech *t, int num, float r, int c, int l);
int electrom_see(Mech *t, int num, float r, int c, int l);
int seismic_see(Mech *t, int num, float r, int c, int l);
int radar_see(Mech *t, int num, float r, int c, int l);
int bap_see(Mech *t, int num, float r, int c, int l);
int blood_see(Mech *t, int num, float r, int c, int l);
int vislight_csee(Mech *m, Mech *t, float r, int f);
int liteamp_csee(Mech *m, Mech *t, float r, int f);
int infrared_csee(Mech *m, Mech *t, float r, int f);
int electrom_csee(Mech *m, Mech *t, float r, int f);
int seismic_csee(Mech *m, Mech *t, float r, int f);
int radar_csee(Mech *m, Mech *t, float r, int f);
int bap_csee(Mech *m, Mech *t, float r, int f);
int blood_csee(Mech *m, Mech *t, float r, int f);
int vislight_tohit(Mech *m, Mech *t, int f, int l);
int liteamp_tohit(Mech *m, Mech *t, int f, int l);
int infrared_tohit(Mech *m, Mech *t, int f, int l);
int electrom_tohit(Mech *m, Mech *t, int f, int l);
int seismic_tohit(Mech *m, Mech *t, int f, int l);
int bap_tohit(Mech *m, Mech *t, int f, int l);
int blood_tohit(Mech *m, Mech *t, int f, int l);
int radar_tohit(Mech *m, Mech *t, int f, int l);
