#include "template_internal.h"

int compare_array(char *list[], char *command) {
  int x;

  if (!list)
    return -1;
  for (x = 0; list[x]; x++)
    if (!strcasecmp(list[x], command))
      return x;

  return -1;
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

char *build_bit_string(char *bitdescs[], int data, char *buffer) {
  int bv;
  int x;

  buffer[0] = 0;
  for (x = 0; bitdescs[x]; x++) {
    bv = 1U << x;
    if (data & bv) {
      strcat(buffer, bitdescs[x]);
      strcat(buffer, " ");
    }
  }
  if ((x = strlen(buffer)) > 0 && buffer[x - 1] == ' ')
    buffer[x - 1] = '\0';
  return buffer;
}

char *build_bit_string2(char *bitdescs[], char *bitdescs2[], int data,
                        int data2, char *buffer) {
  int bv;
  int x;

  buffer[0] = 0;

  for (x = 0; bitdescs[x]; x++) {
    bv = 1U << x;
    if (data & bv) {
      strcat(buffer, bitdescs[x]);
      strcat(buffer, " ");
    }
  }

  for (x = 0; bitdescs2[x]; x++) {
    bv = 1U << x;
    if (data2 & bv) {
      strcat(buffer, bitdescs2[x]);
      strcat(buffer, " ");
    }
  }

  if ((x = strlen(buffer)) > 0 && buffer[x - 1] == ' ') {
    buffer[x - 1] = '\0';
  }

  return buffer;
}

char *build_bit_string_delimited2(char *bitdescs[], char *bitdescs2[], int data,
                                  int data2, char *buffer) {
  int bv;
  int x;

  buffer[0] = 0;

  for (x = 0; bitdescs[x]; x++) {
    bv = 1U << x;
    if (data & bv) {
      strcat(buffer, bitdescs[x]);
      strcat(buffer, "|");
    }
  }

  for (x = 0; bitdescs2[x]; x++) {
    bv = 1U << x;
    if (data2 & bv) {
      strcat(buffer, bitdescs2[x]);
      strcat(buffer, "|");
    }
  }

  if ((x = strlen(buffer)) > 0 && buffer[x - 1] == '|') {
    buffer[x - 1] = '\0';
  }

  return buffer;
}

char *build_bit_string3(char *bitdescs[], char *bitdescs2[], char *bitdescs3[],
                        int data, int data2, int data3, char *buffer) {
  int bv;
  int x;

  buffer[0] = 0;

  for (x = 0; bitdescs[x]; x++) {
    bv = 1U << x;
    if (data & bv) {
      strcat(buffer, bitdescs[x]);
      strcat(buffer, " ");
    }
  }

  for (x = 0; bitdescs2[x]; x++) {
    bv = 1U << x;
    if (data2 & bv) {
      strcat(buffer, bitdescs2[x]);
      strcat(buffer, " ");
    }
  }

  for (x = 0; bitdescs3[x]; x++) {
    bv = 1U << x;
    if (data3 & bv) {
      strcat(buffer, bitdescs3[x]);
      strcat(buffer, " ");
    }
  }

  if ((x = strlen(buffer)) > 0 && buffer[x - 1] == ' ')
    buffer[x - 1] = '\0';

  return buffer;
}

#define QDM(a)                                                                 \
  case I2Special(a):                                                           \
    return 1
static int InvalidVehicleItem(Mech *mech, int x, int y) {
  int t;

  t = GetPartType(mech, x, y);
  switch (t) {
    QDM(SHOULDER_OR_HIP);
    QDM(UPPER_ACTUATOR);
    QDM(LOWER_ACTUATOR);
    QDM(HAND_OR_FOOT_ACTUATOR);
    QDM(ENGINE);
    QDM(GYRO);
    QDM(JUMP_JET);
  }
  return 0;
}

#ifndef CLAN_SUPPORT
#define CLCH(a)                                                                \
  do {                                                                         \
    if (MechWeapons[a].special & (CLAT))                                       \
      return NULL;                                                             \
  } while (0)
#else
#define CLCH(a)                                                                \
  do {                                                                         \
    if (brand) {                                                               \
      if ((!configuration->btech_parts) || (MechWeapons[a].special & CLAT))    \
        return NULL;                                                           \
      else if (MechWeapons[a].special & CLAT)                                  \
        isclan = 1;                                                            \
    }                                                                          \
  } while (0)
#endif

static char *part_figure_out_name_sub(const ServerConfiguration *configuration,
                                      int i, int j, int brand,
                                      char buffer[static BTECH_TEXT_CAPACITY]) {
  int isclan = 0;
  const char *source;

  if (!i)
    return nullptr;
  if (IsWeapon(i) && i < I2Weapon(num_def_weapons)) {
    CLCH(Weapon2I(i));
    source = &MechWeapons[Weapon2I(i)].name[(j && !isclan) ? 3 : 0];
    snprintf(buffer, BTECH_TEXT_CAPACITY, "%s", source);
    return buffer;
  } else if (IsAmmo(i) && i < I2Ammo(num_def_weapons)) {
    CLCH(Ammo2WeaponI(i));
    if (MechWeapons[Ammo2WeaponI(i)].type != TBEAM &&
        MechWeapons[Ammo2WeaponI(i)].type != THAND &&
        !(MechWeapons[Ammo2WeaponI(i)].special & PCOMBAT)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "Ammo_%s",
               &MechWeapons[Ammo2WeaponI(i)].name[(j && !isclan) ? 3 : 0]);
      return buffer;
    }
  } else if (!brand) {
    if (IsBomb(i)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "Bomb_%s", bomb_name(Bomb2I(i)));
      return buffer;
    } else if (IsSpecial(i) && i < I2Special(template_internal_count)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "%s", internals[Special2I(i)]);
      return buffer;
    } else if (IsCargo(i) && i < I2Cargo(template_cargo_count)) {
      snprintf(buffer, BTECH_TEXT_CAPACITY, "%s", cargo[Cargo2I(i)]);
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

#undef CLCH
#ifdef CLAN_SUPPORT
#define CLCH(a) ((!strncasecmp(MechWeapons[a].name, "CL.", 2)) ? 0 : 3)
#else
#define CLCH(a) 3
#endif

char *part_figure_out_shname(int i, char buffer[static BTECH_TEXT_CAPACITY]) {
  char name[BTECH_TEXT_CAPACITY] = {0};

  if (!i)
    return nullptr;
  if (IsWeapon(i) && i < I2Weapon(num_def_weapons)) {
    snprintf(name, sizeof(name), "%s",
             &MechWeapons[Weapon2I(i)].name[CLCH(Weapon2I(i))]);
  } else if (IsAmmo(i) && i < I2Ammo(num_def_weapons)) {
    snprintf(name, sizeof(name), "Ammo_%s",
             &MechWeapons[Ammo2WeaponI(i)].name[CLCH(Ammo2WeaponI(i))]);
  } else if (IsBomb(i))
    snprintf(name, sizeof(name), "Bomb_%s", bomb_name(Bomb2I(i)));
  else if (IsSpecial(i) && i < I2Special(template_internal_count))
    snprintf(name, sizeof(name), "%s", internals[Special2I(i)]);
  if (IsCargo(i) && i < I2Cargo(template_cargo_count))
    snprintf(name, sizeof(name), "%s", cargo[Cargo2I(i)]);
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

  if (!GetPartType(mech, x, y))
    return 1;
  if (MechType(mech) != CLASS_MECH && InvalidVehicleItem(mech, x, y))
    return 1;
  for (y1 = y + 1; y1 < 12; y1++) {
    if (GetPartType(mech, x, y1) != GetPartType(mech, x, y))
      break;
    if (GetPartData(mech, x, y1) != GetPartData(mech, x, y))
      break;
    if (GetPartFireMode(mech, x, y1) != GetPartFireMode(mech, x, y))
      break;
    if (GetPartAmmoMode(mech, x, y1) != GetPartAmmoMode(mech, x, y))
      break;
    if (mech->xcode.context->configuration->btech_parts)
      if (GetPartBrand(mech, x, y1) != GetPartBrand(mech, x, y))
        break;
  }
  y1--;
  if (IsWeapon(GetPartType(mech, x, y))) {
    /* Nonbeams, or flamers don't have TC */
    if (!TCAble(GetPartType(mech, x, y)))
      flaggo = ON_TC;
    if (((y1 - y) + 1) >
        (z = GetWeaponCrits(mech, Weapon2I(GetPartType(mech, x, y)))))
      y1 = y + z - 1;
  }
  if (y != y1)
    snprintf(crit, 32, "CRIT_%d-%d", y + 1, y1 + 1);
  else
    snprintf(crit, 32, "CRIT_%d", y + 1);

  wFireModes = GetPartFireMode(mech, x, y);
  wFireModes &= ~flaggo;
  wAmmoModes = GetPartAmmoMode(mech, x, y);

  if (IsWeapon(GetPartType(mech, x, y)))
    fprintf(
        fp, "    %s		  { %s - %s %s}\n", crit,
        get_parts_vlong_name(mech->xcode.context, GetPartType(mech, x, y), 0),
        (wFireModes || wAmmoModes)
            ? build_bit_string_delimited2(crit_fire_modes, crit_ammo_modes,
                                          wFireModes, wAmmoModes,
                                          (char[BTECH_TEXT_CAPACITY]){0})
            : "-",
        !mech->xcode.context->configuration->btech_parts
            ? ""
            : tprintf("%d ", GetPartBrand(mech, x, y)));
  else if (IsAmmo(MechSections(mech)[x].criticals[y].type))
    fprintf(
        fp, "    %s		  { %s %d %s - }\n", crit,
        get_parts_vlong_name(mech->xcode.context, GetPartType(mech, x, y), 0),
        FullAmmo(mech, x, y),
        (MechSections(mech)[x].criticals[y].firemode ||
         MechSections(mech)[x].criticals[y].ammomode)
            ? build_bit_string_delimited2(
                  crit_fire_modes, crit_ammo_modes,
                  MechSections(mech)[x].criticals[y].firemode,
                  MechSections(mech)[x].criticals[y].ammomode,
                  (char[BTECH_TEXT_CAPACITY]){0})
            : "-");
  else if (IsBomb(MechSections(mech)[x].criticals[y].type))
    fprintf(
        fp, "    %s		  { %s - - - }\n", crit,
        get_parts_vlong_name(mech->xcode.context, GetPartType(mech, x, y), 0));
  else {
    fprintf(
        fp, "    %s		  { %s %s - %s}\n", crit,
        get_parts_vlong_name(mech->xcode.context, GetPartType(mech, x, y), 0),
        GetPartData(mech, x, y) ? tprintf("%d", GetPartData(mech, x, y)) : "-",
        !mech->xcode.context->configuration->btech_parts
            ? ""
            : tprintf("%d ", GetPartBrand(mech, x, y)));
  }
  return (y1 - y + 1);
}

void dump_locations(FILE *fp, Mech *mech, const char *locdesc[]) {
  int x, y, l;
  char buf[512];
  char *ch;

  for (x = 0; locdesc[x]; x++) {
    if (!GetSectOInt(mech, x))
      continue;
    strcpy(buf, locdesc[x]);
    for (ch = buf; *ch; ch++)
      if (*ch == ' ')
        *ch = '_';
    fprintf(fp, "%s\n", buf);
    if (GetSectOArmor(mech, x))
      fprintf(fp, "  Armor            { %d }\n", GetSectOArmor(mech, x));
    if (GetSectOInt(mech, x))
      fprintf(fp, "  Internals        { %d }\n", GetSectOInt(mech, x));
    if (GetSectORArmor(mech, x))
      fprintf(fp, "  Rear             { %d }\n", GetSectORArmor(mech, x));
    y = MechSections(mech)[x].config;
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
  switch (MechComputer(mech)) {
  case 1:
    return 0.8;
  case 2:
    return 1;
  case 3:
    return 1.25;
  case 4:
    return 1.5;
  case 5:
    return 1.75;
  }
  return 0;
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
  switch (MechRadio(mech)) {
  case 1:
    return 0.8;
  case 2:
    return 1;
  case 3:
    return 1.25;
  case 4:
    return 1.5;
  case 5:
    return 1.75;
  }
  return 0.0;
}

void computer_conversion(Mech *mech) {
  int l = 0;

  switch (MechScanRange(mech)) {
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
    MechComputer(mech) = l;
    MechScanRange(mech) = MechComputersScanRange(mech);
    MechTacRange(mech) = MechComputersTacRange(mech);
    MechLRSRange(mech) = MechComputersLRSRange(mech);
    MechRadioRange(mech) = MechComputersRadioRange(mech);
  }
}
