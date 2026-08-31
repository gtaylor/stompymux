/* Implements BattleTech sensor mechanics for unit contacts. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "map_building_query_api.h"
#include "map_coordinates.h"
#include "map_los_types.h"
#include "map_object_query_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_contacts_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/access.h"
#include "registry_api.h"

static const char DEFAULT_CONTACTOPTIONS[] = "!db";
static constexpr size_t CONTACT_OPTIONS_LENGTH_LIMIT = 49;

static bool mech_contact_is_friend(Mech *observer, Mech *target) {
  return (mech_team(observer) == mech_team(target) &&
          mech_los_check_unblocked(observer, target, 0, 0, 0)) != 0;
}

static int mech_contact_heading(const Mech *mech) {
  return acceptable_degree(mech_heading_degrees(mech) +
                           mech_lateral_movement(mech));
}

static bool mech_contact_carries_club(const Mech *mech) {
  return (mech_section_carries_club(mech, RARM) ||
          mech_section_carries_club(mech, LARM)) != 0;
}

static const char *const AC_DESC[] = {
    "0 - See enemies and friends, long text, color",
    "1 - See enemies and friends, short text, color",
    "2 - See enemies only, long text, color",
    "3 - See enemies only, short text, color",
    "4 - See enemies and friends, short text, no color",
    "5 - See enemies only, short text, no color",

    "6 - Disabled"};

static const char *const C_DESC[] = {
    "0 - Very verbose", "1 - Short form, the usual one",
    "2 - Short form, the usual one, but do not see buildings",
    "3 - Shorter form"};

typedef struct ContactLine {
  float sort_range;
  char text[120];
} ContactLine;

static const char *contact_description(const char *const *descriptions,
                                       size_t count, int index) {
  return *(const char *const *)checked_storage_at_const(
      (const void *)descriptions, count, sizeof(*descriptions), (size_t)index);
}

static ContactLine *contact_line(ContactLine *lines, int index) {
  return checked_storage_at(lines, BATTLE_MAP_UNIT_CAPACITY, sizeof(*lines),
                            (size_t)index);
}

static void status_string_append(MechStatusString *status, size_t *length,
                                 char value) {
  *(char *)checked_storage_at(status->text, sizeof(status->text), sizeof(char),
                              *length) = value;
  ++*length;
}

void show_brief_flags(DbRef player, Mech *mech) {
  notify_printf(
      btech_context_evaluation(mech_context(mech)), player,
      "Brief status for %s:", mech_to_mech_display_id(mech, mech).text);
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "    (A)utocontacts: %s",
                contact_description(AC_DESC, sizeof(AC_DESC) / sizeof(*AC_DESC),
                                    mech_brief_mode(mech) / 4));
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "    (C)ontacts:     %s",
                contact_description(C_DESC, sizeof(C_DESC) / sizeof(*C_DESC),
                                    mech_brief_mode(mech) % 4));
}

void mech_brief(DbRef player, Mech *mech, char *buffer) {
  char c;
  int v;

  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  if (buffer != nullptr)
    buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(char),
                                strspn(buffer, " \t\r\n\f\v"));
  if (!buffer || !*buffer) {
    show_brief_flags(player, mech);
    return;
  }
  c = *buffer;
  buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(char), 1);
  buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(char),
                              strspn(buffer, " \t\r\n\f\v"));
  if (!buffer || !*buffer) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Argument missing!");
    return;
  }
  if (!parse_int_checked(buffer, &v)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number!");
    return;
  }
  switch (ascii_to_upper(c)) {
  case 'A':
    if (v < 0 || v > 6) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Number out of range!");
      return;
    }
    v = bounded(0, v, 6);
    mech_brief_mode_set(mech, (mech_brief_mode(mech) % 4) + (v * 4));
    mech_printf(
        mech, MECHALL, "Autocontact brevity set to %s.",
        contact_description(AC_DESC, sizeof(AC_DESC) / sizeof(*AC_DESC), v));
    return;
  case 'C':
    if (v < 0 || v > 3) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Number out of range!");
      return;
    }
    v = bounded(0, v, 3);
    mech_brief_mode_set(mech, ((mech_brief_mode(mech) / 4) * 4) + v);
    mech_printf(
        mech, MECHALL, "Contact brevity set to %s.",
        contact_description(C_DESC, sizeof(C_DESC) / sizeof(*C_DESC), v));
    return;
  }
}

enum ContactVisibility : int {
  SEE_DEAD = 0x01,
  SEE_SHUTDOWN = 0x02,
  SEE_ALLY = 0x04,
  SEE_ENEMA = 0x08,
  SEE_TARGET = 0x10,
  SEE_BUILDINGS = 0x20,
  SEE_NEGNEXT = 0x80,
};

char mech_contact_weapon_arc(int arc) {
  if (arc & FORWARDARC)
    return '*';
  if (arc & TURRETARC)
    return 't';
  if (arc & RSIDEARC)
    return 'r';
  if (arc & LSIDEARC)
    return 'l';
  if (arc & REARARC)
    return 'v';
  return '?';
}

/* who: 0 for friend, 1 for enemy, 2 for 'self' */
MechStatusString mech_status_string(Mech *target, int who) {
  MechStatusString status = {0};
  size_t sptr = 0;

  const MechConditionSummary CONDITION = mech_condition_summary(target);

  if (mech_is_destroyed(target))
    status_string_append(&status, &sptr, 'D');

  if (mech_event_count(target, EVENT_STARTUP))
    status_string_append(&status, &sptr, 's');
  else if (!mech_is_started(target))
    status_string_append(&status, &sptr, 'S');

  if (mech_event_count(target, EVENT_STAND))
    status_string_append(&status, &sptr, 'f');
  else if (mech_is_fallen(target))
    status_string_append(&status, &sptr, 'F');

  if (mech_event_count(target, EVENT_CHANGING_HULLDOWN))
    status_string_append(&status, &sptr, 'h');
  else if (CONDITION.hull_down)
    status_string_append(&status, &sptr, 'H');

  if (mech_is_towed(target))
    status_string_append(&status, &sptr, 'T');
  else if (mech_carried_dbref(target) > 0)
    status_string_append(&status, &sptr, 't');

  if (mech_is_jumping(target))
    status_string_append(&status, &sptr, 'J');

  if (mech_is_out_of_control(target))
    status_string_append(&status, &sptr, 'O');

  if (mech_excess_heat(target) != 0.0F)
    status_string_append(&status, &sptr, '+');

  if (mech_is_jellied(target))
    status_string_append(&status, &sptr, 'I');

  if (mech_event_count(target, EVENT_VEHICLEBURN))
    status_string_append(&status, &sptr, 'B');

  if (mech_searchlight_active(target))
    status_string_append(&status, &sptr, 'L');

  if (CONDITION.illuminated)
    status_string_append(&status, &sptr, 'l');

  if (CONDITION.swarm_target > 0)
    status_string_append(&status, &sptr, 'W');

  if (mech_contact_carries_club(target))
    status_string_append(&status, &sptr, 'C');

  if (mech_has_attached_homing_beacon(target)) {
    if (who == 1)
      status_string_append(&status, &sptr, 'N');
    else
      status_string_append(&status, &sptr, 'n');
  }
  if (CONDITION.eccm_enabled || CONDITION.angel_eccm_enabled)
    status_string_append(&status, &sptr, 'P');

  if (CONDITION.ecm_active || CONDITION.angel_ecm_active)
    status_string_append(&status, &sptr, 'E');

  if (CONDITION.ecm_protected || CONDITION.angel_ecm_protected)
    status_string_append(&status, &sptr, 'p');

  if (mech_is_any_ecm_disturbed(target))
    status_string_append(&status, &sptr, 'e');

  if (CONDITION.spinning)
    status_string_append(&status, &sptr, 'X');

  status_string_append(&status, &sptr, '\0');
  return status;
}

char mech_contact_status_character(Mech *mech, Mech *mech_target,
                                   int w_char_num) {
  const MechConditionSummary CONDITION = mech_condition_summary(mech_target);

  switch (w_char_num) {
  case 1:
    if (CONDITION.swarm_target > 0)
      return 'W';
    if (mech_is_towed(mech_target))
      return 'T';
    if (mech_carried_dbref(mech_target) > 0)
      return 't';
    if (mech_contact_carries_club(mech_target))
      return 'C';
    return ' ';
  case 2:
    if (mech_is_destroyed(mech_target))
      return 'D';
    if (mech_searchlight_active(mech_target))
      return 'L';
    return CONDITION.illuminated ? 'l' : ' ';
  case 3:
    if (mech_is_jumping(mech_target))
      return 'J';
    if (mech_is_out_of_control(mech_target))
      return 'O';
    if (mech_is_fallen(mech_target))
      return 'F';
    if (mech_event_count(mech_target, EVENT_STAND))
      return 'f';
    if (mech_event_count(mech_target, EVENT_CHANGING_HULLDOWN))
      return 'h';
    if (CONDITION.hull_down)
      return 'H';
    return mech_condition_summary(mech).spinning ? 'X' : ' ';
  case 4:
    if (mech_is_started(mech_target)) {
      if (mech_excess_heat(mech_target) != 0.0F)
        return '+';
      if (mech_is_jellied(mech_target))
        return 'I';
      return mech_event_count(mech_target, EVENT_VEHICLEBURN) ? 'B' : ' ';
    }
    if (CONDITION.staggering)
      return 'G';
    return mech_event_count(mech_target, EVENT_STARTUP) ? 's' : 'S';
  case 5:
    if (mech_has_attached_homing_beacon(mech_target))
      return mech_team(mech_target) == mech_team(mech) ? 'n' : 'N';
    if (CONDITION.eccm_enabled || CONDITION.angel_eccm_enabled)
      return 'P';
    if (CONDITION.ecm_active || CONDITION.angel_ecm_active)
      return 'E';
    if (CONDITION.ecm_protected || CONDITION.angel_ecm_protected)
      return 'p';
    if (mech_is_any_ecm_disturbed(mech_target))
      return 'e';
    return ' ';
  }
  return ' ';
}

void mech_contacts(DbRef player, Mech *mech, char *buffer) {
  Mech *temp_mech;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  BattleMap *tmp_map;
  MapObject *building;
  int loop;
  int i;
  int j;
  int argc;
  int bearing;
  int buffindex = 0;
  char *args[1];
  char buff[100];
  ContactLine *contacts;
  float range;
  float fx;
  float fy;
  char weaponarc;
  const char *mech_name;
  unsigned char see_what;
  char *str;
  char move_type[30];
  char c_status1;
  char c_status2;
  char c_status3;
  char c_status4;
  char c_status5;
  int losflag;
  int isvb;
  int inlos;
  char *new;
  char *attribute_buffer;
  LuaLockInvocation lock;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  contacts = checked_storage_allocate_array(BATTLE_MAP_UNIT_CAPACITY,
                                            sizeof(*contacts));
  attribute_buffer = alloc_lbuf("mech_contacts.attribute");
  new = alloc_lbuf("mech_contacts.new");
  LuaLockResult *lock_result = checked_storage_allocate(sizeof(*lock_result));
  argc = mech_parseattributes(buffer, args, 1);

  isvb = (mech_brief_mode(mech) % 4);
  if (argc > 0) {
    char *argument =
        *(char **)checked_storage_at((void *)args, 1, sizeof(*args), 0);
    if (*argument == '+') {
      str = btech_attribute_read(mech_context(mech)->database, player,
                                 A_CONTACTOPT, attribute_buffer);
      if (!*str) {
        (void)string_copy_bounded(buff, sizeof(buff), DEFAULT_CONTACTOPTIONS);
      } else {
        (void)string_copy_bounded(buff, CONTACT_OPTIONS_LENGTH_LIMIT + 1, str);

        if (strlen(buff) == 0)
          (void)string_copy_bounded(buff, sizeof(buff), DEFAULT_CONTACTOPTIONS);
      }
    } else {
      (void)string_copy_bounded(buff, CONTACT_OPTIONS_LENGTH_LIMIT + 1,
                                argument);
    }

    if (isvb == 1)
      see_what = SEE_BUILDINGS;
    else
      see_what = 0x0;

    for (loop = 0; loop < 50; loop++) {
      const char C = *checked_string_suffix(buff, (size_t)loop);
      if (C == '\0')
        break;

      if (C == 'd') {
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_DEAD)
                                 : (see_what |= SEE_DEAD);
      } else if (C == 's') {
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_SHUTDOWN)
                                 : (see_what |= SEE_SHUTDOWN);
      } else if (C == 'b') {
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_BUILDINGS)
                                 : (see_what |= SEE_BUILDINGS);
      } else if (C == 'e') {
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_ENEMA)
                                 : (see_what |= SEE_ENEMA);
      } else if (C == 'a') {
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_ALLY)
                                 : (see_what |= SEE_ALLY);
      } else if (C == 't') {
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_TARGET)
                                 : (see_what |= SEE_TARGET);
      } else if (C == '!') {
        see_what = (SEE_NEGNEXT | SEE_DEAD | SEE_SHUTDOWN | SEE_ENEMA |
                    SEE_ALLY | SEE_TARGET);
      } else {
        notify_printf(btech_context_evaluation(mech_context(mech)), player,
                      "Ignoring %c as contact option.", C);
      }
    }
  } else {
    see_what = (SEE_DEAD | SEE_SHUTDOWN | SEE_ENEMA | SEE_ALLY | SEE_TARGET);
    if (isvb == 1)
      see_what |= SEE_BUILDINGS;
  }

  if (isvb <= 2)
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Line of Sight Contacts:");

  for (loop = 0; loop < battle_map_unit_count(mech_map); loop++) {
    const DbRef CONTACT_DBREF = battle_map_unit_dbref(mech_map, loop);
    if (!(CONTACT_DBREF != mech_dbref(mech) && CONTACT_DBREF != -1))
      continue;

    temp_mech = btech_context_get_mech(mech_context(mech), CONTACT_DBREF);

    if (!temp_mech)
      continue;
    if (argc) {
      if (!((mech_contact_is_friend(mech, temp_mech)
                 ? (see_what & SEE_ALLY)
                 : (see_what & SEE_ENEMA)) ||
            ((see_what & SEE_TARGET) &&
             (mech_dbref(temp_mech) == mech_target_dbref(mech)))))
        continue;
      if (!(((see_what & SEE_SHUTDOWN) || mech_is_started(temp_mech)) ||
            mech_is_destroyed(temp_mech) ||
            ((see_what & SEE_TARGET) &&
             (mech_dbref(temp_mech) == mech_target_dbref(mech)))))
        continue;
      if (!(((see_what & SEE_DEAD) || !mech_is_destroyed(temp_mech)) ||
            ((see_what & SEE_TARGET) &&
             (mech_dbref(temp_mech) == mech_target_dbref(mech)))))
        continue;
    }
    range = mech_range_to(mech, temp_mech);
    losflag = mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                             mech_position_y(temp_mech), range);
    if (!losflag)
      continue;
    if (is_good_obj(mech_context(mech)->database, mech_dbref(temp_mech))) {
      if (!mech_los_check_unblocked(mech, temp_mech, mech_position_x(temp_mech),
                                    mech_position_y(temp_mech), 0.0)) {
        mech_name = "something";
        inlos = 0;
      } else {
        mech_name = btech_attribute_read(mech_context(temp_mech)->database,
                                         mech_dbref(temp_mech), A_MECHNAME,
                                         attribute_buffer);
        inlos = 1;
      }
    } else {
      continue;
    }
    bearing = map_bearing(
        &(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                    .y = mech_position_real_y(mech)},
                          .end = {.x = mech_position_real_x(temp_mech),
                                  .y = mech_position_real_y(temp_mech)}});
    weaponarc = mech_contact_weapon_arc(
        in_weapon_arc(mech, mech_position_real_x(temp_mech),
                      mech_position_real_y(temp_mech)));

    (void)string_copy_bounded(move_type, sizeof(move_type),
                              get_move_type_id(mech_movement_type(temp_mech)));

    if (isvb) {
      if (!inlos) {
        c_status1 = ' ';
        c_status2 = ' ';
        c_status3 = ' ';
        c_status4 = ' ';
        c_status5 = ' ';
      } else {
        c_status1 = mech_contact_status_character(mech, temp_mech, 1);
        c_status2 = mech_contact_status_character(mech, temp_mech, 2);
        c_status3 = mech_contact_status_character(mech, temp_mech, 3);
        c_status4 = mech_contact_status_character(mech, temp_mech, 4);
        c_status5 = mech_contact_status_character(mech, temp_mech, 5);
      }

      const char *contact_color = "";
      if (mech_dbref(temp_mech) == mech_target_dbref(mech))
        contact_color = "[fg=red bold]";
      else if (!mech_contact_is_friend(mech, temp_mech))
        contact_color = "[fg=yellow bold]";
      (void)snprintf(
          buff, sizeof(buff),
          "%s%c%c%c[%s]%c %-12.12s x:%3d y:%3d z:%3d r:%4.1f b:%3d "
          "s:%5.1f h:%3d S:%c%c%c%c%c%s",
          contact_color, (losflag & BATTLE_MAP_LOS_SEEN_PRIMARY) ? 'P' : ' ',
          (losflag & BATTLE_MAP_LOS_SEEN_SECONDARY) ? 'S' : ' ', weaponarc,
          mech_id(temp_mech, mech_contact_is_friend(mech, temp_mech)).text,
          *move_type, mech_name, mech_position_x(temp_mech),
          mech_position_y(temp_mech), mech_position_z(temp_mech), (double)range,
          bearing, (double)mech_current_speed(temp_mech),
          mech_contact_heading(temp_mech), c_status1, c_status2, c_status3,
          c_status4, c_status5,
          (mech_dbref(temp_mech) == mech_target_dbref(mech) ||
           !mech_contact_is_friend(mech, temp_mech))
              ? "[reset]"
              : "");

      if (buffindex < BATTLE_MAP_UNIT_CAPACITY) {
        ContactLine *contact = contact_line(contacts, buffindex++);
        contact->sort_range =
            range + (mech_is_destroyed(temp_mech) ? 10000.0F : 0.0F);
        (void)snprintf(contact->text, sizeof(contact->text), "%s", buff);
      }
    } else {
      (void)snprintf(
          buff, sizeof(buff), "[%s] %-17s  Tonnage: %d",
          mech_id(temp_mech, mech_contact_is_friend(mech, temp_mech)).text,
          mech_name, mech_tonnage(temp_mech));
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      (void)snprintf(buff, sizeof(buff),
                     "      Range: %.1f hex\tBearing: %d degrees",
                     (double)range, bearing);
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      (void)snprintf(buff, sizeof(buff),
                     "      Speed: %.1f KPH\tHeading: %d degrees",
                     (double)mech_current_speed(temp_mech),
                     mech_contact_heading(temp_mech));
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      (void)snprintf(buff, sizeof(buff),
                     "      X, Y: %3d, %3d \tHeat: %.0f deg C.",
                     mech_position_x(temp_mech), mech_position_y(temp_mech),
                     (double)mech_excess_heat(temp_mech));
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      (void)snprintf(buff, sizeof(buff), "      Movement Type: %s", move_type);
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      notify_printf(
          btech_context_evaluation(mech_context(mech)), player,
          "      Mech is in %s Arc",
          get_arc_id(mech, in_weapon_arc(mech, mech_position_real_x(temp_mech),
                                         mech_position_real_y(temp_mech))));
      if (mech_is_destroyed(temp_mech))
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "      Mech Destroyed");
      if (!mech_is_started(temp_mech))
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "      Mech Shutdown");
      if (mech_is_fallen(temp_mech))
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "      Mech has Fallen!");
      if (mech_is_jumping(temp_mech))
        notify_printf(btech_context_evaluation(mech_context(mech)), player,
                      "      Mech is Jumping!\tJump Heading: %d",
                      mech_jump_heading_degrees(temp_mech));
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, " ");
    }
  }

  if (see_what & SEE_BUILDINGS) {
    for (building =
             battle_map_object_first(mech_map, BATTLE_MAP_OBJECT_BUILDING);
         building; building = battle_map_object_next(building)) {
      const int BUILDING_X = battle_map_object_x(building);
      const int BUILDING_Y = battle_map_object_y(building);
      const DbRef BUILDING_DBREF = battle_map_object_dbref(building);

      map_coord_to_real_coord(BUILDING_X, BUILDING_Y, &fx, &fy);
      const int BUILDING_ELEVATION =
          battle_map_hex_elevation(mech_map, BUILDING_X, BUILDING_Y);
      i = BUILDING_ELEVATION + 1;
      const float BUILDING_REAL_Z = ZSCALE * (float)i;
      range = map_spatial_range(&(MapSpatialSegment){
          .start = {.x = mech_position_real_x(mech),
                    .y = mech_position_real_y(mech),
                    .z = mech_position_real_z(mech)},
          .end = {.x = fx, .y = fy, .z = BUILDING_REAL_Z},
      });

      losflag = mech_los_check(mech, nullptr, BUILDING_X, BUILDING_Y, range);
      if (!losflag || (losflag & BATTLE_MAP_LOS_BLOCKED))
        continue;

      if (!BUILDING_DBREF)
        continue;
      tmp_map = btech_context_get_map(mech_context(mech), BUILDING_DBREF);
      if (!tmp_map)
        continue;
      if (battle_map_building_is_invisible(tmp_map))
        continue;
      j = !lock_test(btech_context_evaluation(mech_context(mech)), player,
                     player, mech_dbref(mech), battle_map_dbref(tmp_map),
                     LUA_LOCK_IDENTIFY_BUILDING, true, &lock, lock_result);
      if (j && battle_map_building_is_hidden(tmp_map))
        continue;
      bearing = map_bearing(
          &(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                      .y = mech_position_real_y(mech)},
                            .end = {.x = fx, .y = fy}});
      weaponarc = mech_contact_weapon_arc(in_weapon_arc(mech, fx, fy));

      mech_name =
          btech_attribute_read(mech_context(mech)->database, BUILDING_DBREF,
                               A_MECHNAME, attribute_buffer);
      if (!mech_name || !*mech_name) {
        styled_text_strip(
            mech_context(mech)->database->styled_text_palette,
            game_object_name(mech_context(mech)->database, BUILDING_DBREF), new,
            sizeof(new));
        mech_name = new;
      }

      char building_status = ' ';
      if (battle_map_building_is_safe(tmp_map) ||
          (j && battle_map_building_is_command_center(tmp_map)))
        building_status = 'X';
      else if (j)
        building_status = 'x';
      else if (battle_map_building_is_command_center(tmp_map))
        building_status = 'C';
      (void)snprintf(
          buff, sizeof(buff),
          "%s%c%c%c %-23.23s x:%3d y:%3d z:%2d r:%4.1f b:%3d CF:%4d /%4d "
          "S:%c%c%s",
          j ? "[fg=yellow bold]" : "",
          (losflag & BATTLE_MAP_LOS_SEEN_PRIMARY) ? 'P' : ' ',
          (losflag & BATTLE_MAP_LOS_SEEN_SECONDARY) ? 'S' : ' ', weaponarc,
          mech_name, BUILDING_X, BUILDING_Y, i, (double)range, bearing,
          battle_map_building_integrity(tmp_map),
          battle_map_building_maximum_integrity(tmp_map), building_status,
          battle_map_building_is_hidden(tmp_map) ? 'H' : ' ',
          j ? "[reset]" : "");
      if (buffindex < BATTLE_MAP_UNIT_CAPACITY) {
        ContactLine *contact = contact_line(contacts, buffindex++);
        contact->sort_range = range + 20000.0F;
        (void)snprintf(contact->text, sizeof(contact->text), "%s", buff);
      }
    }
  }

  if (isvb) {
    /* print a sorted list of detected mechs */
    /* use the ever-popular bubble sort */
    for (i = 0; i < (buffindex - 1); i++) {
      for (j = (i + 1); j < buffindex; j++) {
        if (contact_line(contacts, j)->sort_range >
            contact_line(contacts, i)->sort_range) {
          ContactLine temporary = *contact_line(contacts, i);
          *contact_line(contacts, i) = *contact_line(contacts, j);
          *contact_line(contacts, j) = temporary;
        }
      }
    }
    for (loop = 0; loop < buffindex; loop++)
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   contact_line(contacts, loop)->text);
  }

  if (isvb <= 2)
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "End Contact List");
  free_buf(attribute_buffer);
  free_buf(new);
  free_buf(lock_result);
  free(contacts);
}
