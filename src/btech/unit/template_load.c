#include "btech_channel.h"
#include "checked_conversion.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "template_implementation.h"
#include "weapon_catalogue_api.h"

static bool template_load_error(FILE *fp, Mech *mech, DbRef player,
                                bool condition, bool global, const char *format,
                                ...) __attribute__((format(printf, 6, 7)));

static bool template_load_error(FILE *fp, Mech *mech, DbRef player,
                                bool condition, bool global, const char *format,
                                ...) {
  if (!condition) {
    return false;
  }
#ifdef TEMPLATE_VERBOSE_ERRORS
  char message[LBUF_SIZE] = {0};
  va_list args;
  va_start(args, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  vsnprintf(message, sizeof(message), format, args);
  va_end(args);
  if (global) {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       message);
  } else {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 message);
  }
#else
  (void)mech;
  (void)player;
  (void)global;
  (void)format;
#endif
  if (fp) {
    if (fclose(fp) != 0)
      return true;
  }
  return true;
}

int load_template(DbRef player, Mech *mech, char *filename) {
  char line[MAX_STRING_LENGTH], buf[MAX_STRING_LENGTH];
  int x, y, value, i;
  char cmd[MAX_STRING_LENGTH];
  char *ptr, *line2;
  int section = 0, critical, selection, type, brand;
  FILE *fp = fopen(filename, "r");
  char *tmpc;
  int lpos, hpos;
  int ok_count = 0;
  int isClan = 0;
  int t;
  int wFireModes, wAmmoModes;
  BattleMap *map;

  if (!fp)
    return -1;

  ptr = strrchr(filename, '/');
  if (ptr == nullptr) {
    ptr = filename;
  } else {
    ptr = checked_mutable_string_suffix(ptr, 1);
  }
  strncpy(((mech)->ud.mech_type), ptr, 25);
  ((mech)->ud.mech_type)[24] = '\0';

  silly_atr_set_in(mech->xcode.context->database, mech->mynum, A_MECHTYPE,
                   ((mech)->ud.mech_type));
  mech_radio_configuration_set(mech, 0);
  while (fgets(line, 512, fp)) {
    size_t line_length = strlen(line);
    if (line_length > 0) {
      char *last = checked_storage_at(line, sizeof(line), sizeof(*line),
                                      line_length - 1);
      if (*last == '\n')
        *last = '\0';
    }
    size_t leading = strspn(line, " \t\n\v\f\r");
    if (leading > 0) {
      char *content = checked_mutable_string_suffix(line, leading);
      memmove(line, content, strlen(content) + 1);
    }
    if ((ptr = strpbrk(line, " \t"))) {
      size_t command_length = (size_t)(ptr - line);
      memcpy(cmd, line, command_length);
      char *terminator =
          checked_storage_at(cmd, sizeof(cmd), sizeof(*cmd), command_length);
      *terminator = '\0';
      ptr = checked_mutable_string_suffix(ptr, 1);
      ptr = checked_mutable_string_suffix(ptr, strspn(ptr, " \t\n\v\f\r"));
    } else {
      strlcpy(cmd, line, sizeof(cmd));
      strcpy(line, "");
      ptr = NULL;
    }
    if (!strncasecmp(cmd, "CRIT_", 5))
      selection = 9999;
    else if ((selection = compare_const_array(
                  load_cmds, template_load_command_count(), cmd)) == -1) {
      /* Initial premise: we will have a mech type before we get to this */
      section = find_section(cmd, ((mech)->ud.type), ((mech)->ud.move));
      if (template_load_error(
              fp, mech, player, section == -1 && !ok_count, false,
              "New template loading system: %s is invalid template file.",
              filename)) {
        return -1;
      }
      if (section == -1) {
        template_load_error(fp, mech, player, true, true,
                            "Error while loading: Section %s not found.", cmd);
        return -1;
      }
      mech_section_recycle_ticks_set(mech, section, 0);
      ok_count++;
      continue;
    }
    ok_count++;
    switch (selection) {
    case 0: /* Reference */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      if (strcmp(tmpc, ((mech)->ud.mech_type))) {
        btech_channel_send(
            mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Template %s has Reference <-> Filename mismatch : %s <-> "
                    "%s - It is automatically fixed by saving again.",
                    filename, tmpc, ((mech)->ud.mech_type)));
        tmpc = ((mech)->ud.mech_type);
      }
      silly_atr_set_in(mech->xcode.context->database, mech->mynum, A_MECHTYPE,
                       tmpc);
      strlcpy(((mech)->ud.mech_type), tmpc, sizeof(((mech)->ud.mech_type)));
      break;
    case 1: /* Type */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      type = compare_const_array(mech_types, template_unit_class_count(), tmpc);
      if (template_load_error(fp, mech, player, type == -1, true,
                              "Error while loading: Type %s not found.",
                              tmpc)) {
        return -1;
      }
      ((mech)->ud.type) = clamp_int_to_char(type);
      ((mech)->ud.fuel) = ((mech)->ud.fuel_orig) = DefaultFuelByType(mech);
      break;
    case 2: /* Movement Type */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      type =
          compare_const_array(move_types, template_movement_type_count(), tmpc);
      if (template_load_error(fp, mech, player, type == -1, true,
                              "Error while loading: Type %s not found.",
                              tmpc)) {
        return -1;
      }
      ((mech)->ud.move) = clamp_int_to_char(type);
      break;
    case 3: /* Tons */
      ((mech)->ud.tons) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 4: /* Tac_Range */
      mech_tactical_range_set(
          mech, atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 5: /* LRS_Range */
      mech_long_range_sensor_range_set(
          mech, atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 6: /* Radio Range */
      mech_radio_range_set(
          mech, atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 7: /* Scan Range */
      mech_scanner_range_set(
          mech, atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 8: /* Heat Sinks */
      ((mech)->ud.numsinks) = clamp_int_to_char(
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 9: /* Max Speed */
      mech_max_speed_set(
          mech,
          strtof(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}), nullptr));
      ((mech)->ud.template_maxspeed) = ((mech)->ud.maxspeed);
      break;
    case 10: /* Specials */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});

      if (CheckSpecialsList(specials, primary_technology_name_count(),
                            specials2, secondary_technology_name_count(),
                            tmpc)) {
        ((mech)->rd.specials) |= BuildBitVectorNoErr(
            specials, primary_technology_name_count(), tmpc);
        ((mech)->rd.specials2) |= BuildBitVectorNoErr(
            specials2, secondary_technology_name_count(), tmpc);
      } else if (template_load_error(
                     fp, mech, player, ((mech)->rd.specials) == -1, true,
                     "Error while loading: Invalid specials - %s.", tmpc)) {
        return -1;
      }
      break;
    case 11: /* Armor */
      mech_section_original_armor_set(
          mech, section,
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      mech_section_armor_set(mech, section,
                             mech_section_original_armor(mech, section));
      break;
    case 12: /* Internals */
      mech_section_original_internal_set(
          mech, section,
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      mech_section_internal_set(mech, section,
                                mech_section_original_internal(mech, section));
      break;
    case 13: /* Rear */
      mech_section_original_rear_armor_set(
          mech, section,
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      mech_section_rear_armor_set(
          mech, section, mech_section_original_rear_armor(mech, section));
      break;
    case 14: /* Config */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      mech_section_configuration_set(
          mech, section,
          clamp_long_to_int(
              BuildBitVector(section_configs,
                             template_section_configuration_count(), tmpc) &
              ~(CASE_TECH | SECTION_DESTROYED)));
      if (template_load_error(
              fp, mech, player, mech_section_configuration(mech, section) == -1,
              true, "Error while loading: Invalid location config: %s.",
              tmpc)) {
        return -1;
      }
      break;
    case 9999:
      if ((sscanf(cmd, "CRIT_%d-%d", &x, &y)) == 2) {
        lpos = x - 1;
        hpos = y - 1;
      } else if ((sscanf(cmd, "CRIT_%d", &x)) == 1) {
        lpos = x - 1;
        hpos = x - 1;
      } else
        break;
      critical = lpos;
      line2 = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      line2 = one_arg(line2, buf, sizeof(buf));
      if (!strncasecmp(buf, "CL.", 3))
        isClan = 1;
      if (template_load_error(fp, mech, player,
                              !find_matching_vlong_part(mech->xcode.context,
                                                        buf, NULL, &type,
                                                        &brand),
                              true, "Unable to find %s", buf)) {
        return -1;
      }
      mech_critical_part_type_set(mech, section, critical, type);
      if (!mech->xcode.context->configuration->btech_parts)
        brand = 0;
      mech_critical_brand_set(mech, section, critical, brand);
      mech_critical_desired_ammo_section_set(mech, section, critical, -1);

      if (equipment_is_weapon(type)) {
        /* Thanks to legacy of past, we _do_ have to do this.. sniff */
        if (weapon_catalogue_is_anti_missile(
                weapon_from_equipment_index(type))) {
          if (weapon_catalogue_has_special(weapon_from_equipment_index(type),
                                           CLAT))
            ((mech)->rd.specials) |= CL_ANTI_MISSILE_TECH;
          else
            ((mech)->rd.specials) |= IS_ANTI_MISSILE_TECH;
        }
        mech_critical_data_set(mech, section, critical, 0);
        line2 = one_arg(line2, buf, sizeof(buf)); /* Don't need the '-' */
        line2 = one_arg(line2, buf, sizeof(buf));

        /*              wFireModes = BuildBitVector(crit_fire_modes, buf); */

        /*              wAmmoModes = BuildBitVector(crit_ammo_modes, buf); */

        wFireModes = clamp_long_to_int(BuildBitVectorWithDelim(
            crit_fire_modes, template_critical_fire_mode_count(), buf));
        wAmmoModes = clamp_long_to_int(BuildBitVectorWithDelim(
            crit_ammo_modes, template_critical_ammo_mode_count(), buf));

        if (template_load_error(
                fp, mech, player, wFireModes < 0 && wAmmoModes < 0, true,
                "Error while loading: Invalid crit modes for weapon: %s.",
                buf)) {
          return -1;
        }

        if (wFireModes < 0)
          wFireModes = 0;

        if (wAmmoModes < 0)
          wAmmoModes = 0;

        mech_critical_fire_mode_set(mech, section, critical, wFireModes);
        mech_critical_ammo_mode_set(mech, section, critical, wAmmoModes);

        line2 = one_arg(line2, buf, sizeof(buf));
        if (mech->xcode.context->configuration->btech_parts)
          if (atoi(buf)) {
            mech_critical_brand_set(mech, section, critical, atoi(buf));
          }
      } else if (equipment_is_ammunition(type)) {
        line2 = one_arg(line2, buf, sizeof(buf));
        mech_critical_data_set(mech, section, critical, atoi(buf));
        line2 = one_arg(line2, buf, sizeof(buf));

        /*              wFireModes = BuildBitVector(crit_fire_modes, buf); */

        /*              wAmmoModes = BuildBitVector(crit_ammo_modes, buf); */

        wFireModes = clamp_long_to_int(BuildBitVectorWithDelim(
            crit_fire_modes, template_critical_fire_mode_count(), buf));
        wAmmoModes = clamp_long_to_int(BuildBitVectorWithDelim(
            crit_ammo_modes, template_critical_ammo_mode_count(), buf));

        if (template_load_error(
                fp, mech, player, wFireModes < 0 && wAmmoModes < 0, true,
                "Error while loading: Invalid crit modes for ammo: %s.", buf)) {
          return -1;
        }

        if (wFireModes < 0)
          wFireModes = 0;

        if (wAmmoModes < 0)
          wAmmoModes = 0;

        mech_critical_fire_mode_set(mech, section, critical, wFireModes);
        mech_critical_ammo_mode_set(mech, section, critical, wAmmoModes);

        if (mech_critical_data(mech, section, critical) <
            FullAmmo(mech, section, critical)) {
          mech_critical_fire_mode_add(mech, section, critical, HALFTON_MODE);
          if (mech_critical_data(mech, section, critical) >
              FullAmmo(mech, section, critical))
            mech_critical_fire_mode_clear(mech, section, critical,
                                          HALFTON_MODE);
        }

        if (mech_critical_data(mech, section, critical) !=
                FullAmmo(mech, section, critical) &&
            ((mech)->ud.type) != CLASS_MW && ((mech)->ud.type) != CLASS_BSUIT) {
          btech_channel_send(
              mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
              tprintf("Invalid ammo crit for %s in #%ld %s (%d/%d)",
                      weapon_catalogue_name(ammunition_to_weapon_index(type)),
                      mech->mynum, filename,
                      mech_critical_data(mech, section, critical),
                      FullAmmo(mech, section, critical)));
          mech_critical_data_set(mech, section, critical,
                                 FullAmmo(mech, section, critical));
        }
      } else {
        if ((line2 = one_arg(line2, buf, sizeof(buf))))
          mech_critical_data_set(mech, section, critical, atoi(buf));
        else
          mech_critical_data_set(mech, section, critical, 0);
        mech_critical_fire_mode_set(mech, section, critical, 0);
        mech_critical_ammo_mode_set(mech, section, critical, 0);
        if ((line2 = one_arg(line2, buf, sizeof(buf))))
          if ((line2 = one_arg(line2, buf, sizeof(buf)))) {
            if (mech->xcode.context->configuration->btech_parts)
              if (atoi(buf)) {
                mech_critical_brand_set(mech, section, critical, atoi(buf));
              }
          }
      }
      for (x = (lpos + 1); x <= hpos; x++) {
        mech_critical_part_type_set(
            mech, section, x, mech_critical_part_type(mech, section, lpos));
        mech_critical_data_set(mech, section, x,
                               mech_critical_data(mech, section, lpos));
        mech_critical_fire_mode_set(
            mech, section, x, mech_critical_fire_mode(mech, section, lpos));
        mech_critical_ammo_mode_set(
            mech, section, x, mech_critical_ammo_mode(mech, section, lpos));
        mech_critical_brand_set(mech, section, x,
                                mech_critical_brand(mech, section, lpos));
      }
      break;
    case 15: /* Mech's Computer level */
      mech_computer_quality_set(
          mech, atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 16: /* Name of the mech */
      strlcpy(((mech)->ud.mech_name),
              read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}),
              sizeof(((mech)->ud.mech_name)));
      break;
    case 17: /* Jj's */
      ((mech)->rd.jumpspeed) =
          strtof(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}), nullptr);
      break;
    case 18: /* Radio */
      mech_radio_quality_set(
          mech, atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 19: /* SI */
      ((mech)->ud.si) = ((mech)->ud.si_orig) = clamp_int_to_char(
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 20: /* Fuel */
      ((mech)->ud.fuel) = ((mech)->ud.fuel_orig) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 21: /* Comment */
      break;
    case 22: /* Radio_freqs */
      mech_radio_configuration_set(
          mech, atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 23: /* Mech battle value */
      ((mech)->ud.mechbv) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 24: /* Cargospace */
      ((mech)->ud.cargospace) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 25: /* Maxsuits */
      ((mech)->rd.maxsuits) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 26: /* Specials */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});

      if (CheckSpecialsList(infantry_specials, infantry_technology_name_count(),
                            nullptr, 0, tmpc))
        ((mech)->rd.infantry_specials) |= BuildBitVectorNoErr(
            infantry_specials, infantry_technology_name_count(), tmpc);

      break;
    case 27: /* Carmaxton */
      ((mech)->ud.carmaxton) = clamp_int_to_char(
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      break;
    case 28:
      ((mech)->ud.hsengoverride) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 29:
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      if (strlen(tmpc) == 1) /* just the \0 */
        strcpy(((mech)->ud.unit_era), "Undefined");
      else
        strlcpy(((mech)->ud.unit_era), tmpc, sizeof(((mech)->ud.unit_era)));
      break;
    case 30:
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      if (strlen(tmpc) == 1) /* just the \0 */
        strcpy(((mech)->ud.unit_tro), "Undefined");
      else
        strlcpy(((mech)->ud.unit_tro), tmpc, sizeof(((mech)->ud.unit_tro)));
      break;
    }
  }
  if (fclose(fp) != 0)
    return -1;
  ((mech)->rd.erat) = mech_calculated_engine_rating(mech);
  /* So we're not getting 'blank' ERA/TRO values, we'll default to 'Undefined'
   */
  if (strlen(((mech)->ud.unit_era)) == 0) {
    strcpy(((mech)->ud.unit_era), "Undefined");
  }
  if (strlen(((mech)->ud.unit_tro)) == 0) {
    strcpy(((mech)->ud.unit_tro), "Undefined");
  }
  if (!(((mech)->rd.specials) & ICE_TECH) && !((mech)->ud.numsinks))
    ((mech)->ud.numsinks) = DEFAULT_HEATSINKS;
  if (((mech)->ud.type) == CLASS_MECH)
    do_sub_magic(mech, 1);
  if (((mech)->ud.type) == CLASS_MW)
    mech_power_up(mech);

  if (((mech)->ud.type) == CLASS_MECH)
    value = 8;
  else
    value = 6;

  if (mech->xcode.context->configuration->btech_parts)
    for (x = 0; x < value; x++)
      for (y = 0; y < CritsInLoc(mech, x); y++)
        if ((t = mech_critical_part_type(mech, x, y))) {
          if (mech_critical_brand(mech, x, y))
            continue;
          if (equipment_is_ammunition(t))
            continue;
          if (equipment_is_bomb(t))
            continue;
          mech_critical_brand_set(
              mech, x, y, isClan ? DEFAULT_CLPART_LEVEL : DEFAULT_PART_LEVEL);
        }
  if (isClan) {
    if (!mech_computer_quality(mech))
      mech_computer_quality_set(mech, DEFAULT_CLCOMPUTER);
    if (!mech_radio_quality(mech))
      mech_radio_quality_set(mech, DEFAULT_CLRADIO);
  } else {
    if (!mech_computer_quality(mech))
      mech_computer_quality_set(mech, DEFAULT_COMPUTER);
    if (!mech_radio_quality(mech))
      mech_radio_quality_set(mech, DEFAULT_RADIO);
  }
  if (!mech_radio_configuration(mech))
    mech_radio_configuration_set(
        mech, generic_radio_type(mech_radio_quality(mech), isClan));
  if (!mech_computer_quality(mech)) {
    if (!mech_scanner_range(mech))
      mech_scanner_range_set(mech, DEFAULT_SCANRANGE);
    if (!mech_long_range_sensor_range(mech))
      mech_long_range_sensor_range_set(mech, DEFAULT_LRSRANGE);
    if (!mech_radio_range(mech))
      mech_radio_range_set(mech, DEFAULT_RADIORANGE);
    if (!mech_tactical_range(mech))
      mech_tactical_range_set(mech, DEFAULT_TACRANGE);
  } else {
    if (!mech_scanner_range(mech))
      mech_scanner_range_set(mech, mech_default_scanner_range(mech));
    if (!mech_long_range_sensor_range(mech))
      mech_long_range_sensor_range_set(
          mech, mech_default_long_range_sensor_range(mech));
    if (!mech_radio_range(mech))
      mech_radio_range_set(mech, mech_default_radio_range(mech));
    if (!mech_tactical_range(mech))
      mech_tactical_range_set(mech, mech_default_tactical_range(mech));
  }
#if 1 /* Don't know if we're ready for this yet - aw, what the hell :) */
  ((mech)->rd.specials) &= ~FLIPABLE_ARMS;
  if (((mech)->ud.type) == CLASS_MECH)
    if ((mech_critical_part_type(mech, LARM, 2) !=
         special_equipment_index(LOWER_ACTUATOR)) &&
        (mech_critical_part_type(mech, RARM, 2) !=
         special_equipment_index(LOWER_ACTUATOR)) &&
        (mech_critical_part_type(mech, LARM, 3) !=
         special_equipment_index(HAND_OR_FOOT_ACTUATOR)) &&
        (mech_critical_part_type(mech, RARM, 3) !=
         special_equipment_index(HAND_OR_FOOT_ACTUATOR)))
      ((mech)->rd.specials) |= FLIPABLE_ARMS;
#endif
  update_specials(mech);
  ((mech)->rd.xpmod) = 1.0;      /* Default it to 1 (no mod effect at all) */
  ((mech)->rd.units_killed) = 0; /* Clear the mechs killed */
  mech_int_check(mech, 1);
  x = mech_weight_sub(GOD, mech, 0);
  y = ((mech)->ud.tons) * 1024;
  /* While we're at it, lets report those that are overweight */
  if ((x - y) > 40)
    if (((mech)->ud.type) != CLASS_BSUIT && ((mech)->ud.move) != MOVE_NONE)
      btech_channel_send(
          mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
          tprintf(
              "Error in %s template: %.1f tons of 'stuff', yet %d ton frame.",
              ((mech)->ud.mech_type), x / 1024.0, y / 1024));
  update_oweight(mech, x);
  if ((map = btech_context_get_map(mech->xcode.context, mech->mapindex)))
    map_conditions_apply(mech, map);
  /* To prevent certain funny occurences.. */
  for (i = 0; i < NUM_SECTIONS; i++) {
    if (!(mech_section_original_internal(mech, i))) {
    }
  }
  return 0;
}
