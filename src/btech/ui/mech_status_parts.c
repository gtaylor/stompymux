#include "mech_status_internal.h"

void mech_critstatus(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  char *args[1];
  int index;

  cch(MECH_USUALSM);
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) == CLASS_MW, "Huh?");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "You must specify a section to list the criticals for!");
  index = ArmorSectionFromString(MechType(mech), MechMove(mech), args[0]);
  DOCHECK_CONTEXT(mech->xcode.context, index == -1, "Invalid section!");
  DOCHECK_CONTEXT(mech->xcode.context, !GetSectOInt(mech, index),
                  "Invalid section!");
  CriticalStatus(evaluation, player, mech, index);
}

typedef struct WeaponSpecsMenuContext WeaponSpecsMenuContext;
struct WeaponSpecsMenuContext {
  const ServerConfiguration *configuration;
  const BtechWeaponSettings *weapon_settings;
  int weapons[MAX_WEAPONS_PER_MECH];
  int weapon_count;
};

static PartDisplayName part_display_name(const char *source) {
  PartDisplayName name = {0};
  char *separator;

  if (!source)
    return name;

  name.valid = true;
  snprintf(name.text, sizeof(name.text), "%s", source);
  if (!strcmp(source, "LifeSupport"))
    snprintf(name.text, sizeof(name.text), "Life Support");
  else if (!strcmp(source, "TripleStrengthMyomer"))
    snprintf(name.text, sizeof(name.text), "Triple Strength Myomer");
  if ((separator = strstr(name.text, "Actuator")))
    if (separator != name.text)
      snprintf(separator, sizeof(name.text) - (size_t)(separator - name.text),
               " Actuator");
  while ((separator = strchr(name.text, '_')))
    *separator = ' ';
  while ((separator = strchr(name.text, '.')))
    *separator = ' ';
  return name;
}

PartDisplayName part_name(BtechContext *context, int type, int brand) {
  if (type == EMPTY)
    return part_display_name("Empty");
  return part_display_name(get_parts_long_name(context, type, brand));
}

PartDisplayName part_name_long(BtechContext *context, int type, int brand) {
  if (type == EMPTY)
    return part_display_name("Empty");
  return part_display_name(get_parts_vlong_name(context, type, brand));
}

PartDisplayName pos_part_name(Mech *mech, int index, int loop) {
  int t, b;
  int newloop, newindex;
  PartDisplayName name;

  if (index < 0 || index >= NUM_SECTIONS || loop < 0 || loop >= NUM_CRITICALS) {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("INVALID: For mech #%ld, %d/%d was requested.",
                               mech->mynum, index, loop));
    return part_display_name("--?LocationBug?--");
  }
  t = GetPartType(mech, index, loop);
  b = GetPartBrand(mech, index, loop);
  if (t == Special(HAND_OR_FOOT_ACTUATOR)) {
    if (index == LLEG || index == RLEG || MechIsQuad(mech))
      return part_display_name("Foot Actuator");
    return part_display_name("Hand Actuator");
  }
  if (t == Special(SHOULDER_OR_HIP)) {
    if (index == LLEG || index == RLEG || MechIsQuad(mech))
      return part_display_name("Hip");
    return part_display_name("Shoulder");
  }

  if (t == Special(HEAT_SINK)) {
    return part_display_name((MechSpecials(mech) & DOUBLE_HEAT_TECH ||
                              MechSpecials(mech) & CLAN_TECH)
                                 ? "Double Heatsink"
                                 : "Heatsink");
  }

  if (t == Special(SPLIT_CRIT_RIGHT) || t == Special(SPLIT_CRIT_LEFT)) {
    newindex = ReverseSplitCritLoc(mech, index, loop);
    newloop = GetPartData(mech, index, loop);
    if (newindex >= 0) {
      t = GetPartType(mech, newindex, newloop);
      b = GetPartBrand(mech, newindex, newloop);
    }
  }

  if (t == Special(ENGINE)) {
    return part_display_name(MechSpecials(mech) & LE_TECH   ? "Engine (Light)"
                             : MechSpecials(mech) & CE_TECH ? "Engine (Compact)"
                             : MechSpecials(mech) & XXL_TECH ? "Engine (XXL)"
                             : MechSpecials(mech) & XL_TECH  ? "Engine (XL)"
                                                             : "Engine");
  }

  if (t == Special(JUMP_JET)) {
    return part_display_name(MechSpecials2(mech) & IMPROVED_JJ_TECH
                                 ? "JumpJet (Improved)"
                                 : "Jumpjet");
  }

  if (t == Special(COCKPIT)) {
    return part_display_name(
        MechSpecials2(mech) & SMALLCOCKPIT_TECH ? "Small Cockpit" : "Cockpit");
  }

  name = part_name(mech->xcode.context, t, b);
  if (!name.valid)
    return part_display_name("--?ErrorInTemplate?--");
  return name;
}

static char *wspec_fun(void *data, int i, char buffer[static LBUF_SIZE]) {
  WeaponSpecsMenuContext *menu = data;
  int j;

  buffer[0] = '\0';
  if (!i)
    if (menu->configuration->btech_erange)
      snprintf(buffer, LBUF_SIZE, WSDUMP_MASKS_ER);
    else
      snprintf(buffer, LBUF_SIZE, WSDUMP_MASKS_NOER);
  else {
    i--;
    j = menu->weapons[i];
    if (menu->configuration->btech_erange)
      snprintf(buffer, LBUF_SIZE, WSDUMP_MASK_ER, MechWeapons[j].name,
               MechWeapons[j].heat, MechWeapons[j].damage, MechWeapons[j].min,
               MechWeapons[j].shortrange, MechWeapons[j].medrange, GunRange(j),
               EGunRange(menu->configuration, j),
               btech_weapon_settings_recycle_time(menu->weapon_settings, j));
    else
      snprintf(buffer, LBUF_SIZE, WSDUMP_MASK_NOER, MechWeapons[j].name,
               MechWeapons[j].heat, MechWeapons[j].damage, MechWeapons[j].min,
               MechWeapons[j].shortrange, MechWeapons[j].medrange, GunRange(j),
               btech_weapon_settings_recycle_time(menu->weapon_settings, j));
  }
  return buffer;
}

void mech_weaponspecs(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  int loop;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];

  /*   unsigned char weaps[8 * MAX_WEAPS_SECTION]; */
  int num_weaps;
  int index;
  int duplicate, ii;
  CoolMenu *c;
  WeaponSpecsMenuContext menu = {
      .configuration = mech->xcode.context->configuration,
      .weapon_settings = &mech->xcode.context->weapon_settings,
  };

  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    num_weaps = FindWeapons(mech, loop, weaparray, weapdata, critical);
    for (index = 0; index < num_weaps; index++) {
      duplicate = 0;
      for (ii = 0; ii < menu.weapon_count; ii++)
        if (weaparray[index] == menu.weapons[ii])
          duplicate = 1;
      if (!duplicate && menu.weapon_count < MAX_WEAPONS_PER_MECH)
        menu.weapons[menu.weapon_count++] = weaparray[index];
    }
  }
  DOCHECK_CONTEXT(mech->xcode.context, !menu.weapon_count,
                  "You have no weapons!");
  if (strcmp(MechType_Name(mech), MechType_Ref(mech)))
    c = SelCol_FunStringMenuContextK(1,
                                     tprintf("Weapons statistics for %s: %s",
                                             MechType_Name(mech),
                                             MechType_Ref(mech)),
                                     wspec_fun, &menu, menu.weapon_count + 1);
  else
    c = SelCol_FunStringMenuContextK(
        1, tprintf("Weapons statistics for %s", MechType_Ref(mech)), wspec_fun,
        &menu, menu.weapon_count + 1);
  ShowCoolMenu(evaluation, player, c);
  KillCoolMenu(c);
}

static char *status_text(char buffer[static MBUF_SIZE], const char *text) {
  snprintf(buffer, MBUF_SIZE, "%s", text);
  return buffer;
}

char *sectstatus_func(Mech *mech, char *arg, char buffer[static MBUF_SIZE]) {
  /* Show if Section is destroyed or not
   * -1 = Section Flooded
   * 1 = Section Exists
   * 0 = Section Destroyed
   */
  int index;

  if (!arg || !*arg)
    return status_text(buffer, "#-1 INVALID SECTION");

  index = ArmorSectionFromString(MechType(mech), MechMove(mech), arg);
  if (index == -1)
    return status_text(buffer, "#-1 INVALID SECTION");

  snprintf(buffer, MBUF_SIZE, "%d",
           SectIsFlooded(mech, index) ? -1 : !(SectIsDestroyed(mech, index)));

  return buffer;
}

char *critstatus_func(Mech *mech, char *arg, char buffer[static MBUF_SIZE]) {
  char *tmp;
  int index, i, max_crits;
  int type;

  if (!arg || !*arg)
    return status_text(buffer, "#-1 INVALID SECTION");

  index = ArmorSectionFromString(MechType(mech), MechMove(mech), arg);
  if (index == -1 || !GetSectOInt(mech, index))
    return status_text(buffer, "#-1 INVALID SECTION");

  buffer[0] = '\0';
  max_crits = CritsInLoc(mech, index);
  for (i = 0; i < max_crits; i++) {
    if (buffer[0])
      append_status(buffer, MBUF_SIZE, ",");
    append_status(buffer, MBUF_SIZE, "%d|", i + 1);
    type = GetPartType(mech, index, i);
    if (IsAmmo(type))
      type = FindAmmoType(mech, index, i);
    tmp = get_parts_long_name(mech->xcode.context, type,
                              GetPartBrand(mech, index, i));
    append_status(buffer, MBUF_SIZE, "|%s", tmp ? tmp : "Empty");
    append_status(buffer, MBUF_SIZE, "|%d",
                  (PartIsNonfunctional(mech, index, i) && type != EMPTY &&
                   (!IsCrap(type) || SectIsDestroyed(mech, index)))
                      ? -1
                      : PartTempNuke(mech, index, i));
    append_status(buffer, MBUF_SIZE, "|%d",
                  IsWeapon(type)                    ? 1
                  : IsAmmo(type)                    ? 2
                  : IsActuator(type)                ? 3
                  : IsCargo(type)                   ? 4
                  : (IsCrap(type) || type == EMPTY) ? 5
                                                    : 0);
  }
  return buffer;
}

char *armorstatus_func(Mech *mech, char *arg, char buffer[static MBUF_SIZE]) {
  char **locs;
  int index;
  int iter, curarm, curint, totarm, totint;

  if (!arg || !*arg)
    return status_text(buffer, "#-1 INVALID SECTION");

  if (strcmp(arg, "all") == 0) {
    locs = ProperSectionStringFromType(MechType(mech), MechMove(mech));
    curarm = totarm = curint = totint = 0;
    for (iter = 0; locs[iter]; iter++) {
      curarm += GetSectArmor(mech, iter) + GetSectRArmor(mech, iter);
      totarm += GetSectOArmor(mech, iter) + GetSectORArmor(mech, iter);
      curint += GetSectInt(mech, iter);
      totint += GetSectOInt(mech, iter);
    }
    buffer[0] = '\0';
    snprintf(buffer, MBUF_SIZE, "%d/%d|%d/%d", curarm, totarm, curint, totint);
    return buffer;
  }

  index = ArmorSectionFromString(MechType(mech), MechMove(mech), arg);
  if (index == -1 || !GetSectOInt(mech, index))
    return status_text(buffer, "#-1 INVALID SECTION");

  buffer[0] = '\0';
  snprintf(buffer, MBUF_SIZE, "%d/%d|%d/%d|%d/%d", GetSectArmor(mech, index),
           GetSectOArmor(mech, index), GetSectInt(mech, index),
           GetSectOInt(mech, index), GetSectRArmor(mech, index),
           GetSectORArmor(mech, index));
  return buffer;
}

/* weaponstatus_func. Returns a string containing:

   <weapon number> | <weapon (long) name> | <number of crits> |
        <part quality> | <weapon recycle time> | <recycle time left> |
        <weapon type> | <weapon status>
   [ , <next weapon> ]

   Weapon number is the number of the weapon in this particular 'mech.
   Long weapon name is 'agra.mediumlaser' and such.
   Weapon type is as defined in mech.h:
           #define TBEAM      0
           #define TMISSILE   1
           #define TARTILLERY 2
           #define TAMMO      3
           #define THAND      4
   Weapon status is:
        0 - weapon operational
        1 - weapon (temporarily) glitched
        2 - weapon destroyed/flooded
*/

char *weaponstatus_func(Mech *mech, char *arg, char buffer[static MBUF_SIZE]) {
  int count, sect, loopsect, i, type, totalcount = 0;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int criticals[MAX_WEAPS_SECTION];

  if (!arg)
    sect = -1;
  else if (!*arg)
    return status_text(buffer, "#-1 INVALID SECTION");
  else if ((sect = ArmorSectionFromString(MechType(mech), MechMove(mech),
                                          arg)) == -1 ||
           !GetSectOInt(mech, sect))
    return status_text(buffer, "#-1 INVALID SECTION");

  buffer[0] = '\0';
  for ((sect == -1) ? (loopsect = 0) : (loopsect = sect);
       (sect == -1) ? (loopsect < NUM_SECTIONS) : (loopsect < sect + 1);
       loopsect++) {
    count = FindWeapons(mech, loopsect, weaparray, weapdata, criticals);
    for (i = 0; i < count; i++, totalcount++) {
      if (buffer[0])
        append_status(buffer, MBUF_SIZE, ",");
      type = Weapon2I(GetPartType(mech, loopsect, criticals[i]));
      append_status(
          buffer, MBUF_SIZE, "%d|%s|%d|%d|%d|%d|%d|%d", totalcount,
          get_parts_long_name(mech->xcode.context, I2Weapon(type),
                              GetPartBrand(mech, loopsect, criticals[i])),
          GetWeaponCrits(mech, type),
          GetPartBrand(mech, loopsect, criticals[i]),
          btech_weapon_settings_recycle_time(
              &mech->xcode.context->weapon_settings, type),
          weapdata[i], MechWeapons[type].type,
          PartIsNonfunctional(mech, loopsect, criticals[i]) ? 2
          : PartTempNuke(mech, loopsect, criticals[i])      ? 1
                                                            : 0);
    }
  }
  return buffer;
}

char *critslot_func(Mech *mech, char *buf_section, char *buf_critnum,
                    char *buf_flag, char buffer[static MBUF_SIZE]) {
  int index, crit, flag, type;

  index = ArmorSectionFromString(MechType(mech), MechMove(mech), buf_section);
  if (index == -1)
    return status_text(buffer, "#-1 INVALID SECTION");
  if (!GetSectOInt(mech, index))
    return status_text(buffer, "#-1 INVALID SECTION");
  crit = atoi(buf_critnum);
  if (crit < 1 || crit > CritsInLoc(mech, index))
    return status_text(buffer, "#-1 INVALID CRITICAL");
  crit--;
  if (!buf_flag)
    flag = 0;
  else if (strcasecmp(buf_flag, "NAME") == 0)
    flag = 0;
  else if (strcasecmp(buf_flag, "STATUS") == 0)
    flag = 1;
  else if (strcasecmp(buf_flag, "DATA") == 0)
    flag = 2;
  else if (strcasecmp(buf_flag, "MAXAMMO") == 0)
    flag = 3;
  else if (strcasecmp(buf_flag, "AMMOTYPE") == 0)
    flag = 4;
  else if (strcasecmp(buf_flag, "MODE") == 0)
    flag = 5;
  else if (strcasecmp(buf_flag, "HALFTON") == 0)
    flag = 6;
  else
    flag = 0;

  type = GetPartType(mech, index, crit);

  if (flag == 1) {
    if (PartIsDisabled(mech, index, crit))
      return status_text(buffer, "Disabled");
    if (PartIsDestroyed(mech, index, crit))
      return status_text(buffer, "Destroyed");
    return status_text(buffer, "Operational");
  } else if (flag == 2) {
    snprintf(buffer, MBUF_SIZE, "%d", GetPartData(mech, index, crit));
    return buffer;
  } else if (flag == 3) {
    if (!IsAmmo(type))
      return status_text(buffer, "#-1 NOT AMMO");
    snprintf(buffer, MBUF_SIZE, "%d", FullAmmo(mech, index, crit));
    return buffer;
  } else if (flag == 4) {
    if (!IsAmmo(type))
      return status_text(buffer, "#-1 NOT AMMO");
    type = FindAmmoType(mech, index, crit);
  } else if (flag == 5) {
    int weapindex;
    if (!IsWeapon(type))
      return status_text(buffer, "#-1 NOT AMMO OR WEAPON");
    else {
      weapindex = Weapon2I(type);
      snprintf(buffer, MBUF_SIZE, "%c%c",
               GetWeaponFireModeLetter_Model_Mode(
                   weapindex, GetPartFireMode(mech, index, crit)),
               GetWeaponAmmoModeLetter_Model_Mode(
                   weapindex, GetPartAmmoMode(mech, index, crit)));
      return buffer;
    }
  } else if (flag == 6) {
    if (!IsAmmo(type))
      return status_text(buffer, "#-1 NOT AMMO");
    snprintf(buffer, MBUF_SIZE, "%d",
             GetPartFireMode(mech, index, crit) & HALFTON_MODE ? 1 : 0);
    return buffer;
  }

  if (type == EMPTY || IsCrap(type))
    return status_text(buffer, "Empty");
  if (flag == 0) {
    type = mech_parts_alias(mech, index, type);
  }
  snprintf(buffer, MBUF_SIZE, "%s",
           get_parts_vlong_name(mech->xcode.context, type,
                                GetPartBrand(mech, index, crit)));
  return buffer;
}

void CriticalStatus(EvaluationContext *evaluation, DbRef player, Mech *mech,
                    int index) {
  int loop, i;
  char buffer[LBUF_SIZE] = {0};
  int type, data, wFireMode;
  int max_crits = CritsInLoc(mech, index);
  char **foo;
  int count = 0;
  CoolMenu *cm;

  Create(foo, char *, NUM_CRITICALS + 1);

  for (i = 0; i < max_crits; i++) {
    loop = ((i % 2) ? (max_crits / 2) : 0) + i / 2;
    snprintf(buffer, sizeof(buffer), "%2d ", loop + 1);
    type = GetPartType(mech, index, loop);
    data = GetPartData(mech, index, loop);
    wFireMode = GetPartFireMode(mech, index, loop);
    if (IsAmmo(type)) {
      char trash[50];

      strcat(buffer, &MechWeapons[Ammo2WeaponI(type)].name[3]);
      strcat(buffer,
             GetAmmoDesc_Model_Mode(Ammo2WeaponI(type),
                                    GetPartAmmoMode(mech, index, loop)));
      strcat(buffer, " Ammo");
      if (!PartIsNonfunctional(mech, index, loop)) {
        snprintf(trash, sizeof(trash), " [%3.3d/%3.3d]", data,
                 FullAmmo(mech, index, loop));
        strcat(buffer, trash);
      }

    } else {
      if (IsWeapon(type) && (wFireMode & OS_MODE))
        strcat(buffer, "OS ");
      strcat(buffer, pos_part_name(mech, index, loop).text);
      if (IsWeapon(type) && (((wFireMode & OS_MODE) && (wFireMode & OS_USED)) ||
                             (wFireMode & ROCKET_FIRED)))
        strcat(buffer, " (Empty)");
      if (wFireMode & WILL_JETTISON_MODE)
        strcat(buffer, " (backpack)");

      if (IsWeapon(type) && (wFireMode & REAR_MOUNT))
        strcat(buffer, " (R)");
      if (!PartIsNonfunctional(mech, index, loop)) {
        if (Special2I(type) == ARTEMIS_IV) {
          char trash[50];
          if (data) {
            snprintf(trash, sizeof(trash), " [Controls Slot %d]", data);
            strcat(buffer, trash);
          }
        }
      }
    }

    if (PartIsBroken(mech, index, loop) && type != EMPTY &&
        (!IsCrap(type) || SectIsDestroyed(mech, index)))
      strcat(buffer,
             PartIsDestroyed(mech, index, loop) ? " (Destroyed)" : " (Broken)");
    else if (PartIsDisabled(mech, index, loop) && type != EMPTY)
      strcat(buffer, " (Disabled)");
    else if (PartIsDamaged(mech, index, loop) && type != EMPTY)
      strcat(buffer, " (Damaged)");

    foo[count++] = strdup(buffer);
  }

  ArmorStringFromIndex(index, buffer, MechType(mech), MechMove(mech));
  strcat(buffer, " Criticals");
  cm = SelCol_StringMenu(2, buffer, foo);
  ShowCoolMenu(evaluation, player, cm);
  KillCoolMenu(cm);
  KillText(foo);
}

char *evaluate_ammo_amount(int now, int max) {
  int f = (now * 100) / max;

  if (f >= 50)
    return "[fg=green bold]";
  if (f >= 25)
    return "[fg=yellow bold]";
  return "[fg=red bold]";
}
