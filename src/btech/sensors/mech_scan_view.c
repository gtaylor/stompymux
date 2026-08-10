#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_scan_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static unsigned char scan_weapon_byte(const unsigned char *values, int index) {
  return *(const unsigned char *)checked_storage_at_const(
      values, MAX_WEAPS_SECTION, sizeof(*values), (size_t)index);
}

static int scan_weapon_integer(const int *values, int index) {
  return *(const int *)checked_storage_at_const(values, MAX_WEAPS_SECTION,
                                                sizeof(*values), (size_t)index);
}

void PrintEnemyWeaponStatus(Mech *mech, DbRef player) {
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int count;
  int loop;
  int ii;
  char weapbuff[LBUF_SIZE] = {0};
  char tempbuff[50] = {0};
  char location[20] = {0};
  int running_sum = 0;

  mech_weapon_recycle_update(mech);
  mecha_notify(evaluation, player,
               "================WEAPON SYSTEMS================");
  if (mech_class(mech) == CLASS_BSUIT)
    mecha_notify(evaluation, player,
                 "----- Weapon ------ [##]  Holder ------ Status");
  else
    mecha_notify(evaluation, player,
                 "----- Weapon ------ [##]  Location ---- Status");
  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    if (mech_section_is_destroyed(mech, loop))
      continue;
    count = FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 1);
    if (count > 0) {
      ArmorStringFromIndex(loop, tempbuff, mech_class(mech),
                           mech_movement_type(mech));
      (void)snprintf(location, sizeof(location), "%-14.14s", tempbuff);

      for (ii = 0; ii < count; ii++) {
        const int weapon_index = scan_weapon_byte(weaparray, ii);
        (void)snprintf(
            weapbuff, sizeof(weapbuff), " %-18.18s [%2d]  ",
            checked_string_suffix(weapon_catalogue_name(weapon_index), 3),
            running_sum + ii);
        strlcat(weapbuff, location, sizeof(weapbuff));

        if (mech_critical_is_nonfunctional(mech, loop,
                                           scan_weapon_integer(critical, ii))) {
          strlcat(weapbuff, "[fg=black bold]*****[reset]", sizeof(weapbuff));
        } else {
          if (scan_weapon_byte(weapdata, ii)) {
            strlcat(weapbuff, "-----", sizeof(weapbuff));
          } else {
            strlcat(weapbuff, "[fg=green]Ready[reset]", sizeof(weapbuff));
          }
        }
        mecha_notify(evaluation, player, weapbuff);
      }
      running_sum += count;
    }
  }
}

void mech_sight(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[5];
  int argc;
  int weapnum;

  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!common_checks(player, mech, MECH_USUAL))
    return;
  argc = mech_parseattributes(buffer, args, 5);
  if (argc >= 1) {
    if (!parse_int_checked(args[0], &weapnum)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid weapon number!");
      return;
    }
    mech_weapon_fire_command(
        &(WeaponFireCommandRequest){.actor = player,
                                    .mech = mech,
                                    .map = mech_map,
                                    .weapon_number = weapnum,
                                    .argument_count = argc,
                                    .arguments = args,
                                    .sight = true});
  } else {
    mecha_notify(evaluation, player, "Not enough arguments to the function");
  }
}

void mech_view(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  DbRef targetnum;
  char targetID[5];
  char *args[5];
  int argc;
  char *target_desc;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  argc = mech_parseattributes(buffer, args, 2);
  if (argc == 0) { /* default target */
    if (mech_target_dbref(mech) == -1) {
      mech_notify(mech, MECHALL, "You do not have a default target set!");
      return;
    }
    target =
        btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));
    if (!target) {
      mech_notify(mech, MECHALL, "Invalid default target!");
      mech_targeting_target_clear(mech);
      return;
    }
    if (!mech_los_check_unblocked(mech, target, mech_position_x(target),
                                  mech_position_y(target),
                                  mech_range_to(mech, target))) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "That target isn't seen well enough by the scannfers for viewing!");
      return;
    }
    target_desc =
        btech_attribute_read(mech_context(target)->database, mech_dbref(target),
                             A_MECHDESC, (char[LBUF_SIZE]){0});
    if (*target_desc)
      mecha_notify(evaluation, player, target_desc);
    else
      mecha_notify(evaluation, player, "That target has no markings.");
  } else if (argc == 1) { /* ID number */
    targetID[0] = args[0][0];
    targetID[1] = *checked_string_suffix(*args, 1);
    targetnum = FindTargetDBREFFromMapNumber(mech, targetID);
    if (targetnum == -1) {
      mech_notify(mech, MECHPILOT, "Target is not in line of sight!");
      return;
    }
    target = btech_context_get_mech(mech_context(mech), targetnum);

    if (!target ||
        !mech_los_check(mech, target, mech_position_x(target),
                        mech_position_y(target), mech_range_to(mech, target))) {
      mech_notify(mech, MECHPILOT, "Target is not in line of sight!");
      return;
    }

    if (!mech_los_check_unblocked(mech, target, mech_position_x(target),
                                  mech_position_y(target),
                                  mech_range_to(mech, target))) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "That target isn't seen well enough by the scanners for viewing!");
      return;
    }

    target_desc =
        btech_attribute_read(mech_context(target)->database, mech_dbref(target),
                             A_MECHDESC, (char[LBUF_SIZE]){0});
    if (*target_desc)
      mecha_notify(evaluation, player, target_desc);
    else
      mecha_notify(evaluation, player, "That target has no markings.");
  } else
    mecha_notify(evaluation, player,
                 "Invalid number of arguments to function.");
}
