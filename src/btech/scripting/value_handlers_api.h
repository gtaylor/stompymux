/* Declares the BattleTech value handlers API. */

#pragma once

#include "mux/server/platform.h"
#include "script_functions_api.h"

#include "mux/commands/command_context.h"

/* values.c */
typedef struct GmvBufferedBidirectionalCall {
  int mode;
  Mech *mech;
  char *value;
  char *buffer;
} GmvBufferedBidirectionalCall;

char *mechIDfunc(Mech *mech, char buffer[static LBUF_SIZE]);
char *mechTypefunc(int mode, Mech *mech, char *arg);
char *mechMovefunc(int mode, Mech *mech, char *arg);
char *mechTechTimefunc(Mech *mech, char buffer[static LBUF_SIZE]);
void apply_mechDamage(Mech *omech, char *buf);
char *mechDamagefunc(const GmvBufferedBidirectionalCall *call);
char *mechCentBearingfunc(Mech *mech, char buffer[static LBUF_SIZE]);
char *mechCentDistfunc(Mech *mech, char buffer[static LBUF_SIZE]);
void set_xcodestuff(DbRef player, void *data, char *buffer);
void list_xcodestuff(DbRef player, void *data, char *buffer);
void list_xcodevalues(EvaluationContext *context, DbRef player);
