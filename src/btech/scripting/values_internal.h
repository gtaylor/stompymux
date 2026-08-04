#include "btech_event.h"                  // IWYU pragma: keep
#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/server/runtime_clock.h"     // IWYU pragma: keep

/*
 * $Id: values.c,v 1.5 2005/08/08 09:43:09 murrayma Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Created: Wed Oct  9 19:13:52 1996 fingon
 * Last modified: Tue Sep  8 10:00:29 1998 fingon
 *
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "econ_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_consistency_api.h"
#include "mech_damage_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_partnames.h"
#include "mech_partnames_api.h"
#include "mech_radio_api.h"
#include "mech_restrict_api.h"
#include "mech_sensor_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_damages_api.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/network/mux_event.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mycool.h"
#include "registry_api.h"
#include "registry_internal.h"
#include "special_object.h"
#include "template_api.h"
#include "turret.h"
#include "value_handlers_api.h"
#include "weapon_settings.h"

extern const BtechSpecialObjectDefinition SpecialObjects[];
char *mechref_path(BtechContext *context, const char *mech_path, char *id);
char *setarmorstatus_func(Mech *mech, char *sectstr, char *typestr,
                          char *valuestr);
typedef struct {
  int gtype;
  char *name;
  void *rel_addr;
  int type;
  int size;
} GMV;
enum {
  TYPE_STRING,
  TYPE_CHAR,
  TYPE_SHORT,
  TYPE_INT,
  TYPE_FLOAT,
  TYPE_DBREF,
  TYPE_STRFUNC,
  TYPE_STRFUNC_BUF,
  TYPE_STRFUNC_S,
  TYPE_BV,
  TYPE_STRFUNC_BD,
  TYPE_STRFUNC_BD_BUF,
  TYPE_CBV,
  TYPE_CHAR_RO,
  TYPE_SHORT_RO,
  TYPE_INT_RO,
  TYPE_FLOAT_RO,
  TYPE_DBREF_RO,
  TYPE_LAST_TYPE
};
#define Uglie(dat) ((void *)&dat((Mech *)0))
#define UglieV(dat, val) ((void *)&dat((Mech *)0, val))

#define MeEntry(Name, Func, Type) {GTYPE_MECH, Name, Uglie(Func), Type, 0}

#define MeEntryS(Name, Func, Type, Size)                                       \
  {GTYPE_MECH, Name, Uglie(Func), Type, Size}

#define MeVEntry(Name, Func, Val, Type)                                        \
  {GTYPE_MECH, Name, UglieV(Func, Val), Type, 0}

#define UglieM(dat) ((void *)&((BattleMap *)0)->dat)
#define MaEntry(Name, Func, Type) {GTYPE_MAP, Name, UglieM(Func), Type, 0}
#define MaEntryS(Name, Func, Type, Size)                                       \
  {GTYPE_MAP, Name, UglieM(Func), Type, Size}

#define UglieT(dat) (void *)&((Turret *)0)->dat

#define TuEntry(Name, Func, Type) {GTYPE_TURRET, Name, UglieT(Func), Type, 0}
#define TuEntryS(Name, Func, Type, Size)                                       \
  {GTYPE_TURRET, Name, UglieT(Func), Type, Size}

extern const int scode_in_out[TYPE_LAST_TYPE];
extern GMV xcode_data[];

int text2bv(char *text);
char *bv2text(int value, char *buffer);
char *mech_getset_ref(int mode, Mech *mech, char *data);
