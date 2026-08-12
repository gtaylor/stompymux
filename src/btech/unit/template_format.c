#include "aero_bomb_api.h"
#include "btech_text_builder.h"
#include "checked_conversion.h"
#include "equipment_types.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_partnames_api.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "section_types.h"
#include "template_api.h"
#include "template_implementation.h"
#include "weapon_catalogue_api.h"
#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>

int compare_array(char *const list[], size_t count, const char *command) {
  if (!list)
    return -1;
  for (size_t index = 0; index < count; index++) {
    char *const *entry = (char *const *)checked_storage_at_const(
        (const void *)list, count, sizeof(*list), index);
    if (!strcasecmp(*entry, command))
      return clamp_size_to_int(index);
  }

  return -1;
}

int compare_const_array(const char *const list[], size_t count,
                        const char *command) {
  if (!list)
    return -1;
  for (size_t index = 0; index < count; index++) {
    const char *const *entry = (const char *const *)checked_storage_at_const(
        (const void *)list, count, sizeof(*list), index);
    if (!strcasecmp(*entry, command))
      return clamp_size_to_int(index);
  }
  return -1;
}

static int bit_is_set(int data, size_t index) {
  if (index >= sizeof(unsigned int) * CHAR_BIT)
    return 0;
  return (((unsigned int)data & (1U << index)) != 0U);
}

char *template_token_parse(const TemplateTokenRequest *request) {
  const char *separators =
      request->pipe_delimited ? " \t\n\v\f\r|" : " \t\n\v\f\r";
  size_t start = strspn(request->input, separators);
  char *word = checked_mutable_string_suffix(request->input, start);
  size_t source_length =
      strcspn(word, request->pipe_delimited ? "|" : " \t\n\v\f\r");
  size_t length = source_length;
  if (length >= request->output_capacity)
    length = request->output_capacity - 1;
  memcpy(request->output, word, length);
  char *terminator =
      checked_storage_at(request->output, request->output_capacity,
                         sizeof(*request->output), length);
  *terminator = '\0';
  return checked_mutable_string_suffix(word, source_length);
}

typedef struct BitNameAppendCall {
  BtechTextBuilder *builder;
  const TemplateBitSet *set;
  char delimiter;
} BitNameAppendCall;

static void append_bit_names(const BitNameAppendCall *call) {
  for (size_t index = 0; index < call->set->count; index++) {
    if (bit_is_set(call->set->bits, index)) {
      const char *const *description =
          (const char *const *)checked_storage_at_const(
              (const void *)call->set->descriptions, call->set->count,
              sizeof(*call->set->descriptions), index);
      btech_text_builder_append(call->builder, *description);
      btech_text_builder_append_character(call->builder, call->delimiter);
    }
  }
}

static void remove_trailing_delimiter(BtechTextBuilder *builder,
                                      char delimiter) {
  if (builder->length == 0)
    return;
  char *last = checked_storage_at(builder->text, builder->capacity,
                                  sizeof(*builder->text), builder->length - 1);
  if (*last == delimiter) {
    *last = '\0';
    builder->length--;
  }
}

char *template_bit_string_build(const TemplateBitStringRequest *request) {
  BtechTextBuilder builder;
  btech_text_builder_initialize(&builder, request->buffer, BTECH_TEXT_CAPACITY);
  for (size_t index = 0; index < request->set_count; index++) {
    const TemplateBitSet *set = checked_storage_at_const(
        request->sets, request->set_count, sizeof(*request->sets), index);
    append_bit_names(&(BitNameAppendCall){
        .builder = &builder, .set = set, .delimiter = request->delimiter});
  }
  remove_trailing_delimiter(&builder, request->delimiter);
  return request->buffer;
}

static int invalid_vehicle_item(Mech *mech, int x, int y) {
  int t;

  t = mech_critical_part_type(mech, x, y);
  switch (t) {
  case SPECIAL_BASE_INDEX + SHOULDER_OR_HIP:
  case SPECIAL_BASE_INDEX + UPPER_ACTUATOR:
  case SPECIAL_BASE_INDEX + LOWER_ACTUATOR:
  case SPECIAL_BASE_INDEX + HAND_OR_FOOT_ACTUATOR:
  case SPECIAL_BASE_INDEX + ENGINE:
  case SPECIAL_BASE_INDEX + GYRO:
  case SPECIAL_BASE_INDEX + JUMP_JET:
    return 1;
  }
  return 0;
}

typedef struct PartWeaponNameResult {
  bool allowed;
  bool is_clan;
} PartWeaponNameResult;

static PartWeaponNameResult
part_weapon_name_check(const PartNameRequest *request, int weapon) {
#ifndef CLAN_SUPPORT
  (void)request;
  return (PartWeaponNameResult){
      .allowed = !weapon_catalogue_has_special(weapon, CLAT)};
#else
  PartWeaponNameResult result = {.allowed = true};
  if (request->brand) {
    if (!request->configuration->btech_parts ||
        weapon_catalogue_has_special(weapon, CLAT))
      return (PartWeaponNameResult){0};
    if (weapon_catalogue_has_special(weapon, CLAT))
      result.is_clan = true;
  }
  return result;
#endif
}

static size_t part_weapon_short_name_offset(int weapon) {
#ifdef CLAN_SUPPORT
  return !strncasecmp(weapon_catalogue_name(weapon), "CL.", 2) ? 0 : 3;
#else
  (void)weapon;
  return 3;
#endif
}

char *part_name_format(const PartNameRequest *request) {
  int i = request->part;
  const char *source;

  if (!i)
    return nullptr;
  if (equipment_is_weapon(i) &&
      i < weapon_equipment_index(DEFAULT_WEAPON_COUNT)) {
    PartWeaponNameResult result =
        part_weapon_name_check(request, weapon_from_equipment_index(i));
    if (!result.allowed)
      return nullptr;
    source = checked_string_suffix(
        weapon_catalogue_name(weapon_from_equipment_index(i)),
        (request->short_name && !result.is_clan) ? 3 : 0);
    (void)snprintf(request->buffer, BTECH_TEXT_CAPACITY, "%s", source);
    return request->buffer;
  }
  if (equipment_is_ammunition(i) &&
      i < ammunition_equipment_index(DEFAULT_WEAPON_COUNT)) {
    PartWeaponNameResult result =
        part_weapon_name_check(request, ammunition_to_weapon_index(i));
    if (!result.allowed)
      return nullptr;
    int weapon = ammunition_to_weapon_index(i);
    if (weapon_catalogue_type(weapon) != TBEAM &&
        weapon_catalogue_type(weapon) != THAND &&
        !weapon_catalogue_has_special(weapon, PCOMBAT)) {
      (void)snprintf(request->buffer, BTECH_TEXT_CAPACITY, "Ammo_%s",
                     checked_string_suffix(
                         weapon_catalogue_name(weapon),
                         (request->short_name && !result.is_clan ? 3 : 0)));
      return request->buffer;
    }
  } else if (!request->brand) {
    if (equipment_is_bomb(i)) {
      (void)snprintf(request->buffer, BTECH_TEXT_CAPACITY, "Bomb_%s",
                     bomb_name(bomb_from_equipment_index(i)));
      return request->buffer;
    }
    if (equipment_is_special(i) &&
        i < special_equipment_index(TEMPLATE_INTERNAL_COUNT)) {
      (void)snprintf(request->buffer, BTECH_TEXT_CAPACITY, "%s",
                     template_internal_name(special_from_equipment_index(i)));
      return request->buffer;
    }
    if (equipment_is_cargo(i) &&
        i < cargo_equipment_index(TEMPLATE_CARGO_COUNT)) {
      (void)snprintf(request->buffer, BTECH_TEXT_CAPACITY, "%s",
                     template_cargo_name(cargo_from_equipment_index(i)));
      return request->buffer;
    }
  }
  return nullptr;
}

char *my_shortform(const char *source,
                   char buffer[static BTECH_TEXT_CAPACITY]) {
  if (!source)
    return nullptr;
  if (strlen(source) <= 4 && !strchr(source, '/')) {
    (void)snprintf(buffer, BTECH_TEXT_CAPACITY, "%s", source);
  } else {
    size_t source_length = strlen(source);
    size_t output_length = 0;
    for (size_t index = 0;
         index < source_length && output_length < BTECH_TEXT_CAPACITY - 1;
         index++) {
      const char *character = checked_storage_at_const(
          source, source_length + 1, sizeof(*source), index);
      if ((*character >= '0' && *character <= '9') ||
          (*character >= 'A' && *character <= 'Z') || *character == '_') {
        char *destination = checked_storage_at(
            buffer, BTECH_TEXT_CAPACITY, sizeof(*buffer), output_length++);
        *destination = *character;
      }
    }
    if (output_length == 1 && source_length > 1 &&
        output_length < BTECH_TEXT_CAPACITY - 1) {
      const char *second = checked_storage_at_const(source, source_length + 1,
                                                    sizeof(*source), 1);
      char *destination = checked_storage_at(buffer, BTECH_TEXT_CAPACITY,
                                             sizeof(*buffer), output_length++);
      *destination = *second;
    }
    char *terminator = checked_storage_at(buffer, BTECH_TEXT_CAPACITY,
                                          sizeof(*buffer), output_length);
    *terminator = '\0';
  }
  return buffer;
}

char *part_figure_out_shname(int i, char buffer[static BTECH_TEXT_CAPACITY]) {
  char name[BTECH_TEXT_CAPACITY] = {0};

  if (!i)
    return nullptr;
  if (equipment_is_weapon(i) &&
      i < weapon_equipment_index(DEFAULT_WEAPON_COUNT)) {
    (void)snprintf(
        name, sizeof(name), "%s",
        checked_string_suffix(
            weapon_catalogue_name(weapon_from_equipment_index(i)),
            part_weapon_short_name_offset(weapon_from_equipment_index(i))));
  } else if (equipment_is_ammunition(i) &&
             i < ammunition_equipment_index(DEFAULT_WEAPON_COUNT)) {
    (void)snprintf(
        name, sizeof(name), "Ammo_%s",
        checked_string_suffix(
            weapon_catalogue_name(ammunition_to_weapon_index(i)),
            part_weapon_short_name_offset(ammunition_to_weapon_index(i))));
  } else if (equipment_is_bomb(i)) {
    (void)snprintf(name, sizeof(name), "Bomb_%s",
                   bomb_name(bomb_from_equipment_index(i)));
  } else if (equipment_is_special(i) &&
             i < special_equipment_index(TEMPLATE_INTERNAL_COUNT)) {
    (void)snprintf(name, sizeof(name), "%s",
                   template_internal_name(special_from_equipment_index(i)));
  }
  if (equipment_is_cargo(i) && i < cargo_equipment_index(TEMPLATE_CARGO_COUNT))
    (void)snprintf(name, sizeof(name), "%s",
                   template_cargo_name(cargo_from_equipment_index(i)));
  if (!name[0])
    return nullptr;
  return my_shortform(name, buffer);
}

static int dump_item(FILE *fp, Mech *mech, int x, int y) {
  char crit[32];
  int y1;
  int flaggo = 0;
  int z;
  int w_fire_modes, w_ammo_modes;

  if (!mech_critical_part_type(mech, x, y))
    return 1;
  if (((mech)->ud.type) != CLASS_MECH && invalid_vehicle_item(mech, x, y))
    return 1;
  for (y1 = y + 1; y1 < 12; y1++) {
    if (mech_critical_part_type(mech, x, y1) !=
        mech_critical_part_type(mech, x, y))
      break;
    if (mech_critical_data(mech, x, y1) != mech_critical_data(mech, x, y))
      break;
    if (mech_critical_fire_mode(mech, x, y1) !=
        mech_critical_fire_mode(mech, x, y))
      break;
    if (mech_critical_ammo_mode(mech, x, y1) !=
        mech_critical_ammo_mode(mech, x, y))
      break;
    if (mech->xcode.context->configuration->btech_parts)
      if (mech_critical_brand(mech, x, y1) != mech_critical_brand(mech, x, y))
        break;
  }
  y1--;
  if (equipment_is_weapon(mech_critical_part_type(mech, x, y))) {
    /* Nonbeams, or flamers don't have TC */
    if (!equipment_can_use_targeting_computer(
            mech_critical_part_type(mech, x, y)))
      flaggo = ON_TC;
    z = get_weapon_crits(
        mech, weapon_from_equipment_index(mech_critical_part_type(mech, x, y)));
    if (((y1 - y) + 1) > z)
      y1 = y + z - 1;
  }
  if (y != y1)
    (void)snprintf(crit, 32, "CRIT_%d-%d", y + 1, y1 + 1);
  else
    (void)snprintf(crit, 32, "CRIT_%d", y + 1);

  w_fire_modes = mech_critical_fire_mode(mech, x, y);
  w_fire_modes &= ~flaggo;
  w_ammo_modes = mech_critical_ammo_mode(mech, x, y);

  if (equipment_is_weapon(mech_critical_part_type(mech, x, y))) {
    (void)fprintf(
        fp, "    %s		  { %s - %s %s}\n", crit,
        get_parts_vlong_name(mech->xcode.context,
                             mech_critical_part_type(mech, x, y), 0),
        (w_fire_modes || w_ammo_modes)
            ? template_bit_string_build(&(TemplateBitStringRequest){
                  .sets =
                      (TemplateBitSet[]){
                          {.descriptions = crit_fire_modes,
                           .count = template_critical_fire_mode_count(),
                           .bits = w_fire_modes},
                          {.descriptions = crit_ammo_modes,
                           .count = template_critical_ammo_mode_count(),
                           .bits = w_ammo_modes}},
                  .set_count = 2,
                  .delimiter = '|',
                  .buffer = (char[BTECH_TEXT_CAPACITY]){0}})
            : "-",
        !mech->xcode.context->configuration->btech_parts
            ? ""
            : tprintf("%d ", mech_critical_brand(mech, x, y)));
  } else if (equipment_is_ammunition(mech_critical_part_type(mech, x, y))) {
    (void)fprintf(
        fp, "    %s		  { %s %d %s - }\n", crit,
        get_parts_vlong_name(mech->xcode.context,
                             mech_critical_part_type(mech, x, y), 0),
        full_ammo(mech, x, y),
        (mech_critical_fire_mode(mech, x, y) ||
         mech_critical_ammo_mode(mech, x, y))
            ? template_bit_string_build(&(TemplateBitStringRequest){
                  .sets =
                      (TemplateBitSet[]){
                          {.descriptions = crit_fire_modes,
                           .count = template_critical_fire_mode_count(),
                           .bits = mech_critical_fire_mode(mech, x, y)},
                          {.descriptions = crit_ammo_modes,
                           .count = template_critical_ammo_mode_count(),
                           .bits = mech_critical_ammo_mode(mech, x, y)}},
                  .set_count = 2,
                  .delimiter = '|',
                  .buffer = (char[BTECH_TEXT_CAPACITY]){0}})
            : "-");
  } else if (equipment_is_bomb(mech_critical_part_type(mech, x, y))) {
    (void)fprintf(fp, "    %s		  { %s - - - }\n", crit,
                  get_parts_vlong_name(mech->xcode.context,
                                       mech_critical_part_type(mech, x, y), 0));
  } else {
    (void)fprintf(fp, "    %s		  { %s %s - %s}\n", crit,
                  get_parts_vlong_name(mech->xcode.context,
                                       mech_critical_part_type(mech, x, y), 0),
                  mech_critical_data(mech, x, y)
                      ? tprintf("%d", mech_critical_data(mech, x, y))
                      : "-",
                  !mech->xcode.context->configuration->btech_parts
                      ? ""
                      : tprintf("%d ", mech_critical_brand(mech, x, y)));
  }
  return (y1 - y + 1);
}

void dump_locations(FILE *fp, Mech *mech, const char *const locdesc[],
                    size_t location_count) {
  int x, y, l;
  char buf[512];

  for (x = 0; (size_t)x < location_count; x++) {
    if (!mech_section_original_internal(mech, x))
      continue;
    const char *const *location = (const char *const *)checked_storage_at_const(
        (const void *)locdesc, location_count, sizeof(*locdesc), (size_t)x);
    strlcpy(buf, *location, sizeof(buf));
    size_t location_length = strlen(buf);
    for (size_t index = 0; index < location_length; index++) {
      char *character =
          checked_storage_at(buf, sizeof(buf), sizeof(*buf), index);
      if (*character == ' ')
        *character = '_';
    }
    (void)fprintf(fp, "%s\n", buf);
    if (mech_section_original_armor(mech, x))
      (void)fprintf(fp, "  Armor            { %d }\n",
                    mech_section_original_armor(mech, x));
    if (mech_section_original_internal(mech, x))
      (void)fprintf(fp, "  Internals        { %d }\n",
                    mech_section_original_internal(mech, x));
    if (mech_section_original_rear_armor(mech, x))
      (void)fprintf(fp, "  Rear             { %d }\n",
                    mech_section_original_rear_armor(mech, x));
    y = mech_section_configuration(mech, x);
    y &= ~CASE_TECH;
    if (y)
      (void)fprintf(fp, "  Config           { %s }\n",
                    template_bit_string_build(&(TemplateBitStringRequest){
                        .sets =
                            &(TemplateBitSet){
                                .descriptions = section_configs,
                                .count = template_section_configuration_count(),
                                .bits = y},
                        .set_count = 1,
                        .delimiter = ' ',
                        .buffer = (char[BTECH_TEXT_CAPACITY]){0}}));
    l = crits_in_loc(mech, x);
    for (y = 0; y < l;)
      y += dump_item(fp, mech, x, y);
  }
}

float generic_computer_multiplier(Mech *mech) {
  switch (mech_computer_quality(mech)) {
  case 1:
    return 0.8F;
  case 2:
    return 1.0F;
  case 3:
    return 1.25F;
  case 4:
    return 1.5F;
  case 5:
    return 1.75F;
  }
  return 0.0F;
}

int generic_radio_type(int i, int is_clan) {
  int f = DEFAULT_FREQS;

  if (is_clan || i >= 4)
    f += FREQS * RADIO_RELAY;
  if (i < 3)
    f -= (3 - i) * 2 - 1; /* 2 or 4 */
  else
    f += (i - 3) * 3; /* 5 / 8 / 11 */
  return f;
}

float generic_radio_multiplier(Mech *mech) {
  switch (mech_radio_quality(mech)) {
  case 1:
    return 0.8F;
  case 2:
    return 1.0F;
  case 3:
    return 1.25F;
  case 4:
    return 1.5F;
  case 5:
    return 1.75F;
  }
  return 0.0F;
}

void computer_conversion(Mech *mech) {
  int l = 0;

  switch (mech_scanner_range(mech)) {
  case 20:
    l = 2;
    break;
  case 25:
    l = 3;
    break;
  case 30:
    l = 4;
    break;
  }
  if (l) {
    mech_computer_quality_set(mech, l);
    mech_scanner_range_set(mech, mech_default_scanner_range(mech));
    mech_tactical_range_set(mech, mech_default_tactical_range(mech));
    mech_long_range_sensor_range_set(
        mech, mech_default_long_range_sensor_range(mech));
    mech_radio_range_set(mech, mech_default_radio_range(mech));
  }
}
