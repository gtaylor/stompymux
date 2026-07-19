
/*
   p.ai.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:33 CET 1999 from ai.c */

#pragma once

#include "mux/server/platform.h"

typedef struct AiInfo {
  char text[MBUF_SIZE];
} AiInfo;

/* ai.c */
void sendAIM(AUTO *a, MECH *m, char *msg);
AiInfo ai_info(MECH *m, AUTO *a);
int ai_max_speed(MECH *m, AUTO *a);
int ai_opponents(AUTO *a, MECH *m);
void ai_run_speed(MECH *mech, AUTO *a);
void ai_stop(MECH *mech, AUTO *a);
#if 0
void ai_set_speed(MECH * mech, AUTO * a, int s);
#endif
void ai_set_speed(MECH *mech, AUTO *a, float s);
void ai_set_heading(MECH *mech, AUTO *a, int dir);
void ai_adjust_move(AUTO *a, MECH *m, char *text, int hmod, int smod,
                    int b_score);
int ai_check_path(MECH *m, AUTO *a, float dx, float dy, float delx, float dely);
void ai_init(AUTO *a, MECH *m);
void mech_snipe(DbRef player, MECH *mech, char *buffer);
