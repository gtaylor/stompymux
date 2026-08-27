#include "btech/context.h"
#include "btech_channel.h"
#include "checked_conversion.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_partnames_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"
#include "template_implementation.h"
#include "template_load_internal.h"
#include "weapon_catalogue_api.h"
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
typedef struct TemplateLoadScratch {
  char line[MAX_STRING_LENGTH];
  char buf[MAX_STRING_LENGTH];
  char cmd[MAX_STRING_LENGTH];
  char description_buffer[BTECH_TEXT_CAPACITY];
} TemplateLoadScratch;

typedef struct TemplateCriticalLoadRequest {
  FILE *file;
  DbRef player;
  Mech *mech;
  const char *filename;
  char *command;
  char *description;
  char *token;
  char *description_buffer;
  int section;
  int *is_clan;
} TemplateCriticalLoadRequest;

static bool template_critical_load(const TemplateCriticalLoadRequest *request) {
  FILE *fp = request->file;
  Mech *mech = request->mech;
  DbRef player = request->player;
  const char *filename = request->filename;
  char *cmd = request->command;
  char *ptr = request->description;
  char *buf = request->token;
  char *description_buffer = request->description_buffer;
  char *line2;
  int section = request->section;
  int x;
  int y;
  int value = 0;
  int type;
  int brand;
  int lpos;
  int hpos;
  int critical;
  int w_fire_modes;
  int w_ammo_modes;

  if (!template_parse_critical_range(cmd, &x, &y) || x > NUM_CRITICALS ||
      y > NUM_CRITICALS) {
    template_load_error(fp, mech, player, true, true,
                        "Error while loading: Invalid critical '%s'.", cmd);
    return false;
  }
  lpos = x - 1;
  hpos = y - 1;
  critical = lpos;
  line2 = template_description_read(&(TemplateDescriptionRead){
      .file = fp, .line = ptr, .buffer = description_buffer});
  line2 = template_token_parse(&(TemplateTokenRequest){
      .input = line2, .output = buf, .output_capacity = MAX_STRING_LENGTH});
  if (!strncasecmp(buf, "CL.", 3))
    *request->is_clan = 1;
  PartMatchResult match =
      part_match_next(&(PartMatchRequest){.context = mech->xcode.context,
                                          .pattern = buf,
                                          .kind = PART_MATCH_VERY_LONG,
                                          .cursor = -1});
  if (template_load_error(fp, mech, player, (!match.found) != 0, true,
                          "Unable to find %s", buf)) {
    return false;
  }
  type = match.part.id;
  brand = match.part.brand;
  mech_critical_part_type_set(mech, section, critical, type);
  if (!mech->xcode.context->configuration->btech_parts)
    brand = 0;
  mech_critical_brand_set(&(CriticalSlotBrandSet){
      .mech = mech,
      .slot = {.section = section, .critical = critical},
      .brand = brand});
  mech_critical_desired_ammo_section_set(mech, section, critical, -1);
  if (equipment_is_weapon(type)) {
    /* Thanks to legacy of past, we _do_ have to do this.. sniff */
    if (weapon_catalogue_is_anti_missile(weapon_from_equipment_index(type))) {
      if (weapon_catalogue_has_special(weapon_from_equipment_index(type), CLAT))
        ((mech)->rd.specials) |= CL_ANTI_MISSILE_TECH;
      else
        ((mech)->rd.specials) |= IS_ANTI_MISSILE_TECH;
    }
    mech_critical_data_set(mech, section, critical, 0);
    line2 = template_token_parse(&(TemplateTokenRequest){
        .input = line2, .output = buf, .output_capacity = MAX_STRING_LENGTH});
    line2 = template_token_parse(&(TemplateTokenRequest){
        .input = line2, .output = buf, .output_capacity = MAX_STRING_LENGTH});
    /*              wAmmoModes = BuildBitVector(crit_ammo_modes, buf); */
    w_fire_modes = clamp_long_to_int(
        build_bit_vector_with_delim(template_critical_fire_mode_names(),
                                    template_critical_fire_mode_count(), buf));
    w_ammo_modes = clamp_long_to_int(
        build_bit_vector_with_delim(template_critical_ammo_mode_names(),
                                    template_critical_ammo_mode_count(), buf));
    if (template_load_error(
            fp, mech, player, (w_fire_modes < 0 && w_ammo_modes < 0) != 0, true,
            "Error while loading: Invalid crit modes for weapon: %s.", buf)) {
      return false;
    }
    if (w_fire_modes < 0)
      w_fire_modes = 0;
    if (w_ammo_modes < 0)
      w_ammo_modes = 0;
    mech_critical_fire_mode_set(mech, section, critical, w_fire_modes);
    mech_critical_ammo_mode_set(mech, section, critical, w_ammo_modes);
    template_token_parse(&(TemplateTokenRequest){
        .input = line2, .output = buf, .output_capacity = MAX_STRING_LENGTH});
    if (mech->xcode.context->configuration->btech_parts &&
        !template_read_int(fp, mech, player, buf, &value))
      return false;
    if (mech->xcode.context->configuration->btech_parts && value != 0)
      mech_critical_brand_set(&(CriticalSlotBrandSet){
          .mech = mech,
          .slot = {.section = section, .critical = critical},
          .brand = value});
  } else if (equipment_is_ammunition(type)) {
    template_token_parse(&(TemplateTokenRequest){
        .input = line2, .output = buf, .output_capacity = MAX_STRING_LENGTH});
    if (!template_read_int(fp, mech, player, buf, &value))
      return false;
    mech_critical_data_set(mech, section, critical, value);
    template_token_parse(&(TemplateTokenRequest){
        .input = line2, .output = buf, .output_capacity = MAX_STRING_LENGTH});
    /*              wFireModes = BuildBitVector(crit_fire_modes, buf); */
    /*              wAmmoModes = BuildBitVector(crit_ammo_modes, buf); */
    w_fire_modes = clamp_long_to_int(
        build_bit_vector_with_delim(template_critical_fire_mode_names(),
                                    template_critical_fire_mode_count(), buf));
    w_ammo_modes = clamp_long_to_int(
        build_bit_vector_with_delim(template_critical_ammo_mode_names(),
                                    template_critical_ammo_mode_count(), buf));
    if (template_load_error(
            fp, mech, player, (w_fire_modes < 0 && w_ammo_modes < 0) != 0, true,
            "Error while loading: Invalid crit modes for ammo: %s.", buf)) {
      return false;
    }
    if (w_fire_modes < 0)
      w_fire_modes = 0;
    if (w_ammo_modes < 0)
      w_ammo_modes = 0;
    mech_critical_fire_mode_set(mech, section, critical, w_fire_modes);
    mech_critical_ammo_mode_set(mech, section, critical, w_ammo_modes);
    if (mech_critical_data(mech, section, critical) <
        full_ammo(mech, section, critical)) {
      mech_critical_fire_mode_add(mech, section, critical, HALFTON_MODE);
      if (mech_critical_data(mech, section, critical) >
          full_ammo(mech, section, critical))
        mech_critical_fire_mode_clear(mech, section, critical, HALFTON_MODE);
    }
    if (mech_critical_data(mech, section, critical) !=
            full_ammo(mech, section, critical) &&
        ((mech)->ud.type) != CLASS_MW && ((mech)->ud.type) != CLASS_BSUIT) {
      btech_channel_send(
          mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS,
          "Invalid ammo crit for %s in #%ld %s (%d/%d)",
          weapon_catalogue_name(ammunition_to_weapon_index(type)), mech->mynum,
          filename, mech_critical_data(mech, section, critical),
          full_ammo(mech, section, critical));
      mech_critical_data_set(mech, section, critical,
                             full_ammo(mech, section, critical));
    }
  } else {
    if (template_token_parse(
            &(TemplateTokenRequest){.input = line2,
                                    .output = buf,
                                    .output_capacity = MAX_STRING_LENGTH})) {
      if (!template_read_int(fp, mech, player, buf, &value))
        return false;
      mech_critical_data_set(mech, section, critical, value);
    } else {
      mech_critical_data_set(mech, section, critical, 0);
    }
    mech_critical_fire_mode_set(mech, section, critical, 0);
    mech_critical_ammo_mode_set(mech, section, critical, 0);
    if (template_token_parse(
            &(TemplateTokenRequest){.input = line2,
                                    .output = buf,
                                    .output_capacity = MAX_STRING_LENGTH})) {
      if (template_token_parse(
              &(TemplateTokenRequest){.input = line2,
                                      .output = buf,
                                      .output_capacity = MAX_STRING_LENGTH})) {
        if (mech->xcode.context->configuration->btech_parts) {
          if (!template_read_int(fp, mech, player, buf, &value))
            return false;
          if (value != 0)
            mech_critical_brand_set(&(CriticalSlotBrandSet){
                .mech = mech,
                .slot = {.section = section, .critical = critical},
                .brand = value});
        }
      }
    }
  }
  for (x = (lpos + 1); x <= hpos; x++) {
    mech_critical_part_type_set(mech, section, x,
                                mech_critical_part_type(mech, section, lpos));
    mech_critical_data_set(mech, section, x,
                           mech_critical_data(mech, section, lpos));
    mech_critical_fire_mode_set(mech, section, x,
                                mech_critical_fire_mode(mech, section, lpos));
    mech_critical_ammo_mode_set(mech, section, x,
                                mech_critical_ammo_mode(mech, section, lpos));
    mech_critical_brand_set(&(CriticalSlotBrandSet){
        .mech = mech,
        .slot = {.section = section, .critical = x},
        .brand = mech_critical_brand(mech, section, lpos)});
  }

  return true;
}

static int load_template_internal(DbRef player, Mech *mech, char *filename,
                                  TemplateLoadScratch *scratch) {
  char *line = scratch->line;
  char *buf = scratch->buf;
  int value = 0;
  float decimal_value;
  char *cmd = scratch->cmd;
  char *description_buffer = scratch->description_buffer;
  char *ptr;
  int section = 0;
  int selection;
  int type;
  FILE *fp = fopen(filename, "r");
  char *tmpc;
  int ok_count = 0;
  int is_clan = 0;
  if (!fp)
    return -1;
  ptr = strrchr(filename, '/');
  if (ptr == nullptr) {
    ptr = filename;
  } else {
    ptr = checked_mutable_string_suffix(ptr, 1);
  }
  (void)string_copy_bounded(mech->ud.mech_type, sizeof(mech->ud.mech_type),
                            ptr);
  mech_radio_configuration_set(mech, 0);
  while (fgets(line, 512, fp)) {
    size_t line_length = strlen(line);
    if (line_length > 0) {
      char *last = checked_storage_at(line, MAX_STRING_LENGTH, sizeof(*line),
                                      line_length - 1);
      if (*last == '\n')
        *last = '\0';
    }
    size_t leading = strspn(line, " \t\n\v\f\r");
    if (leading > 0) {
      char *content = checked_mutable_string_suffix(line, leading);
      memmove(line, content, strlen(content) + 1);
    }
    ptr = strpbrk(line, " \t");
    if (ptr) {
      size_t command_length = (size_t)(ptr - line);
      memcpy(cmd, line, command_length);
      char *terminator = checked_storage_at(cmd, MAX_STRING_LENGTH,
                                            sizeof(*cmd), command_length);
      *terminator = '\0';
      ptr = checked_mutable_string_suffix(ptr, 1);
      ptr = checked_mutable_string_suffix(ptr, strspn(ptr, " \t\n\v\f\r"));
    } else {
      (void)string_copy_bounded(cmd, MAX_STRING_LENGTH, line);
      line[0] = '\0';
      ptr = nullptr;
    }
    if (!strncasecmp(cmd, "CRIT_", 5)) {
      selection = 9999;
    } else {
      selection = compare_const_array(template_load_command_names(),
                                      template_load_command_count(), cmd);
    }
    if (selection == -1) {
      /* Initial premise: we will have a mech type before we get to this */
      section = find_section(cmd, ((mech)->ud.type), ((mech)->ud.move));
      if (template_load_error(
              fp, mech, player, (section == -1 && !ok_count) != 0, false,
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
      tmpc = template_description_read(&(TemplateDescriptionRead){
          .file = fp, .line = ptr, .buffer = description_buffer});
      if (strcmp(tmpc, ((mech)->ud.mech_type))) {
        btech_channel_send(
            mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS,
            "Template %s has Reference <-> Filename mismatch : %s <-> "
            "%s - It is automatically fixed by saving again.",
            filename, tmpc, ((mech)->ud.mech_type));
        tmpc = ((mech)->ud.mech_type);
      }
      if (tmpc != ((mech)->ud.mech_type)) {
        (void)string_copy_bounded(((mech)->ud.mech_type),
                                  sizeof(((mech)->ud.mech_type)), tmpc);
      }
      break;
    case 1: /* Type */
      tmpc = template_description_read(&(TemplateDescriptionRead){
          .file = fp, .line = ptr, .buffer = description_buffer});
      type = compare_const_array(template_unit_class_names(),
                                 template_unit_class_count(), tmpc);
      if (template_load_error(fp, mech, player, type == -1, true,
                              "Error while loading: Type %s not found.",
                              tmpc)) {
        return -1;
      }
      ((mech)->ud.type) = clamp_int_to_char(type);
      ((mech)->ud.fuel) = ((mech)->ud.fuel_orig) = default_fuel_by_type(mech);
      break;
    case 2: /* Movement Type */
      tmpc = template_description_read(&(TemplateDescriptionRead){
          .file = fp, .line = ptr, .buffer = description_buffer});
      type = compare_const_array(template_movement_type_names(),
                                 template_movement_type_count(), tmpc);
      if (template_load_error(fp, mech, player, type == -1, true,
                              "Error while loading: Type %s not found.",
                              tmpc)) {
        return -1;
      }
      ((mech)->ud.move) = clamp_int_to_char(type);
      break;
    case 3: /* Tons */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      ((mech)->ud.tons) = value;
      break;
    case 4: /* Tac_Range */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_tactical_range_set(mech, value);
      break;
    case 5: /* LRS_Range */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_long_range_sensor_range_set(mech, value);
      break;
    case 6: /* Radio Range */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_radio_range_set(mech, value);
      break;
    case 7: /* Scan Range */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_scanner_range_set(mech, value);
      break;
    case 8: /* Heat Sinks */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      ((mech)->ud.numsinks) = clamp_int_to_char(value);
      break;
    case 9: /* Max Speed */
      if (!template_read_float(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &decimal_value))
        return -1;
      mech_max_speed_set(mech, decimal_value);
      ((mech)->ud.template_maxspeed) = ((mech)->ud.maxspeed);
      break;
    case 10: /* Specials */
      tmpc = template_description_read(&(TemplateDescriptionRead){
          .file = fp, .line = ptr, .buffer = description_buffer});
      if (check_specials_list(primary_technology_names(),
                              primary_technology_name_count(),
                              secondary_technology_names(),
                              secondary_technology_name_count(), tmpc)) {
        ((mech)->rd.specials) |= build_bit_vector_no_err(
            primary_technology_names(), primary_technology_name_count(), tmpc);
        ((mech)->rd.specials2) |=
            build_bit_vector_no_err(secondary_technology_names(),
                                    secondary_technology_name_count(), tmpc);
      } else if (template_load_error(
                     fp, mech, player, ((mech)->rd.specials) == -1, true,
                     "Error while loading: Invalid specials - %s.", tmpc)) {
        return -1;
      }
      break;
    case 11: /* Armor */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_section_original_armor_set(mech, section, value);
      mech_section_armor_set(mech, section,
                             mech_section_original_armor(mech, section));
      break;
    case 12: /* Internals */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_section_original_internal_set(mech, section, value);
      mech_section_internal_set(mech, section,
                                mech_section_original_internal(mech, section));
      break;
    case 13: /* Rear */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_section_original_rear_armor_set(mech, section, value);
      mech_section_rear_armor_set(
          mech, section, mech_section_original_rear_armor(mech, section));
      break;
    case 14: /* Config */
      tmpc = template_description_read(&(TemplateDescriptionRead){
          .file = fp, .line = ptr, .buffer = description_buffer});
      mech_section_configuration_set(
          mech, section,
          clamp_long_to_int(
              build_bit_vector(template_section_configuration_names(),
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
      if (!template_critical_load(&(TemplateCriticalLoadRequest){
              .file = fp,
              .player = player,
              .mech = mech,
              .filename = filename,
              .command = cmd,
              .description = ptr,
              .token = buf,
              .description_buffer = description_buffer,
              .section = section,
              .is_clan = &is_clan,
          })) {
        return -1;
      }
      break;
    case 15: /* Mech's Computer level */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_computer_quality_set(mech, value);
      break;
    case 16: /* Name of the mech */
      (void)string_copy_bounded(
          ((mech)->ud.mech_name), sizeof(((mech)->ud.mech_name)),
          template_description_read(&(TemplateDescriptionRead){
              .file = fp, .line = ptr, .buffer = description_buffer}));
      break;
    case 17: /* Jj's */
      if (!template_read_float(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &decimal_value))
        return -1;
      ((mech)->rd.jumpspeed) = decimal_value;
      break;
    case 18: /* Radio */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_radio_quality_set(mech, value);
      break;
    case 19: /* SI */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      ((mech)->ud.si) = ((mech)->ud.si_orig) = clamp_int_to_char(value);
      break;
    case 20: /* Fuel */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      ((mech)->ud.fuel) = ((mech)->ud.fuel_orig) = value;
      break;
    case 21: /* Comment */
      break;
    case 22: /* Radio_freqs */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      mech_radio_configuration_set(mech, value);
      break;
    case 23: /* Mech battle value */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      ((mech)->ud.mechbv) = value;
      break;
    case 24: /* Cargospace */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      ((mech)->ud.cargospace) = value;
      break;
    case 25: /* Maxsuits */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      ((mech)->rd.maxsuits) = value;
      break;
    case 26: /* Specials */
      tmpc = template_description_read(&(TemplateDescriptionRead){
          .file = fp, .line = ptr, .buffer = description_buffer});
      if (check_specials_list(infantry_technology_names(),
                              infantry_technology_name_count(), nullptr, 0,
                              tmpc))
        ((mech)->rd.infantry_specials) |=
            build_bit_vector_no_err(infantry_technology_names(),
                                    infantry_technology_name_count(), tmpc);
      break;
    case 27: /* Carmaxton */
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      ((mech)->ud.carmaxton) = clamp_int_to_char(value);
      break;
    case 28:
      if (!template_read_int(
              fp, mech, player,
              template_description_read(&(TemplateDescriptionRead){
                  .file = fp, .line = ptr, .buffer = description_buffer}),
              &value))
        return -1;
      ((mech)->ud.hsengoverride) = value;
      break;
    case 29:
      tmpc = template_description_read(&(TemplateDescriptionRead){
          .file = fp, .line = ptr, .buffer = description_buffer});
      if (strlen(tmpc) == 1) /* just the \0 */
        (void)string_copy_bounded(((mech)->ud.unit_era),
                                  sizeof(((mech)->ud.unit_era)), "Undefined");
      else
        (void)string_copy_bounded(((mech)->ud.unit_era),
                                  sizeof(((mech)->ud.unit_era)), tmpc);
      break;
    case 30:
      tmpc = template_description_read(&(TemplateDescriptionRead){
          .file = fp, .line = ptr, .buffer = description_buffer});
      if (strlen(tmpc) == 1) /* just the \0 */
        (void)string_copy_bounded(((mech)->ud.unit_tro),
                                  sizeof(((mech)->ud.unit_tro)), "Undefined");
      else
        (void)string_copy_bounded(((mech)->ud.unit_tro),
                                  sizeof(((mech)->ud.unit_tro)), tmpc);
      break;
    }
  }
  if (fclose(fp) != 0)
    return -1;
  template_load_finalize(mech, is_clan != 0);
  return 0;
}

int load_template(DbRef player, Mech *mech, char *filename) {
  TemplateLoadScratch *scratch = checked_storage_allocate(sizeof(*scratch));
  int result = load_template_internal(player, mech, filename, scratch);
  free_buf(scratch);
  return result;
}
