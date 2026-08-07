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
#include <stdbool.h>
#include <stddef.h>
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
#include "equipment_types.h"
#include "map.h"
#include "map_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
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
#include "mech_script_value_api.h"
#include "mech_sensor_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
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
#include "part_cost_api.h"
#include "registry_api.h"
#include "section_types.h"
#include "special_object.h"
#include "template_api.h"
#include "turret.h"
#include "unit_cost_api.h"
#include "value_handlers_api.h"
#include "weapon_settings.h"

char *mech_armor_status_set_value(Mech *mech, char *section, char *armor_type,
                                  char *value);

typedef enum GmvSourceKind {
  GMV_SOURCE_MECH_KEY,
  GMV_SOURCE_FIELD_OFFSET,
  GMV_SOURCE_CALLBACK,
  GMV_SOURCE_SENTINEL,
} GmvSourceKind;

typedef union GmvSource {
  MechScriptValueKey mech_key;
  size_t field_offset;
  char *(*string_callback)(int mode, Mech *mech);
  char *(*bidirectional_callback)(int mode, Mech *mech, char *value);
  char *(*buffered_callback)(Mech *mech, char *buffer);
  char *(*buffered_bidirectional_callback)(int mode, Mech *mech, char *value,
                                           char *buffer);
} GmvSource;

typedef struct {
  int gtype;
  char *name;
  GmvSourceKind source_kind;
  GmvSource source;
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
extern const int scode_in_out[TYPE_LAST_TYPE];
extern GMV xcode_data[];

int text2bv(char *text);
char *bv2text(int value, char *buffer);
char *mech_getset_ref(int mode, Mech *mech, char *data);
