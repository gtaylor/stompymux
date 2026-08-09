#include "mech_api_types.h"
#include "mech_status_api.h"
#include "mech_status_render_internal.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "weapon_catalogue_api.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_text_builder.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "equipment_types.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_parts.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tech_do_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_settings.h"

void mech_critstatus(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);
  EvaluationContext *evaluation = btech_context_evaluation(context);
  char *args[1];
  int index;

  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  if (mech_class(mech) == CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player, "Huh?");
    return;
  }
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You must specify a section to list the criticals for!");
    return;
  }
  index = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                 args[0]);
  if (index == -1) {
    mecha_notify(btech_context_evaluation(context), player, "Invalid section!");
    return;
  }
  if (!mech_section_original_internal(mech, index)) {
    mecha_notify(btech_context_evaluation(context), player, "Invalid section!");
    return;
  }
  CriticalStatus(evaluation, player, mech, index);
}

typedef struct WeaponSpecsMenuContext WeaponSpecsMenuContext;
struct WeaponSpecsMenuContext {
  const ServerConfiguration *configuration;
  const BtechWeaponSettings *weapon_settings;
  int weapons[MAX_WEAPONS_PER_MECH];
  int weapon_count;
};

static int weapon_menu_get(const WeaponSpecsMenuContext *menu, int index) {
  if (index < 0)
    abort();
  return *(const int *)checked_storage_at_const(
      menu->weapons, MAX_WEAPONS_PER_MECH, sizeof(*menu->weapons),
      (size_t)index);
}

static void weapon_menu_set(WeaponSpecsMenuContext *menu, int index,
                            int weapon) {
  if (index < 0)
    abort();
  int *slot = checked_storage_at(menu->weapons, MAX_WEAPONS_PER_MECH,
                                 sizeof(*menu->weapons), (size_t)index);
  *slot = weapon;
}

static unsigned char weapon_array_get(const unsigned char *weapons, int index) {
  if (index < 0)
    abort();
  return *(const unsigned char *)checked_storage_at_const(
      weapons, MAX_WEAPS_SECTION, sizeof(*weapons), (size_t)index);
}

static int critical_array_get(const int *criticals, int index) {
  if (index < 0)
    abort();
  return *(const int *)checked_storage_at_const(
      criticals, MAX_WEAPS_SECTION, sizeof(*criticals), (size_t)index);
}

static PartDisplayName part_display_name(const char *source) {
  PartDisplayName name = {0};
  char *separator;

  if (!source)
    return name;

  name.valid = true;
  (void)snprintf(name.text, sizeof(name.text), "%s", source);
  if (!strcmp(source, "LifeSupport"))
    (void)snprintf(name.text, sizeof(name.text), "Life Support");
  else if (!strcmp(source, "TripleStrengthMyomer"))
    (void)snprintf(name.text, sizeof(name.text), "Triple Strength Myomer");
  if ((separator = strstr(name.text, "Actuator")))
    if (separator != name.text)
      (void)snprintf(separator,
                     sizeof(name.text) - (size_t)(separator - name.text),
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
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("INVALID: For mech #%ld, %d/%d was requested.",
                               mech_dbref(mech), index, loop));
    return part_display_name("--?LocationBug?--");
  }
  t = mech_critical_part_type(mech, index, loop);
  b = mech_critical_brand(mech, index, loop);
  if (t == special_equipment_index(HAND_OR_FOOT_ACTUATOR)) {
    if (index == LLEG || index == RLEG || mech_movement_type(mech) == MOVE_QUAD)
      return part_display_name("Foot Actuator");
    return part_display_name("Hand Actuator");
  }
  if (t == special_equipment_index(SHOULDER_OR_HIP)) {
    if (index == LLEG || index == RLEG || mech_movement_type(mech) == MOVE_QUAD)
      return part_display_name("Hip");
    return part_display_name("Shoulder");
  }

  if (t == special_equipment_index(HEAT_SINK)) {
    return part_display_name((mech_technology_flags(mech) & DOUBLE_HEAT_TECH ||
                              mech_technology_flags(mech) & CLAN_TECH)
                                 ? "Double Heatsink"
                                 : "Heatsink");
  }

  if (t == special_equipment_index(SPLIT_CRIT_RIGHT) ||
      t == special_equipment_index(SPLIT_CRIT_LEFT)) {
    newindex = ReverseSplitCritLoc(mech, index, loop);
    newloop = mech_critical_data(mech, index, loop);
    if (newindex >= 0) {
      t = mech_critical_part_type(mech, newindex, newloop);
      b = mech_critical_brand(mech, newindex, newloop);
    }
  }

  if (t == special_equipment_index(ENGINE)) {
    int technology = mech_technology_flags(mech);
    return part_display_name(technology & LE_TECH    ? "Engine (Light)"
                             : technology & CE_TECH  ? "Engine (Compact)"
                             : technology & XXL_TECH ? "Engine (XXL)"
                             : technology & XL_TECH  ? "Engine (XL)"
                                                     : "Engine");
  }

  if (t == special_equipment_index(JUMP_JET)) {
    return part_display_name(mech_technology_flags_secondary(mech) &
                                     IMPROVED_JJ_TECH
                                 ? "JumpJet (Improved)"
                                 : "Jumpjet");
  }

  if (t == special_equipment_index(COCKPIT)) {
    return part_display_name(mech_technology_flags_secondary(mech) &
                                     SMALLCOCKPIT_TECH
                                 ? "Small Cockpit"
                                 : "Cockpit");
  }

  name = part_name(mech_context(mech), t, b);
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
      (void)snprintf(buffer, LBUF_SIZE, WSDUMP_MASKS_ER);
    else
      (void)snprintf(buffer, LBUF_SIZE, WSDUMP_MASKS_NOER);
  else {
    i--;
    j = weapon_menu_get(menu, i);
    const WeaponRangeProfile ranges = weapon_catalogue_ranges(j);
    if (menu->configuration->btech_erange)
      (void)snprintf(
          buffer, LBUF_SIZE, WSDUMP_MASK_ER, weapon_catalogue_name(j),
          weapon_catalogue_heat(j), weapon_catalogue_damage(j), ranges.minimum,
          ranges.short_range, ranges.medium_range,
          weapon_catalogue_effective_range(j, false),
          weapon_catalogue_effective_range(j,
                                           menu->configuration->btech_erange),
          btech_weapon_settings_recycle_time(menu->weapon_settings, j));
    else
      (void)snprintf(
          buffer, LBUF_SIZE, WSDUMP_MASK_NOER, weapon_catalogue_name(j),
          weapon_catalogue_heat(j), weapon_catalogue_damage(j), ranges.minimum,
          ranges.short_range, ranges.medium_range,
          weapon_catalogue_effective_range(j, false),
          btech_weapon_settings_recycle_time(menu->weapon_settings, j));
  }
  return buffer;
}

void mech_weaponspecs(DbRef player, void *data, const char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);
  EvaluationContext *evaluation = btech_context_evaluation(context);
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
      .configuration = context->configuration,
      .weapon_settings = &context->weapon_settings,
  };

  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    num_weaps =
        FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 0);
    for (index = 0; index < num_weaps; index++) {
      duplicate = 0;
      const int weapon = weapon_array_get(weaparray, index);
      for (ii = 0; ii < menu.weapon_count; ii++)
        if (weapon == weapon_menu_get(&menu, ii))
          duplicate = 1;
      if (!duplicate && menu.weapon_count < MAX_WEAPONS_PER_MECH)
        weapon_menu_set(&menu, menu.weapon_count++, weapon);
    }
  }
  if (!menu.weapon_count) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You have no weapons!");
    return;
  }
  if (strcmp(mech_model_name(mech), mech_model_reference(mech)))
    c = SelCol_FunStringMenuContextK(1,
                                     tprintf("Weapons statistics for %s: %s",
                                             mech_model_name(mech),
                                             mech_model_reference(mech)),
                                     wspec_fun, &menu, menu.weapon_count + 1);
  else
    c = SelCol_FunStringMenuContextK(
        1, tprintf("Weapons statistics for %s", mech_model_reference(mech)),
        wspec_fun, &menu, menu.weapon_count + 1);
  ShowCoolMenu(evaluation, player, c);
  KillCoolMenu(c);
}

static char *status_text(char buffer[static MBUF_SIZE], const char *text) {
  (void)snprintf(buffer, MBUF_SIZE, "%s", text);
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

  index =
      ArmorSectionFromString(mech_class(mech), mech_movement_type(mech), arg);
  if (index == -1)
    return status_text(buffer, "#-1 INVALID SECTION");

  (void)snprintf(buffer, MBUF_SIZE, "%d",
                 mech_section_is_flooded(mech, index)
                     ? -1
                     : !mech_section_is_destroyed(mech, index));

  return buffer;
}

char *critstatus_func(Mech *mech, char *arg, char buffer[static MBUF_SIZE]) {
  const char *tmp;
  int index, i, max_crits;
  int type;

  if (!arg || !*arg)
    return status_text(buffer, "#-1 INVALID SECTION");

  index =
      ArmorSectionFromString(mech_class(mech), mech_movement_type(mech), arg);
  if (index == -1 || !mech_section_original_internal(mech, index))
    return status_text(buffer, "#-1 INVALID SECTION");

  buffer[0] = '\0';
  max_crits = CritsInLoc(mech, index);
  for (i = 0; i < max_crits; i++) {
    if (buffer[0])
      append_status(buffer, MBUF_SIZE, ",");
    append_status(buffer, MBUF_SIZE, "%d|", i + 1);
    type = mech_critical_part_type(mech, index, i);
    if (equipment_is_ammunition(type))
      type = FindAmmoType(mech, index, i);
    tmp = get_parts_long_name(mech_context(mech), type,
                              mech_critical_brand(mech, index, i));
    append_status(buffer, MBUF_SIZE, "|%s", tmp ? tmp : "Empty");
    append_status(buffer, MBUF_SIZE, "|%d",
                  (mech_critical_is_nonfunctional(mech, index, i) &&
                   type != EMPTY &&
                   (!mech_part_is_structural_placeholder(type) ||
                    mech_section_is_destroyed(mech, index)))
                      ? -1
                      : mech_critical_temporary_failure(mech, index, i));
    append_status(buffer, MBUF_SIZE, "|%d",
                  equipment_is_weapon(type)       ? 1
                  : equipment_is_ammunition(type) ? 2
                  : equipment_is_actuator(type)   ? 3
                  : equipment_is_cargo(type)      ? 4
                  : (mech_part_is_structural_placeholder(type) || type == EMPTY)
                      ? 5
                      : 0);
  }
  return buffer;
}

char *armorstatus_func(Mech *mech, char *arg, char buffer[static MBUF_SIZE]) {
  const char *const *locs;
  int index;
  int iter, curarm, curint, totarm, totint;

  if (!arg || !*arg)
    return status_text(buffer, "#-1 INVALID SECTION");

  if (strcmp(arg, "all") == 0) {
    locs =
        ProperSectionStringFromType(mech_class(mech), mech_movement_type(mech));
    curarm = totarm = curint = totint = 0;
    for (iter = 0; iter < NUM_SECTIONS; iter++) {
      const char *location = *(const char *const *)checked_storage_at_const(
          locs, NUM_SECTIONS + 1, sizeof(*locs), (size_t)iter);
      if (location == nullptr)
        break;
      curarm +=
          mech_section_armor(mech, iter) + mech_section_rear_armor(mech, iter);
      totarm += mech_section_original_armor(mech, iter) +
                mech_section_original_rear_armor(mech, iter);
      curint += mech_section_internal(mech, iter);
      totint += mech_section_original_internal(mech, iter);
    }
    buffer[0] = '\0';
    (void)snprintf(buffer, MBUF_SIZE, "%d/%d|%d/%d", curarm, totarm, curint,
                   totint);
    return buffer;
  }

  index =
      ArmorSectionFromString(mech_class(mech), mech_movement_type(mech), arg);
  if (index == -1 || !mech_section_original_internal(mech, index))
    return status_text(buffer, "#-1 INVALID SECTION");

  buffer[0] = '\0';
  (void)snprintf(buffer, MBUF_SIZE, "%d/%d|%d/%d|%d/%d",
                 mech_section_armor(mech, index),
                 mech_section_original_armor(mech, index),
                 mech_section_internal(mech, index),
                 mech_section_original_internal(mech, index),
                 mech_section_rear_armor(mech, index),
                 mech_section_original_rear_armor(mech, index));
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
  else if ((sect = ArmorSectionFromString(
                mech_class(mech), mech_movement_type(mech), arg)) == -1 ||
           !mech_section_original_internal(mech, sect))
    return status_text(buffer, "#-1 INVALID SECTION");

  buffer[0] = '\0';
  for ((sect == -1) ? (loopsect = 0) : (loopsect = sect);
       (sect == -1) ? (loopsect < NUM_SECTIONS) : (loopsect < sect + 1);
       loopsect++) {
    count =
        FindWeapons_Advanced(mech, loopsect, weaparray, weapdata, criticals, 0);
    for (i = 0; i < count; i++, totalcount++) {
      const int critical = critical_array_get(criticals, i);
      if (buffer[0])
        append_status(buffer, MBUF_SIZE, ",");
      type = weapon_from_equipment_index(
          mech_critical_part_type(mech, loopsect, critical));
      append_status(
          buffer, MBUF_SIZE, "%d|%s|%d|%d|%d|%d|%d|%d", totalcount,
          get_parts_long_name(mech_context(mech), weapon_equipment_index(type),
                              mech_critical_brand(mech, loopsect, critical)),
          GetWeaponCrits(mech, type),
          mech_critical_brand(mech, loopsect, critical),
          btech_weapon_settings_recycle_time(
              &mech_context(mech)->weapon_settings, type),
          weapon_array_get(weapdata, i), weapon_catalogue_type(type),
          mech_critical_is_nonfunctional(mech, loopsect, critical)    ? 2
          : mech_critical_temporary_failure(mech, loopsect, critical) ? 1
                                                                      : 0);
    }
  }
  return buffer;
}

char *critslot_func(Mech *mech, char *buf_section, char *buf_critnum,
                    char *buf_flag, char buffer[static MBUF_SIZE]) {
  int index, crit, flag, type;

  index = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                 buf_section);
  if (index == -1)
    return status_text(buffer, "#-1 INVALID SECTION");
  if (!mech_section_original_internal(mech, index))
    return status_text(buffer, "#-1 INVALID SECTION");
  if (!parse_int_checked(buf_critnum, &crit) || crit < 1 ||
      crit > CritsInLoc(mech, index))
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

  type = mech_critical_part_type(mech, index, crit);

  if (flag == 1) {
    if (mech_critical_is_disabled(mech, index, crit))
      return status_text(buffer, "Disabled");
    if (mech_critical_is_destroyed(mech, index, crit))
      return status_text(buffer, "Destroyed");
    return status_text(buffer, "Operational");
  } else if (flag == 2) {
    (void)snprintf(buffer, MBUF_SIZE, "%d",
                   mech_critical_data(mech, index, crit));
    return buffer;
  } else if (flag == 3) {
    if (!equipment_is_ammunition(type))
      return status_text(buffer, "#-1 NOT AMMO");
    (void)snprintf(buffer, MBUF_SIZE, "%d", FullAmmo(mech, index, crit));
    return buffer;
  } else if (flag == 4) {
    if (!equipment_is_ammunition(type))
      return status_text(buffer, "#-1 NOT AMMO");
    type = FindAmmoType(mech, index, crit);
  } else if (flag == 5) {
    int weapindex;
    if (!equipment_is_weapon(type))
      return status_text(buffer, "#-1 NOT AMMO OR WEAPON");
    else {
      const int ammo_mode = mech_critical_ammo_mode(mech, index, crit);
      weapindex = weapon_from_equipment_index(type);
      (void)snprintf(buffer, MBUF_SIZE, "%c%c",
                     GetWeaponFireModeLetter_Model_Mode(
                         weapindex, mech_critical_fire_mode(mech, index, crit)),
                     ammo_mode < 0 ? ' '
                                   : GetWeaponAmmoModeLetter_Model_Mode(
                                         weapindex, (unsigned int)ammo_mode));
      return buffer;
    }
  } else if (flag == 6) {
    if (!equipment_is_ammunition(type))
      return status_text(buffer, "#-1 NOT AMMO");
    (void)snprintf(
        buffer, MBUF_SIZE, "%d",
        mech_critical_fire_mode(mech, index, crit) & HALFTON_MODE ? 1 : 0);
    return buffer;
  }

  if (type == EMPTY || mech_part_is_structural_placeholder(type))
    return status_text(buffer, "Empty");
  if (flag == 0) {
    type = mech_parts_alias(mech, index, type);
  }
  (void)snprintf(buffer, MBUF_SIZE, "%s",
                 get_parts_vlong_name(mech_context(mech), type,
                                      mech_critical_brand(mech, index, crit)));
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
    BtechTextBuilder line;
    btech_text_builder_initialize(&line, buffer, sizeof(buffer));
    btech_text_builder_append_format(&line, "%2d ", loop + 1);
    type = mech_critical_part_type(mech, index, loop);
    data = mech_critical_data(mech, index, loop);
    wFireMode = mech_critical_fire_mode(mech, index, loop);
    if (equipment_is_ammunition(type)) {
      const int weapon = ammunition_to_weapon_index(type);
      btech_text_builder_append(
          &line, checked_string_suffix(weapon_catalogue_name(weapon), 3));
      btech_text_builder_append(
          &line, GetAmmoDesc_Model_Mode(
                     weapon, mech_critical_ammo_mode(mech, index, loop)));
      btech_text_builder_append(&line, " Ammo");
      if (!mech_critical_is_nonfunctional(mech, index, loop)) {
        btech_text_builder_append_format(&line, " [%3.3d/%3.3d]", data,
                                         FullAmmo(mech, index, loop));
      }

    } else {
      if (equipment_is_weapon(type) && (wFireMode & OS_MODE))
        btech_text_builder_append(&line, "OS ");
      PartDisplayName name = pos_part_name(mech, index, loop);
      btech_text_builder_append(&line, name.text);
      if (equipment_is_weapon(type) &&
          (((wFireMode & OS_MODE) && (wFireMode & OS_USED)) ||
           (wFireMode & ROCKET_FIRED)))
        btech_text_builder_append(&line, " (Empty)");
      if (wFireMode & WILL_JETTISON_MODE)
        btech_text_builder_append(&line, " (backpack)");

      if (equipment_is_weapon(type) && (wFireMode & REAR_MOUNT))
        btech_text_builder_append(&line, " (R)");
      if (!mech_critical_is_nonfunctional(mech, index, loop)) {
        if (special_from_equipment_index(type) == ARTEMIS_IV) {
          if (data) {
            btech_text_builder_append_format(&line, " [Controls Slot %d]",
                                             data);
          }
        }
      }
    }

    if (mech_critical_is_broken(mech, index, loop) && type != EMPTY &&
        (!mech_part_is_structural_placeholder(type) ||
         mech_section_is_destroyed(mech, index)))
      btech_text_builder_append(
          &line, mech_critical_is_destroyed(mech, index, loop) ? " (Destroyed)"
                                                               : " (Broken)");
    else if (mech_critical_is_disabled(mech, index, loop) && type != EMPTY)
      btech_text_builder_append(&line, " (Disabled)");
    else if (mech_critical_is_damaged(mech, index, loop) && type != EMPTY)
      btech_text_builder_append(&line, " (Damaged)");

    char **entry = checked_storage_at(foo, NUM_CRITICALS + 1, sizeof(*foo),
                                      (size_t)count++);
    *entry = strdup(buffer);
  }

  ArmorStringFromIndex(index, buffer, mech_class(mech),
                       mech_movement_type(mech));
  BtechTextBuilder title;
  title.text = buffer;
  title.capacity = sizeof(buffer);
  title.length = strlen(buffer);
  title.truncated = false;
  btech_text_builder_append(&title, " Criticals");
  cm = selected_column_string_menu(2, buffer, foo, (size_t)count);
  ShowCoolMenu(evaluation, player, cm);
  KillCoolMenu(cm);
  KillText(foo, (size_t)count);
}

const char *evaluate_ammo_amount(int now, int max) {
  int f = (now * 100) / max;

  if (f >= 50)
    return "[fg=green bold]";
  if (f >= 25)
    return "[fg=yellow bold]";
  return "[fg=red bold]";
}
