
#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"

typedef struct Autopilot Autopilot;
typedef struct Mech Mech;

typedef struct AiInfo {
  char text[MBUF_SIZE];
} AiInfo;

/* ai.c */
void sendAIM(Autopilot *a, Mech *m, char *msg);
AiInfo ai_info(Mech *m, Autopilot *a);
int ai_max_speed(Mech *m, Autopilot *a);
int ai_opponents(Autopilot *a, Mech *m);
void ai_run_speed(Mech *mech, Autopilot *a);
void ai_stop(Mech *mech, Autopilot *a);
void ai_set_speed(Mech *mech, Autopilot *a, float s);
void ai_set_heading(Mech *mech, Autopilot *a, int dir);
void ai_adjust_move(Autopilot *a, Mech *m, char *text, int hmod, int smod,
                    int b_score);
int ai_check_path(Mech *m, Autopilot *a, float dx, float dy, float delx,
                  float dely);
void ai_init(Autopilot *a, Mech *m);
void mech_snipe(DbRef player, Mech *mech, char *buffer);
