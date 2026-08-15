
/* Implements dynamic environmental conditions for maps. */

#include "btech/context.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_runtime_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

bool map_read_dimensions(FILE *file, int *width, int *height) {
  char line[64];
  char *token_context = nullptr;
  if (fgets(line, sizeof(line), file) == nullptr)
    return false;

  char *width_text = strtok_r(line, " \t\r\n", &token_context);
  char *height_text = strtok_r(nullptr, " \t\r\n", &token_context);
  return (width_text != nullptr && height_text != nullptr &&
          strtok_r(nullptr, " \t\r\n", &token_context) == nullptr &&
          parse_int_checked(width_text, width) &&
          parse_int_checked(height_text, height)) != 0;
}

bool map_parse_visibility_attribute(const char *attribute, int *visibility,
                                    int *light, int *wind_direction,
                                    int *wind_speed, int *cloud_base,
                                    char *message, size_t message_size) {
  char values[LBUF_SIZE];
  char *token_context = nullptr;
  (void)snprintf(values, sizeof(values), "%s", attribute);
  char *first = strtok_r(values, " \t\r\n", &token_context);
  char *second = strtok_r(nullptr, " \t\r\n", &token_context);
  char *third = strtok_r(nullptr, " \t\r\n", &token_context);
  char *fourth = strtok_r(nullptr, " \t\r\n", &token_context);
  char *fifth = strtok_r(nullptr, " \t\r\n", &token_context);
  if (!first || !second || !third || !fourth ||
      !parse_int_checked(first, visibility) ||
      !parse_int_checked(second, light) ||
      !parse_int_checked(third, wind_direction) ||
      !parse_int_checked(fourth, wind_speed))
    return false;
  if (!fifth)
    return true;
  if (!parse_int_checked(fifth, cloud_base)) {
    char *message_rest = strtok_r(nullptr, "\r\n", &token_context);
    (void)snprintf(message, message_size, "%s%s%s", fifth,
                   message_rest ? " " : "", message_rest ? message_rest : "");
    return true;
  }
  char *message_text = strtok_r(nullptr, "\r\n", &token_context);
  if (message_text)
    (void)snprintf(message, message_size, "%s", message_text);
  return true;
}

void alter_conditions(BattleMap *map) {
  int i;
  Mech *mech;

  for (i = 0; i < battle_map_unit_count(map); i++) {
    mech = btech_context_get_mech(map->xcode.context,
                                  battle_map_unit_dbref(map, i));
    if (mech) {
      map_conditions_apply(mech, map);
    }
  }
}

int battle_map_gravity(const BattleMap *map) { return map->grav; }

int battle_map_light(const BattleMap *map) { return map->maplight; }

int battle_map_visibility(const BattleMap *map) { return map->mapvis; }

int battle_map_maximum_visibility(const BattleMap *map) { return map->maxvis; }

int battle_map_cloud_base(const BattleMap *map) { return map->cloudbase; }

int battle_map_temperature(const BattleMap *map) { return map->temp; }

float battle_map_movement_modifier(const BattleMap *map) {
  return map->movemod > 0 ? (float)map->movemod / 100.0F : 1.0F;
}

bool battle_map_sensor_is_disabled(const BattleMap *map, int sensor) {
  return (map->sensorflags & (1 << sensor)) != 0;
}

bool battle_map_bridges_have_capacity(const BattleMap *map) {
  return (map->flags & MAPFLAG_BRIDGESCS) != 0;
}

bool battle_map_is_vacuum(const BattleMap *map) {
  return (map->flags & MAPFLAG_VACUUM) != 0;
}

bool battle_map_disables_bridgification(const BattleMap *map) {
  return (map->flags & MAPFLAG_NOBRIDGIFY) != 0;
}

bool battle_map_disables_friendly_fire(const BattleMap *map) {
  return (map->flags & MAPFLAG_NOFRIENDLYFIRE) != 0;
}

bool battle_map_disables_physicals(const BattleMap *map) {
  return (map->flags & MAPFLAG_NOPHYSICALS) != 0;
}

bool battle_map_build_is_complex(const BattleMap *map) {
  return (map->buildflag & BUILDFLAG_CSI) != 0;
}

bool battle_map_build_is_complex_structure(const BattleMap *map) {
  return (map->buildflag & BUILDFLAG_CS) != 0;
}

bool battle_map_build_is_hidden(const BattleMap *map) {
  return (map->buildflag & (BUILDFLAG_DSS | BUILDFLAG_HID)) != 0;
}

bool battle_map_build_is_safe(const BattleMap *map) {
  return (map->buildflag & BUILDFLAG_NOB) != 0;
}

bool battle_map_build_is_invisible(const BattleMap *map) {
  return (map->buildflag & BUILDFLAG_HID) != 0;
}

bool battle_map_build_is_dropship_structure(const BattleMap *map) {
  return (map->buildflag & BUILDFLAG_DSS) != 0;
}

bool battle_map_is_dark(const BattleMap *map) {
  return (map->flags & MAPFLAG_DARK) != 0;
}

bool battle_map_is_underground(const BattleMap *map) {
  return (map->flags & MAPFLAG_UNDERGROUND) != 0;
}

void map_setconditions(DbRef player, BattleMap *map, char *buffer) {
  char *args[5];
  int vacuum = -1;
  int underground = -1;
  int grav;
  int temp;
  int argc;
  int fl;

  argc = mech_parseattributes(buffer, args, 4);
  if (argc < 2) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "(At least) 2 options required (gravity + temperature)");
    return;
  }
  if (argc > 4) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Too many options! Command accepts only 4 at max (gravity "
                 "+ temperature + vacuum-flag + underground-flag)");
    return;
  }
  if (!parse_int_checked(args[0], &grav)) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid gravity (must be integer in range of 0 to 255)");
    return;
  }
  if (grav < 0 || grav > 255) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid gravity (must be integer in range of 0 to 255)");
    return;
  }
  if (!parse_int_checked(args[1], &temp)) {
    mecha_notify(
        btech_context_evaluation(map->xcode.context), player,
        "Invalid temperature (must be integer in range of -128 to 127");
    return;
  }
  if (temp < -128 || temp > 127) {
    mecha_notify(
        btech_context_evaluation(map->xcode.context), player,
        "Invalid temperature (must be integer in range of -128 to 127");
    return;
  }
  if (argc > 2) {
    if (!parse_int_checked(args[2], &vacuum)) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid vacuum flag (must be integer, 0 or 1)");
      return;
    }
    if (vacuum < 0 || vacuum > 1) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid vacuum flag (must be integer, 0 or 1)");
      return;
    }
  }
  if (argc > 3) {
    if (!parse_int_checked(args[3], &underground)) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid underground flag (must be integer, 0 or 1)");
      return;
    }
    if (underground < 0 || underground > 1) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid underground flag (must be integer, 0 or 1)");
      return;
    }
  }
  fl = (map->flags & (~(MAPFLAG_SPEC | MAPFLAG_VACUUM)));
  if (vacuum > 0)
    fl |= MAPFLAG_VACUUM;
  if (underground > 0)
    fl |= MAPFLAG_UNDERGROUND;
  if (fl & MAPFLAG_VACUUM)
    fl |= MAPFLAG_SPEC;
  if (temp < -30 || temp > 50 || grav != 100)
    fl |= MAPFLAG_SPEC;
  map->temp = clamp_int_to_char(temp);
  map->grav = clamp_int_to_unsigned_char(grav);
  map->flags = fl;
  mecha_notify(btech_context_evaluation(map->xcode.context), player,
               "Conditions set!");
  alter_conditions(map);
}

void map_conditions_apply(Mech *mech, BattleMap *map) {
  if (!mech)
    return;
  if (!map) {
    mech_environment_conditions_set(mech, false, false, false, false);
    return;
  }
  mech_environment_conditions_set(mech, battle_map_uses_special_rules(map),
                                  (battle_map_temperature(map) < -30 ||
                                   battle_map_temperature(map) > 50) != 0,
                                  battle_map_gravity(map) != 100,
                                  battle_map_is_vacuum(map));
}

bool battle_map_uses_special_rules(const BattleMap *map) {
  return (map->flags & MAPFLAG_SPEC) != 0;
}
