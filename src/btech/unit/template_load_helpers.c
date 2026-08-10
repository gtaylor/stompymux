/* Implements reusable template parsing helpers. */

#include "btconfig.h"
#include "template_load_internal.h"

#include "btech/context.h"
#include "btech_channel.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "mech_consistency_api.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"
#include <stdarg.h>
#include <stdio.h>
#include <string.h>

bool template_load_error(FILE *fp, Mech *mech, DbRef player, bool condition,
                         bool global, const char *format, ...) {
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
bool template_read_int(FILE *fp, Mech *mech, DbRef player, char *text,
                       int *value) {
  if (parse_int_checked(text, value))
    return true;
  template_load_error(fp, mech, player, true, true,
                      "Error while loading: Invalid integer value '%s'.", text);
  return false;
}
bool template_read_float(FILE *fp, Mech *mech, DbRef player, char *text,
                         float *value) {
  if (parse_float_checked(text, value))
    return true;
  template_load_error(fp, mech, player, true, true,
                      "Error while loading: Invalid decimal value '%s'.", text);
  return false;
}
bool template_parse_critical_range(char *command, int *first, int *last) {
  char *range = checked_mutable_string_suffix(command, 5);
  char *separator = strchr(range, '-');
  if (separator != nullptr) {
    *separator = '\0';
    if (!parse_int_checked(range, first) ||
        !parse_int_checked(checked_string_suffix(separator, 1), last)) {
      return false;
    }
  } else if (!parse_int_checked(range, first)) {
    return false;
  } else {
    *last = *first;
  }
  return *first > 0 && *last >= *first;
}

void template_load_finalize(Mech *mech, bool clan_equipment) {
  mech->rd.erat = mech_calculated_engine_rating(mech);
  if (strlen(mech->ud.unit_era) == 0)
    strcpy(mech->ud.unit_era, "Undefined");
  if (strlen(mech->ud.unit_tro) == 0)
    strcpy(mech->ud.unit_tro, "Undefined");

  if (!(mech->rd.specials & ICE_TECH) && !mech->ud.numsinks)
    mech->ud.numsinks = DEFAULT_HEATSINKS;
  if (mech->ud.type == CLASS_MECH)
    do_sub_magic(mech, 1);
  if (mech->ud.type == CLASS_MW)
    mech_power_up(mech);

  const int section_count = mech->ud.type == CLASS_MECH ? 8 : 6;
  if (mech->xcode.context->configuration->btech_parts) {
    for (int section = 0; section < section_count; ++section) {
      for (int critical = 0; critical < CritsInLoc(mech, section); ++critical) {
        const int part = mech_critical_part_type(mech, section, critical);
        if (part == 0 || mech_critical_brand(mech, section, critical) ||
            equipment_is_ammunition(part) || equipment_is_bomb(part))
          continue;
        mech_critical_brand_set(&(CriticalSlotBrandSet){
            .mech = mech,
            .slot = {.section = section, .critical = critical},
            .brand = clan_equipment ? DEFAULT_CLPART_LEVEL : DEFAULT_PART_LEVEL,
        });
      }
    }
  }

  if (!mech_computer_quality(mech))
    mech_computer_quality_set(mech, clan_equipment ? DEFAULT_CLCOMPUTER
                                                   : DEFAULT_COMPUTER);
  if (!mech_radio_quality(mech))
    mech_radio_quality_set(mech,
                           clan_equipment ? DEFAULT_CLRADIO : DEFAULT_RADIO);
  if (!mech_radio_configuration(mech))
    mech_radio_configuration_set(
        mech, generic_radio_type(mech_radio_quality(mech), clan_equipment));

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

  mech->rd.specials &= ~FLIPABLE_ARMS;
  if (mech->ud.type == CLASS_MECH &&
      mech_critical_part_type(mech, LARM, 2) !=
          special_equipment_index(LOWER_ACTUATOR) &&
      mech_critical_part_type(mech, RARM, 2) !=
          special_equipment_index(LOWER_ACTUATOR) &&
      mech_critical_part_type(mech, LARM, 3) !=
          special_equipment_index(HAND_OR_FOOT_ACTUATOR) &&
      mech_critical_part_type(mech, RARM, 3) !=
          special_equipment_index(HAND_OR_FOOT_ACTUATOR))
    mech->rd.specials |= FLIPABLE_ARMS;

  update_specials(mech);
  mech->rd.xpmod = 1.0;
  mech->rd.units_killed = 0;
  mech_int_check(mech, 1);
  const int weight = mech_weight_sub(GOD, mech, 0);
  const int frame_weight = mech->ud.tons * 1024;
  if (weight - frame_weight > 40 && mech->ud.type != CLASS_BSUIT &&
      mech->ud.move != MOVE_NONE)
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Error in %s template: %.1f tons of 'stuff', yet %d ton frame.",
                mech->ud.mech_type, weight / 1024.0, frame_weight / 1024));
  update_oweight(mech, weight);

  BattleMap *map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  if (map != nullptr)
    map_conditions_apply(mech, map);
}
