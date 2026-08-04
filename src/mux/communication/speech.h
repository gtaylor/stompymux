/* speech.h - Player speech and private-message command declarations. */

#pragma once

#include "mux/objects/db.h"

typedef struct EvaluationContext EvaluationContext;
typedef struct ServerConfiguration ServerConfiguration;

constexpr int PEMIT_PEMIT = 1; /* Emit to named player. */
constexpr int PEMIT_OEMIT = 2; /* Emit except to named player. */
constexpr int PEMIT_FSAY = 3;  /* Force controlled object to say. */
constexpr int PEMIT_FEMIT = 4; /* Force controlled object to emit. */
constexpr int PEMIT_FPOSE = 5; /* Force controlled object to pose. */
constexpr int PEMIT_FPOSE_NS =
    6; /* Force controlled object to pose sans space. */
constexpr int PEMIT_CONTENTS = 8; /* Send to contents. */
constexpr int PEMIT_HERE = 16;    /* Send to location. */
constexpr int PEMIT_ROOM = 32;    /* Send to containing room. */
constexpr int PEMIT_LIST = 64;    /* Send to a list. */

constexpr int SAY_SAY = 1;         /* Say in current room. */
constexpr int SAY_NOSPACE = 1;     /* Combine with emit for no-space form. */
constexpr int SAY_POSE = 2;        /* Pose in current room. */
constexpr int SAY_POSE_NOSPC = 3;  /* Pose without a space. */
constexpr int SAY_PREFIX = 4;      /* First character indicates formatting. */
constexpr int SAY_EMIT = 5;        /* Emit in current room. */
constexpr int SAY_SHOUT = 8;       /* Shout to logged-in players. */
constexpr int SAY_WALLPOSE = 9;    /* Pose to logged-in players. */
constexpr int SAY_WALLEMIT = 10;   /* Emit to logged-in players. */
constexpr int SAY_WIZSHOUT = 12;   /* Shout to logged-in wizards. */
constexpr int SAY_WIZPOSE = 13;    /* Pose to logged-in wizards. */
constexpr int SAY_WIZEMIT = 14;    /* Emit to logged-in wizards. */
constexpr int SAY_ADMINSHOUT = 15; /* Emit to administrators. */
constexpr int SAY_NOTAG = 32;      /* Do not add broadcast prefix. */
constexpr int SAY_HERE = 64;       /* Output to current location. */
constexpr int SAY_ROOM = 128;      /* Output to containing room. */

void do_pemit_list(EvaluationContext *evaluation, DbRef player, char *list,
                   const char *message);
