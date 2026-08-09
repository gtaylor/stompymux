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
    char *const *entry =
        checked_storage_at_const(list, count, sizeof(*list), index);
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
    const char *const *entry =
        checked_storage_at_const(list, count, sizeof(*list), index);
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

char *one_arg(char *argument, char *first_arg, size_t first_arg_capacity) {
  size_t start = strspn(argument, " \t\n\v\f\r");
  char *word = checked_mutable_string_suffix(argument, start);
  size_t length = strcspn(word, " \t\n\v\f\r");
  if (length >= first_arg_capacity)
    length = first_arg_capacity - 1;
  memcpy(first_arg, word, length);
  char *terminator = checked_storage_at(first_arg, first_arg_capacity,
                                        sizeof(*first_arg), length);
  *terminator = '\0';
  return checked_mutable_string_suffix(word, strcspn(word, " \t\n\v\f\r"));
}

char *one_arg_delim(char *argument, char *first_arg,
                    size_t first_arg_capacity) {
  size_t start = strspn(argument, " \t\n\v\f\r|");
  char *word = checked_mutable_string_suffix(argument, start);
  size_t source_length = strcspn(word, "|");
  size_t copy_length = source_length;
  if (copy_length >= first_arg_capacity)
    copy_length = first_arg_capacity - 1;
  memcpy(first_arg, word, copy_length);
  char *terminator = checked_storage_at(first_arg, first_arg_capacity,
                                        sizeof(*first_arg), copy_length);
  *terminator = '\0';
  return checked_mutable_string_suffix(word, source_length);
}

static void append_bit_names(BtechTextBuilder *builder,
                             const char *const descriptions[], size_t count,
                             int data, char delimiter) {
  for (size_t index = 0; index < count; index++) {
    if (bit_is_set(data, index)) {
      const char *const *description = checked_storage_at_const(
          descriptions, count, sizeof(*descriptions), index);
      btech_text_builder_append(builder, *description);
      btech_text_builder_append_character(builder, delimiter);
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

char *build_bit_string(const char *const bitdescs[], size_t count, int data,
                       char *buffer) {
  BtechTextBuilder builder;
  btech_text_builder_initialize(&builder, buffer, BTECH_TEXT_CAPACITY);
  append_bit_names(&builder, bitdescs, count, data, ' ');
  remove_trailing_delimiter(&builder, ' ');
  return buffer;
}

char *build_bit_string2(const char *const bitdescs[], size_t count,
                        const char *const bitdescs2[], size_t count2, int data,
                        int data2, char *buffer) {
  BtechTextBuilder builder;
  btech_text_builder_initialize(&builder, buffer, BTECH_TEXT_CAPACITY);
  append_bit_names(&builder, bitdescs, count, data, ' ');
  append_bit_names(&builder, bitdescs2, count2, data2, ' ');
  remove_trailing_delimiter(&builder, ' ');
  return buffer;
}

char *build_bit_string_delimited2(const char *const bitdescs[], size_t count,
                                  const char *const bitdescs2[], size_t count2,
                                  int data, int data2, char *buffer) {
  BtechTextBuilder builder;
  btech_text_builder_initialize(&builder, buffer, BTECH_TEXT_CAPACITY);
  append_bit_names(&builder, bitdescs, count, data, '|');
  append_bit_names(&builder, bitdescs2, count2, data2, '|');
  remove_trailing_delimiter(&builder, '|');
  return buffer;
}

char *build_bit_string3(const char *const bitdescs[], size_t count,
                        const char *const bitdescs2[], size_t count2,
                        const char *const bitdescs3[], size_t count3, int data,
                        int data2, int data3, char *buffer) {
  BtechTextBuilder builder;
  btech_text_builder_initialize(&builder, buffer, BTECH_TEXT_CAPACITY);
  append_bit_names(&builder, bitdescs, count, data, ' ');
  append_bit_names(&builder, bitdescs2, count2, data2, ' ');
  append_bit_names(&builder, bitdescs3, count3, data3, ' ');
  remove_trailing_delimiter(&builder, ' ');
  return buffer;
}

static int InvalidVehicleItem(Mech *mech, int x, int y) {
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

static bool part_weapon_name_allowed(const ServerConfiguration *configuration,
                                     int weapon, int brand, int *is_clan) {
#ifndef CLAN_SUPPORT
  (void)configuration;
  (void)brand;
  (void)is_clan;
  return !weapon_catalogue_has_special(weapon, CLAT);
#else
  if (brand) {
    if (!configuration->btech_parts ||
        weapon_catalogue_has_special(weapon, CLAT))
      return false;
    if (weapon_catalogue_has_special(weapon, CLAT))
      *is_clan = 1;
  }
  return true;
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

static char *part_figure_out_name_sub(const ServerConfiguration *configuration,
                                      int i, int j, int brand,
                                      char buffer[static BTECH_TEXT_CAPACITY]) {
  int isclan = 0;
  const char *source;

  if (!i)
    return nullptr;
  if (equipment_is_weapon(i) && i < weapon_equipment_index(num_def_weapons)) {
    if (!part_weapon_name_allowed(configuration, weapon_from_equipment_index(i),
                                  brand, &isclan))
      return nullptr;
    source = checked_string_suffix(
        weapon_catalogue_name(weapon_from_equipment_index(i)),
        (j && !isclan) ? 3 : 0);
    snprintf(buffer, BTECH_TEXT_CAPACITY, "%s", source);
    return buffer;
  } else if (equipment_is_ammunition(i) &&
             i < ammunition_equipment_index(num_def_weapons)) {
    if (!part_weapon_name_allowed(configuration, ammunition_to_weapon_index(i),
                                  brand, &isclan))
      return nullptr;
    int weapon = ammunition_to_weapon_index(i);
    if (weapon_catalogue_type(weapon) != TBEAM &&
        weapon_catalogue_type(weapon) != THAND &&
        !weapon_catalogue_has_special(weapon, PCOMBAT)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "Ammo_%s",
               checked_string_suffix(weapon_catalogue_name(weapon),
                                     (j && !isclan) ? 3 : 0));
      return buffer;
    }
  } else if (!brand) {
    if (equipment_is_bomb(i)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "Bomb_%s",
               bomb_name(bomb_from_equipment_index(i)));
      return buffer;
    } else if (equipment_is_special(i) &&
               i < special_equipment_index(template_internal_count)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "%s",
               template_internal_name(special_from_equipment_index(i)));
      return buffer;
    } else if (equipment_is_cargo(i) &&
               i < cargo_equipment_index(template_cargo_count)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "%s",
               template_cargo_name(cargo_from_equipment_index(i)));
      return buffer;
    }
  }
  return nullptr;
}

char *my_shortform(const char *source,
                   char buffer[static BTECH_TEXT_CAPACITY]) {
  if (!source)
    return nullptr;
  if (strlen(source) <= 4 && !strchr(source, '/'))
    snprintf(buffer, BTECH_TEXT_CAPACITY, "%s", source);
  else {
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
  if (equipment_is_weapon(i) && i < weapon_equipment_index(num_def_weapons)) {
    snprintf(
        name, sizeof(name), "%s",
        checked_string_suffix(
            weapon_catalogue_name(weapon_from_equipment_index(i)),
            part_weapon_short_name_offset(weapon_from_equipment_index(i))));
  } else if (equipment_is_ammunition(i) &&
             i < ammunition_equipment_index(num_def_weapons)) {
    snprintf(name, sizeof(name), "Ammo_%s",
             checked_string_suffix(
                 weapon_catalogue_name(ammunition_to_weapon_index(i)),
                 part_weapon_short_name_offset(ammunition_to_weapon_index(i))));
  } else if (equipment_is_bomb(i))
    snprintf(name, sizeof(name), "Bomb_%s",
             bomb_name(bomb_from_equipment_index(i)));
  else if (equipment_is_special(i) &&
           i < special_equipment_index(template_internal_count))
    snprintf(name, sizeof(name), "%s",
             template_internal_name(special_from_equipment_index(i)));
  if (equipment_is_cargo(i) && i < cargo_equipment_index(template_cargo_count))
    snprintf(name, sizeof(name), "%s",
             template_cargo_name(cargo_from_equipment_index(i)));
  if (!name[0])
    return nullptr;
  return my_shortform(name, buffer);
}

char *part_figure_out_name(const ServerConfiguration *configuration, int i,
                           int brand, char buffer[static BTECH_TEXT_CAPACITY]) {
  return part_figure_out_name_sub(configuration, i, 0, brand, buffer);
}

char *part_figure_out_sname(const ServerConfiguration *configuration, int i,
                            int brand,
                            char buffer[static BTECH_TEXT_CAPACITY]) {
  return part_figure_out_name_sub(configuration, i, 1, brand, buffer);
}

static int dump_item(FILE *fp, Mech *mech, int x, int y) {
  char crit[32];
  int y1;
  int flaggo = 0;
  int z;
  int wFireModes, wAmmoModes;

  if (!mech_critical_part_type(mech, x, y))
    return 1;
  if (((mech)->ud.type) != CLASS_MECH && InvalidVehicleItem(mech, x, y))
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
    if (((y1 - y) + 1) >
        (z = GetWeaponCrits(mech, weapon_from_equipment_index(
                                      mech_critical_part_type(mech, x, y)))))
      y1 = y + z - 1;
  }
  if (y != y1)
    snprintf(crit, 32, "CRIT_%d-%d", y + 1, y1 + 1);
  else
    snprintf(crit, 32, "CRIT_%d", y + 1);

  wFireModes = mech_critical_fire_mode(mech, x, y);
  wFireModes &= ~flaggo;
  wAmmoModes = mech_critical_ammo_mode(mech, x, y);

  if (equipment_is_weapon(mech_critical_part_type(mech, x, y)))
    fprintf(fp, "    %s		  { %s - %s %s}\n", crit,
            get_parts_vlong_name(mech->xcode.context,
                                 mech_critical_part_type(mech, x, y), 0),
            (wFireModes || wAmmoModes)
                ? build_bit_string_delimited2(
                      crit_fire_modes, template_critical_fire_mode_count(),
                      crit_ammo_modes, template_critical_ammo_mode_count(),
                      wFireModes, wAmmoModes, (char[BTECH_TEXT_CAPACITY]){0})
                : "-",
            !mech->xcode.context->configuration->btech_parts
                ? ""
                : tprintf("%d ", mech_critical_brand(mech, x, y)));
  else if (equipment_is_ammunition(mech_critical_part_type(mech, x, y)))
    fprintf(fp, "    %s		  { %s %d %s - }\n", crit,
            get_parts_vlong_name(mech->xcode.context,
                                 mech_critical_part_type(mech, x, y), 0),
            FullAmmo(mech, x, y),
            (mech_critical_fire_mode(mech, x, y) ||
             mech_critical_ammo_mode(mech, x, y))
                ? build_bit_string_delimited2(
                      crit_fire_modes, template_critical_fire_mode_count(),
                      crit_ammo_modes, template_critical_ammo_mode_count(),
                      mech_critical_fire_mode(mech, x, y),
                      mech_critical_ammo_mode(mech, x, y),
                      (char[BTECH_TEXT_CAPACITY]){0})
                : "-");
  else if (equipment_is_bomb(mech_critical_part_type(mech, x, y)))
    fprintf(fp, "    %s		  { %s - - - }\n", crit,
            get_parts_vlong_name(mech->xcode.context,
                                 mech_critical_part_type(mech, x, y), 0));
  else {
    fprintf(fp, "    %s		  { %s %s - %s}\n", crit,
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
    const char *const *location = checked_storage_at_const(
        locdesc, location_count, sizeof(*locdesc), (size_t)x);
    strlcpy(buf, *location, sizeof(buf));
    size_t location_length = strlen(buf);
    for (size_t index = 0; index < location_length; index++) {
      char *character =
          checked_storage_at(buf, sizeof(buf), sizeof(*buf), index);
      if (*character == ' ')
        *character = '_';
    }
    fprintf(fp, "%s\n", buf);
    if (mech_section_original_armor(mech, x))
      fprintf(fp, "  Armor            { %d }\n",
              mech_section_original_armor(mech, x));
    if (mech_section_original_internal(mech, x))
      fprintf(fp, "  Internals        { %d }\n",
              mech_section_original_internal(mech, x));
    if (mech_section_original_rear_armor(mech, x))
      fprintf(fp, "  Rear             { %d }\n",
              mech_section_original_rear_armor(mech, x));
    y = mech_section_configuration(mech, x);
    y &= ~CASE_TECH;
    if (y)
      fprintf(fp, "  Config           { %s }\n",
              build_bit_string(section_configs,
                               template_section_configuration_count(), y,
                               (char[BTECH_TEXT_CAPACITY]){0}));
    l = CritsInLoc(mech, x);
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

int generic_radio_type(int i, int isClan) {
  int f = DEFAULT_FREQS;

  if (isClan || i >= 4)
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
