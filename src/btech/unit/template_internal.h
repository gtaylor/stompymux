#include <stddef.h>

#include "template_api.h"

constexpr int MAX_STRING_LENGTH = 8192;

typedef enum TemplateMode : int {
  MODE_UNKNOWN = 0,
  MODE_NORMAL = 1,
} TemplateMode;

const char *const *template_load_command_names(void);
const char *const *template_internal_names(void);
size_t template_internal_name_count(void);
const char *template_internal_name(int index);
const char *const *template_cargo_names(void);
const char *template_cargo_name(int index);
const char *const *template_section_configuration_names(void);
const char *const *template_movement_type_names(void);
const char *const *template_unit_class_names(void);
const char *const *template_critical_fire_mode_names(void);
const char *const *template_critical_ammo_mode_names(void);
const char *const *primary_technology_names(void);
const char *const *primary_technology_abbreviations(void);
const char *const *secondary_technology_names(void);
const char *const *secondary_technology_abbreviations(void);
const char *const *infantry_technology_names(void);
const char *const *infantry_technology_abbreviations(void);
extern const int DEFAULT_WEAPON_COUNT;
extern const int TEMPLATE_INTERNAL_COUNT;
extern const int TEMPLATE_CARGO_COUNT;
