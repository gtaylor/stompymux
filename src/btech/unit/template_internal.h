#include "btech_event.h" // IWYU pragma: keep
#include "map.h"         // IWYU pragma: keep
#include "map_terrain.h"
#include "mux/server/runtime_clock.h" // IWYU pragma: keep
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "mech_lifecycle.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "weapon_settings.h"
#include "mux/server/platform.h"

#include <ctype.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>

#include "aero_bomb_api.h"
#include "bsuit_api.h"
#include "coolmenu.h"
#include "map_conditions_api.h"
#include "mech_c3_api.h"
#include "mech_consistency_api.h"
#include "mech_internal.h"
#include "mech_mechref_ident_api.h"
#include "mech_partnames_api.h"
#include "mech_utils_api.h"
#include "template_api.h"

constexpr int MAX_STRING_LENGTH = 8192;

typedef enum TemplateMode : int {
  MODE_UNKNOWN = 0,
  MODE_NORMAL = 1,
} TemplateMode;

extern const char *load_cmds[];
extern const char *internals[];
size_t template_internal_name_count(void);
const char *template_internal_name(int index);
extern const char *cargo[];
const char *template_cargo_name(int index);
extern const char *section_configs[];
extern const char *move_types[];
extern const char *mech_types[];
extern const char *crit_fire_modes[];
extern const char *crit_ammo_modes[];
extern const char *specials[];
extern const char *specialsabrev[];
extern const char *specials2[];
extern const char *specialsabrev2[];
extern const char *infantry_specials[];
extern const char *infspecialsabrev[];
extern const int num_def_weapons;
extern const int template_internal_count;
extern const int template_cargo_count;
