/* Declares the BattleTech value handlers API. */

#pragma once

#include "mux/server/platform.h"
#include "script_functions_api.h"

#include "mux/commands/command_context.h"

/**
 * Context for a scripting value handler that supports both reads and writes
 * without using shared scratch storage.
 *
 * mode is zero for a read and nonzero for a write. value contains the new
 * value on writes and may be nullptr on reads. buffer is caller-owned storage
 * with LBUF_SIZE bytes available. The handler writes any result into buffer
 * and returns buffer.
 */
typedef struct GmvBufferedBidirectionalCall {
  int mode;
  Mech *mech;
  char *value;
  char *buffer;
} GmvBufferedBidirectionalCall;

char *mech_i_dfunc(Mech *mech, char buffer[static LBUF_SIZE]);
const char *mech_typefunc(int mode, Mech *mech, char *arg);
const char *mech_movefunc(int mode, Mech *mech, char *arg);
char *mech_tech_timefunc(Mech *mech, char buffer[static LBUF_SIZE]);
void apply_mech_damage(Mech *omech, char *buf);
char *mech_damagefunc(const GmvBufferedBidirectionalCall *call);
char *mech_getset_ref(const GmvBufferedBidirectionalCall *call);
char *mech_cent_bearingfunc(Mech *mech, char buffer[static LBUF_SIZE]);
char *mech_cent_distfunc(Mech *mech, char buffer[static LBUF_SIZE]);
void set_xcodestuff(DbRef player, void *data, char *buffer);
void list_xcodestuff(DbRef player, void *data, const char *buffer);
void list_xcodevalues(EvaluationContext *context, DbRef player);
