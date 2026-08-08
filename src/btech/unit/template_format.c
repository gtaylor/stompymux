#include "checked_conversion.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "template_internal.h"
#include "weapon_catalogue_api.h"

int compare_array(char *list[], char *command) {
  int x;

  if (!list)
    return -1;
  for (x = 0; list[x]; x++)
    if (!strcasecmp(list[x], command))
      return x;

  return -1;
}

int compare_const_array(const char *const list[], const char *command) {
  if (!list)
    return -1;
  for (int index = 0; list[index]; index++)
    if (!strcasecmp(list[index], command))
      return index;
  return -1;
}

static int bit_is_set(int data, size_t index) {
  if (index >= sizeof(unsigned int) * CHAR_BIT)
    return 0;
  return (((unsigned int)data & (1U << index)) != 0U);
}

char *one_arg(char *argument, char *first_arg) {
  while (*argument && isspace((unsigned char)*argument))
    argument++;

  while (*argument && !isspace((unsigned char)*argument))
    *(first_arg++) = *(argument++);
  *first_arg = '\0';
  return argument;
}

char *one_arg_delim(char *argument, char *first_arg) {
  while (*argument && (isspace((unsigned char)*argument) || *argument == '|'))
    argument++;

  while (*argument && (!(*argument == '|')))
    *(first_arg++) = *(argument++);

  *first_arg = '\0';
  return argument;
}

char *build_bit_string(const char *const bitdescs[], int data, char *buffer) {
  size_t length;

  buffer[0] = 0;
  for (size_t index = 0; bitdescs[index]; index++) {
    if (bit_is_set(data, index)) {
      strcat(buffer, bitdescs[index]);
      strcat(buffer, " ");
    }
  }
  length = strlen(buffer);
  if (length > 0 && buffer[length - 1] == ' ')
    buffer[length - 1] = '\0';
  return buffer;
}

char *build_bit_string2(const char *const bitdescs[],
                        const char *const bitdescs2[], int data, int data2,
                        char *buffer) {
  size_t length;

  buffer[0] = 0;

  for (size_t index = 0; bitdescs[index]; index++) {
    if (bit_is_set(data, index)) {
      strcat(buffer, bitdescs[index]);
      strcat(buffer, " ");
    }
  }

  for (size_t index = 0; bitdescs2[index]; index++) {
    if (bit_is_set(data2, index)) {
      strcat(buffer, bitdescs2[index]);
      strcat(buffer, " ");
    }
  }

  length = strlen(buffer);
  if (length > 0 && buffer[length - 1] == ' ') {
    buffer[length - 1] = '\0';
  }

  return buffer;
}

char *build_bit_string_delimited2(const char *const bitdescs[],
                                  const char *const bitdescs2[], int data,
                                  int data2, char *buffer) {
  size_t length;

  buffer[0] = 0;

  for (size_t index = 0; bitdescs[index]; index++) {
    if (bit_is_set(data, index)) {
      strcat(buffer, bitdescs[index]);
      strcat(buffer, "|");
    }
  }

  for (size_t index = 0; bitdescs2[index]; index++) {
    if (bit_is_set(data2, index)) {
      strcat(buffer, bitdescs2[index]);
      strcat(buffer, "|");
    }
  }

  length = strlen(buffer);
  if (length > 0 && buffer[length - 1] == '|') {
    buffer[length - 1] = '\0';
  }

  return buffer;
}

char *build_bit_string3(const char *const bitdescs[],
                        const char *const bitdescs2[],
                        const char *const bitdescs3[], int data, int data2,
                        int data3, char *buffer) {
  size_t length;

  buffer[0] = 0;

  for (size_t index = 0; bitdescs[index]; index++) {
    if (bit_is_set(data, index)) {
      strcat(buffer, bitdescs[index]);
      strcat(buffer, " ");
    }
  }

  for (size_t index = 0; bitdescs2[index]; index++) {
    if (bit_is_set(data2, index)) {
      strcat(buffer, bitdescs2[index]);
      strcat(buffer, " ");
    }
  }

  for (size_t index = 0; bitdescs3[index]; index++) {
    if (bit_is_set(data3, index)) {
      strcat(buffer, bitdescs3[index]);
      strcat(buffer, " ");
    }
  }

  length = strlen(buffer);
  if (length > 0 && buffer[length - 1] == ' ')
    buffer[length - 1] = '\0';

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
  return !(MechWeapons[weapon].special & CLAT);
#else
  if (brand) {
    if (!configuration->btech_parts || (MechWeapons[weapon].special & CLAT))
      return false;
    if (MechWeapons[weapon].special & CLAT)
      *is_clan = 1;
  }
  return true;
#endif
}

static int part_weapon_short_name_offset(int weapon) {
#ifdef CLAN_SUPPORT
  return !strncasecmp(MechWeapons[weapon].name, "CL.", 2) ? 0 : 3;
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
    source = &MechWeapons[weapon_from_equipment_index(i)]
                  .name[(j && !isclan) ? 3 : 0];
    snprintf(buffer, BTECH_TEXT_CAPACITY, "%s", source);
    return buffer;
  } else if (equipment_is_ammunition(i) &&
             i < ammunition_equipment_index(num_def_weapons)) {
    if (!part_weapon_name_allowed(configuration, ammunition_to_weapon_index(i),
                                  brand, &isclan))
      return nullptr;
    if (MechWeapons[ammunition_to_weapon_index(i)].type != TBEAM &&
        MechWeapons[ammunition_to_weapon_index(i)].type != THAND &&
        !(MechWeapons[ammunition_to_weapon_index(i)].special & PCOMBAT)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "Ammo_%s",
               &MechWeapons[ammunition_to_weapon_index(i)]
                    .name[(j && !isclan) ? 3 : 0]);
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
               internals[special_from_equipment_index(i)]);
      return buffer;
    } else if (equipment_is_cargo(i) &&
               i < cargo_equipment_index(template_cargo_count)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "%s",
               cargo[cargo_from_equipment_index(i)]);
      return buffer;
    }
  }
  return nullptr;
}

char *my_shortform(const char *source,
                   char buffer[static BTECH_TEXT_CAPACITY]) {
  const char *cursor;
  char *destination = buffer;
  char *end = buffer + BTECH_TEXT_CAPACITY - 1;

  if (!source)
    return nullptr;
  if (strlen(source) <= 4 && !strchr(source, '/'))
    snprintf(buffer, BTECH_TEXT_CAPACITY, "%s", source);
  else {
    for (cursor = source; *cursor && destination < end; cursor++)
      if (isdigit(*cursor) || isupper(*cursor) || *cursor == '_')
        *destination++ = *cursor;
    *destination = '\0';
    if (destination == buffer + 1 && source[1] && destination < end) {
      *destination++ = source[1];
      *destination = '\0';
    }
  }
  return buffer;
}

char *part_figure_out_shname(int i, char buffer[static BTECH_TEXT_CAPACITY]) {
  char name[BTECH_TEXT_CAPACITY] = {0};

  if (!i)
    return nullptr;
  if (equipment_is_weapon(i) && i < weapon_equipment_index(num_def_weapons)) {
    snprintf(name, sizeof(name), "%s",
             &MechWeapons[weapon_from_equipment_index(i)]
                  .name[part_weapon_short_name_offset(
                      weapon_from_equipment_index(i))]);
  } else if (equipment_is_ammunition(i) &&
             i < ammunition_equipment_index(num_def_weapons)) {
    snprintf(name, sizeof(name), "Ammo_%s",
             &MechWeapons[ammunition_to_weapon_index(i)]
                  .name[part_weapon_short_name_offset(
                      ammunition_to_weapon_index(i))]);
  } else if (equipment_is_bomb(i))
    snprintf(name, sizeof(name), "Bomb_%s",
             bomb_name(bomb_from_equipment_index(i)));
  else if (equipment_is_special(i) &&
           i < special_equipment_index(template_internal_count))
    snprintf(name, sizeof(name), "%s",
             internals[special_from_equipment_index(i)]);
  if (equipment_is_cargo(i) && i < cargo_equipment_index(template_cargo_count))
    snprintf(name, sizeof(name), "%s", cargo[cargo_from_equipment_index(i)]);
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
                ? build_bit_string_delimited2(crit_fire_modes, crit_ammo_modes,
                                              wFireModes, wAmmoModes,
                                              (char[BTECH_TEXT_CAPACITY]){0})
                : "-",
            !mech->xcode.context->configuration->btech_parts
                ? ""
                : tprintf("%d ", mech_critical_brand(mech, x, y)));
  else if (equipment_is_ammunition(((mech)->ud.sections)[x].criticals[y].type))
    fprintf(fp, "    %s		  { %s %d %s - }\n", crit,
            get_parts_vlong_name(mech->xcode.context,
                                 mech_critical_part_type(mech, x, y), 0),
            FullAmmo(mech, x, y),
            (((mech)->ud.sections)[x].criticals[y].firemode ||
             ((mech)->ud.sections)[x].criticals[y].ammomode)
                ? build_bit_string_delimited2(
                      crit_fire_modes, crit_ammo_modes,
                      clamp_unsigned_int_to_int(
                          ((mech)->ud.sections)[x].criticals[y].firemode),
                      clamp_unsigned_int_to_int(
                          ((mech)->ud.sections)[x].criticals[y].ammomode),
                      (char[BTECH_TEXT_CAPACITY]){0})
                : "-");
  else if (equipment_is_bomb(((mech)->ud.sections)[x].criticals[y].type))
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

void dump_locations(FILE *fp, Mech *mech, const char *const locdesc[]) {
  int x, y, l;
  char buf[512];
  char *ch;

  for (x = 0; locdesc[x]; x++) {
    if (!mech_section_original_internal(mech, x))
      continue;
    strcpy(buf, locdesc[x]);
    for (ch = buf; *ch; ch++)
      if (*ch == ' ')
        *ch = '_';
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
    y = ((mech)->ud.sections)[x].config;
    y &= ~CASE_TECH;
    if (y)
      fprintf(
          fp, "  Config           { %s }\n",
          build_bit_string(section_configs, y, (char[BTECH_TEXT_CAPACITY]){0}));
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
