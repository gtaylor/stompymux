/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include <ctype.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "failures.h"
#include "legacy_macros.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_build_api.h"
#include "mech_contacts_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_parts.h"
#include "mech_scan_api.h"
#include "mech_status_api.h"
#include "mech_tag_api.h"
#include "mech_tech_do_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/lua/lua_runtime.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "weapon_settings.h"

void append_status(char *buffer, size_t size, const char *fmt, ...)
    __attribute__((format(printf, 3, 4)));
void print_weapon_status(EvaluationContext *evaluation, Mech *mech,
                         DbRef player, bool compact, char *compact_buffer,
                         size_t compact_buffer_size);

enum {
  PHY_AXE = 1,
  PHY_SWORD,
  PHY_MACE,
  PHY_SAW,
  PHY_CLAW,
};

extern const char *const lightmechdesc;
extern const char *const heavymechdesc;
extern const char *const mediummechdesc;
extern const char *const assaultmechdesc;
extern const char *const mechdesc;
extern const char *const quaddesc;
extern const char *const mwdesc;
extern const char *const shipdesc;
extern const char *const foildesc;
extern const char *const subdesc;
extern const char *const aerodesc;
extern const char *const spher_ds_desc;
extern const char *const aerod_ds_desc;
extern const char *const vehdesc;
extern const char *const veh_not_desc;
extern const char *const vtoldesc;
extern const char *const bsuitdesc;
