#include <stddef.h>

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
extern const int DEFAULT_WEAPON_COUNT;
extern const int TEMPLATE_INTERNAL_COUNT;
extern const int TEMPLATE_CARGO_COUNT;
