#include "btconfig.h"
#include "checked_conversion.h"
#include "equipment_types.h"
#include "mech_crew_api.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_targeting_api.h"
#include "mech_template_api.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_status_types.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "section_types.h"
#include "template_api.h"
#include "weapon_catalogue_api.h"

void mech_template_clear(Mech *mech, bool clear_communications) {
  mech_template_state_reset(mech);

  mech_spotter_dbref_set(mech, -1);
  mech_targeting_target_clear(mech);
  mech_charge_reset(mech);
  ((mech)->rd.swarming) = -1;
  ((mech)->rd.swarmedby) = -1;
  mech_dfa_target_dbref_set(mech, -1);
  ((mech)->rd.status) = 0;
  mech_pilot_dbref_set(mech, -1);
  mech_targeting_aim_reset(mech);
  mech_event_cancel(mech, EVENT_VEHICLEBURN);
  if (clear_communications)
    mech_communications_clear(mech);
}

static int template_load_modern(DbRef player, Mech *mech, const char *id) {
  FILE *fp = nullptr;
  char *filename;

  filename = mech_template_resolve_path(
      mech_context(mech), btech_context_mech_template_path(mech_context(mech)),
      id);

  if (!filename)
    return 0;
  fp = fopen(filename, "r");
  if (!fp)
    return 0;
  if (fclose(fp) != 0)
    return 0;
  return load_template(player, mech, filename) >= 0 ? 1 : 0;
}

extern const int DEFAULT_WEAPON_COUNT;

static int template_part_type_is_invalid(int i) {
  if (!i)
    return 0;
  if (equipment_is_weapon(i)) {
    if (i > (DEFAULT_WEAPON_COUNT))
      return 1;
  }
  if (equipment_is_ammunition(i)) {
    if ((ammunition_to_weapon_index(i) + 1) > (DEFAULT_WEAPON_COUNT))
      return 1;
  }
  if (equipment_is_special(i))
    if (special_from_equipment_index(i) >= count_special_items())
      return 1;
  return 0;
}

static bool template_load_error(FILE *fp, Mech *mech, bool condition,
                                const char *format, ...)
    __attribute__((format(printf, 4, 5)));

static bool template_load_error(FILE *fp, Mech *mech, bool condition,
                                const char *format, ...) {
  if (!condition) {
    return false;
  }
#ifdef TEMPLATE_VERBOSE_ERRORS
  char message[LBUF_SIZE] = {0};
  va_list args;
  va_start(args, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(message, sizeof(message), format, args);
  va_end(args);
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
                     message);
#else
  (void)mech;
  (void)format;
#endif
  if (fp) {
    if (fclose(fp) != 0)
      return true;
  }
  return true;
}

typedef enum TemplateLineResult {
  TEMPLATE_LINE_OK,
  TEMPLATE_LINE_EOF,
  TEMPLATE_LINE_INVALID,
} TemplateLineResult;

static TemplateLineResult template_read_ints(FILE *fp, int *values,
                                             size_t value_count) {
  char line[LBUF_SIZE];
  if (!fgets(line, sizeof(line), fp))
    return feof(fp) ? TEMPLATE_LINE_EOF : TEMPLATE_LINE_INVALID;

  char *token = strtok(line, " \t\r\n");
  for (size_t index = 0; index < value_count; index++) {
    int *value = checked_storage_at(values, value_count * sizeof(*values),
                                    sizeof(*values), index);
    if (!token || !parse_int_checked(token, value))
      return TEMPLATE_LINE_INVALID;
    token = strtok(nullptr, " \t\r\n");
  }
  return token ? TEMPLATE_LINE_INVALID : TEMPLATE_LINE_OK;
}

static TemplateLineResult
template_read_header(FILE *fp, int *tons, int *tactical_range,
                     int *long_range_sensor_range, int *scanner_range,
                     int *sinks, float *maximum_speed, float *jump_speed,
                     int *specials) {
  char line[LBUF_SIZE];
  if (!fgets(line, sizeof(line), fp))
    return feof(fp) ? TEMPLATE_LINE_EOF : TEMPLATE_LINE_INVALID;

  char *token = strtok(line, " \t\r\n");
  if (!token || !parse_int_checked(token, tons))
    return TEMPLATE_LINE_INVALID;
  token = strtok(nullptr, " \t\r\n");
  if (!token || !parse_int_checked(token, tactical_range))
    return TEMPLATE_LINE_INVALID;
  token = strtok(nullptr, " \t\r\n");
  if (!token || !parse_int_checked(token, long_range_sensor_range))
    return TEMPLATE_LINE_INVALID;
  token = strtok(nullptr, " \t\r\n");
  if (!token || !parse_int_checked(token, scanner_range))
    return TEMPLATE_LINE_INVALID;
  token = strtok(nullptr, " \t\r\n");
  if (!token || !parse_int_checked(token, sinks))
    return TEMPLATE_LINE_INVALID;
  token = strtok(nullptr, " \t\r\n");
  if (!token || !parse_float_checked(token, maximum_speed))
    return TEMPLATE_LINE_INVALID;
  token = strtok(nullptr, " \t\r\n");
  if (!token || !parse_float_checked(token, jump_speed))
    return TEMPLATE_LINE_INVALID;
  token = strtok(nullptr, " \t\r\n");
  if (!token || !parse_int_checked(token, specials) ||
      strtok(nullptr, " \t\r\n"))
    return TEMPLATE_LINE_INVALID;
  return TEMPLATE_LINE_OK;
}

static int template_load_legacy(Mech *mech, const char *id) {
  FILE *fp = nullptr;
  int i, j, k, t;
  int i1 = 0, i2 = 0, i3 = 0, i4 = 0, i5 = 0, i6 = 0;
  char *filename;

  filename = mech_template_resolve_path(
      mech_context(mech), btech_context_mech_template_path(mech_context(mech)),
      id);
  if (template_load_error(fp, mech, filename == nullptr,
                          "No matching file for '%s'.", id)) {
    return -1;
  }
  if (filename == nullptr) {
    return -1;
  }
  fp = fopen(filename, "r");
  if (template_load_error(fp, mech, fp == nullptr,
                          "Unable to open file %s (%s)!", filename, id)) {
    return -1;
  }
  if (fp == nullptr) {
    return -1;
  }
  strncpy(((mech)->ud.mech_type), id, 25);
  ((mech)->ud.mech_type)[24] = '\0';
  if (template_load_error(
          fp, mech,
          template_read_header(fp, &i1, &i2, &i3, &i4, &i5,
                               &((mech)->ud.maxspeed), &((mech)->rd.jumpspeed),
                               &i6) != TEMPLATE_LINE_OK,
          "Old template loading system: %s is invalid template file.", id)) {
    return -1;
  }
  ((mech)->ud.tons) = i1;
  mech_tactical_range_set(mech, i2);
  mech_long_range_sensor_range_set(mech, i3);
  mech_scanner_range_set(mech, i4);
  ((mech)->ud.numsinks) = clamp_int_to_char(i5);
  i6 &= ~32768; /* Quad */
  i6 &= ~16384; /* Salvagetech */
  i6 &= ~8192;  /* Cargotech */
  i6 &= ~4196;  /* Watergun */
  ((mech)->rd.specials) = i6;
  for (k = 0; k < NUM_SECTIONS; k++) {
    i = k;
    if (((mech)->ud.type) == 4) {
      switch (k) {
      case 3:
        i = 4;
        break;
      case 4:
        i = 5;
        break;
      case 5:
        i = 3;
        break;
      }
    }
    int section_values[4] = {0};
    if (template_load_error(fp, mech,
                            template_read_ints(fp, section_values,
                                               sizeof(section_values) /
                                                   sizeof(*section_values)) !=
                                TEMPLATE_LINE_OK,
                            "Insufficient data reading section %d!", i)) {
      return -1;
    }
    i1 = section_values[0];
    i2 = section_values[1];
    i3 = section_values[2];
    i4 = section_values[3];
    mech_section_recycle_ticks_set(mech, i, 0);
    mech_section_armor_set(mech, i, i1);
    mech_section_original_armor_set(mech, i, i1);
    mech_section_internal_set(mech, i, i2);
    mech_section_original_internal_set(mech, i, i2);
    mech_section_rear_armor_set(mech, i, i3);
    mech_section_original_rear_armor_set(mech, i, i3);
    /* Remove all rampant AXEs from the arms themselves, we do
       things differently here */
    if (i4 & 4)
      i4 &= ~4;
    mech_section_configuration_set(mech, i, i4);
    for (j = 0; j < NUM_CRITICALS; j++) {
      int critical_values[3] = {0};
      if (template_load_error(
              fp, mech,
              template_read_ints(fp, critical_values,
                                 sizeof(critical_values) /
                                     sizeof(*critical_values)) !=
                  TEMPLATE_LINE_OK,
              "Insufficient data reading critical %d/%d!", i, j)) {
        return -1;
      }
      i1 = critical_values[0];
      i2 = critical_values[1];
      i3 = critical_values[2];
      mech_critical_part_type_set(mech, i, j, i1);
      if (template_load_error(fp, mech,
                              template_part_type_is_invalid(
                                  mech_critical_part_type(mech, i, j)),
                              "Invalid datatype at %d/%d!", i, j)) {
        return -1;
      }
      if (equipment_is_special(i1))
        i1 += SPECIAL_BASE_INDEX - OSPECIAL_BASE_INDEX;
      if (equipment_is_weapon(mech_critical_part_type(mech, i, j))) {
        t = weapon_from_equipment_index(mech_critical_part_type(mech, i, j));
        if (weapon_catalogue_is_anti_missile(t)) {
          if (weapon_catalogue_has_special(t, CLAT))
            ((mech)->rd.specials) |= CL_ANTI_MISSILE_TECH;
          else
            ((mech)->rd.specials) |= IS_ANTI_MISSILE_TECH;
        }
      }
      mech_critical_data_set(mech, i, j, i2);
      mech_critical_fire_mode_set(mech, i, j, i3);
    }
  }
  int type_values[2];
  TemplateLineResult type_result = template_read_ints(
      fp, type_values, sizeof(type_values) / sizeof(*type_values));
  if (type_result == TEMPLATE_LINE_INVALID) {
    if (template_load_error(fp, mech, true,
                            "Invalid 'mech or movement type!")) {
      return -1;
    }
  }
  if (type_result == TEMPLATE_LINE_OK) {
    i1 = type_values[0];
    i2 = type_values[1];
    if (template_load_error(fp, mech, i1 > CLASS_LAST, "Invalid 'mech type!")) {
      return -1;
    }
    ((mech)->ud.type) = clamp_int_to_char(i1);
    if (template_load_error(fp, mech, i2 > MOVENEMENT_LAST,
                            "Invalid movenement type!")) {
      return -1;
    }
    ((mech)->ud.move) = clamp_int_to_char(i2);
  }
  int radio_values[1];
  TemplateLineResult radio_result = template_read_ints(
      fp, radio_values, sizeof(radio_values) / sizeof(*radio_values));
  if (radio_result == TEMPLATE_LINE_INVALID) {
    if (template_load_error(fp, mech, true, "Invalid radio range!"))
      return -1;
  }
  if (radio_result == TEMPLATE_LINE_EOF)
    mech_radio_range_set(mech, DEFAULT_RADIORANGE);
  else
    mech_radio_range_set(mech, radio_values[0]);
  if (fclose(fp) != 0)
    return -1;
  return 1;
}

#undef LOADNEW_LOADS_OLD_IF_FAIL
#define LOADNEW_LOADS_MUSE_FORMAT

int mech_template_load(DbRef player, Mech *mech, const char *id) {
  char mech_origid[100];

  strncpy(mech_origid, ((mech)->ud.mech_type), 99);
  mech_origid[99] = '\0';

  if (!strcmp(mech_origid, id)) {
    mech_template_clear(mech, 0);
    if (template_load_modern(player, mech, id) <= 0)
      return template_load_legacy(mech, id) > 0;
    return 1;
  }
  mech_template_clear(mech, 1);
  if (template_load_modern(player, mech, id) < 1)
#ifdef LOADNEW_LOADS_MUSE_FORMAT
    if (template_load_legacy(mech, id) < 1)
#endif
#ifdef LOADNEW_LOADS_OLD_IF_FAIL
      if (template_load_modern(player, mech, mech_origid) < 1)
#ifdef LOADNEW_LOADS_MUSE_FORMAT
        if (template_load_legacy(mech, mech_origid) < 1)
#endif
#endif
          return 0;

  return 1;
}
