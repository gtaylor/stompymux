
#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"

typedef struct Autopilot Autopilot;
typedef struct Mech Mech;

/* ai.c */
void ai_set_speed(Mech *mech, Autopilot *a, float s);
void ai_set_heading(Mech *mech, Autopilot *a, int dir);
int ai_check_path(Mech *m, Autopilot *a, float dx, float dy, float delx,
                  float dely);
void ai_init(Autopilot *a, Mech *m);
void mech_snipe(DbRef player, Mech *mech, char *buffer);
