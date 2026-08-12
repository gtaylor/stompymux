#include "equipment_types.h"
#include "mech_api_types.h"
#include "mux/server/runtime_clock.h" // IWYU pragma: keep
#include <stdint.h>
#include <time.h>
/* Implements BattleTech movement mechanics for aerospace move. */
#define MIN_TAKEOFF_SPEED 3
#include "aero_move_api.h"
#include "artillery_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "econ_cmds_api.h"
#include "map_conditions_api.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_lite_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tag_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/commands/action_messages.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include <math.h>
#include <stdlib.h>
struct land_data_type {
  UnitClass type;
  double maxvertup;
  double maxvertdown;
  double minhoriz;
  double maxhoriz;
  double launchvert;
  int launchtime; /* In secs */
  const char *landmsg;
  const char *landmsg_others;
  const char *takeoff;
  const char *takeoff_others;
}; /*           maxvertup / maxvertdown / minhoriz / maxhoriz / launchv /
     launchtime */
static const struct land_data_type land_data[] = {
    {CLASS_VTOL, 10, -60, -15, 15, 5, 0,
     "You bring your VTOL to a safe landing.", "lands.",
     "The rotor whines overhead as you lift off into the sky.", "takes off!"},
    {CLASS_AERO, 10, -30, (double)MIN_TAKEOFF_SPEED * (double)MP1, 999, 20, 10,
     "You land your AeroFighter safely.", "lands safely.",
     "The Aerofighter launches into the air!", "launches into the air!"},
    {CLASS_DS, 10, -25, (double)MIN_TAKEOFF_SPEED * (double)MP1, 999, 20, 300,
     "The DropShip lands safely.", "lands safely.",
     "The DropShip's nose lurches upward, and it starts climbing to the sky!",
     "starts climbing to the sky!"},
    {CLASS_SPHEROID_DS, 15, -40, -40, 40, 20, 300,
     "The DropShip touches down safely.", "touches down, and settles.",
     "The DropShip slowly lurches upwards as engines battle the gravity..",
     "starts climbing up to the sky!"}};
#define NUM_LAND_TYPES (sizeof(land_data) / sizeof(struct land_data_type))
static const struct land_data_type *land_data_entry(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(land_data, NUM_LAND_TYPES, sizeof(*land_data),
                                  (size_t)index);
}
static void aero_takeoff_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int i = -1;
  long count = (long)e->data2;
  if (mech_is_dropship(mech))
    for (i = 0; i < (int)(NUM_LAND_TYPES); i++)
      if (mech_class(mech) == land_data_entry(i)->type)
        break;
  if (count > 0) {
    if (count > 5) {
      if (!(count % 10))
        mech_printf(mech, MECHALL, "Launch countdown: %ld.", count);
    } else {
      mech_printf(mech, MECHALL, "Launch countdown: %ld.", count);
    }
    if (i >= 0) {
      if (count == (land_data_entry(i)->launchtime / 4))
        dropship_notification_broadcast(
            mech, "'s engines start to glow with unbearable intensity..");
      switch (count) {
      case 10:
        dropship_notification_broadcast(
            mech, "'s engines are almost ready to lift off!");
        break;
      case 6:
        dropship_notification_broadcast(
            mech, "'s engines generate a tremendous heat wave!");
        mech_sensors_scramble_infrared_and_liteamp(&(SensorScrambleRequest){
            .source = mech,
            .duration = 2,
            .infrared_message =
                "The blinding flash of light momentarily blinds you!",
            .light_amplification_message =
                "The blinding flash of light momentarily blinds you!"});
        break;
      case 2:
        mech_notify(mech, MECHALL,
                    "The engines pulse out a stream of superheated plasma!");
        dropship_notification_broadcast(
            mech,
            "'s engines send forth a tremendous stream of superheated plasma!");
        mech_sensors_scramble_infrared_and_liteamp(&(SensorScrambleRequest){
            .source = mech,
            .duration = 4,
            .infrared_message = "The blinding flash of light blinds you!",
            .light_amplification_message =
                "The blinding flash of light blinds you!"});
        break;
      case 1:
        dropship_exhaust_blast(&(DropshipExhaustBlastRequest){
            .dropship = mech,
            .direct_message = "You receive a direct hit!",
            .direct_observer_message =
                "is caught in the middle of the inferno!",
            .nearby_message = "You are hit by the wave!",
            .nearby_observer_message = "gets hit by the wave!",
            .tree_message = "are instantly burned to ash!",
            .damage = 400,
        });
        break;
      }
    }
    mech_event_schedule(mech, EVENT_TAKEOFF, aero_takeoff_event, 1, count - 1);
    return;
  }
  if (i < 0) {
    if ((mech_class(mech) == CLASS_AERO || mech_class(mech) == CLASS_DS) &&
        mech_current_speed(mech) < (MIN_TAKEOFF_SPEED * MP1)) {
      mech_notify(mech, MECHALL, "You're moving too slowly to lift off!");
      return;
    }
    for (i = 0; i < (int)(NUM_LAND_TYPES); i++)
      if (mech_class(mech) == land_data_entry(i)->type)
        break;
  }
  mech_spinning_set(mech, false);
  mech_notify(mech, MECHALL, land_data_entry(i)->takeoff);
  mech_los_broadcast(mech, land_data_entry(i)->takeoff_others);
  mech_motion_vector_reset(mech);
  if (mech_is_dropship(mech))
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_DS_INFO, "%s",
                       tprintf("DS #%ld has lifted off at %d %d "
                               "on map #%ld",
                               mech_dbref(mech), mech_position_x(mech),
                               mech_position_y(mech), battle_map_dbref(map)));
  if (mech_condition_summary(mech).hidden) {
    mech_notify(mech, MECHALL, "You move too much and break your cover!");
    mech_los_broadcast(mech, "breaks its cover in the brush.");
    mech_hidden_set(mech, false);
  }
  if (mech_class(mech) != CLASS_VTOL) {
    mech_desired_angle_set(mech, 90);
    mech_desired_speed_set(mech, mech_maximum_speed(mech) * 2 / 3);
  } else {
    mech_movement_stop(mech);
    mech_vertical_speed_set(mech, 60.0F);
  }
  mech_continue_flying(mech);
  mech_maybe_move(mech);
}
void aero_takeoff(DbRef player, void *data, const char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int i;
  long j;
  for (i = 0; i < (int)(NUM_LAND_TYPES); i++)
    if (mech_class(mech) == land_data_entry(i)->type)
      break;
  j = 0;
  if (*buffer != '\0' && !parse_long_checked(buffer, &j)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid takeoff argument!");
    return;
  }
  if (j != 0)
    if (!is_wizard(btech_context_database(mech_context(mech)), player)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Insufficient access!");
      return;
    }
  if (mech_event_count(mech, EVENT_TAKEOFF)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The launch sequence has already been initiated!");
    return;
  }
  if (i == (int)(NUM_LAND_TYPES)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This vehicle type cannot takeoff!");
    return;
  }
  if (!common_checks(player, mech, MECH_USUAL))
    return;
  if (mech_condition_summary(mech).fortified) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your fortified state prevents you from moving.");
    return;
  }
  if (!(mech_is_aerospace_unit(mech) ||
        mech_movement_type(mech) == MOVE_VTOL)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Only VTOL, Aerospace fighters and Dropships can take off.");
    return;
  }
  if (!mech_is_landed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You haven't landed!");
    return;
  }
  if (mech_condition_summary(mech).fallen ||
      (mech_effective_maximum_speed(mech) <= MP1) ||
      ((mech_section_is_destroyed(mech, ROTOR)) &&
       mech_class(mech) == CLASS_VTOL)) {
    if (mech_class(mech) == CLASS_VTOL) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "The rotor's dead!");
      return;
    }
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The engines are dead!");
    return;
  }
  if (!mech_aero_has_free_fuel(mech) && mech_fuel(mech) < 1) {
    if (mech_class(mech) == CLASS_VTOL) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Your VTOL's out of fuel!");
      return;
    }
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "Your craft's out of fuel! No taking off until it's refueled.");
    return;
  }
  if (mech_class(mech) == CLASS_AERO &&
      mech_current_speed(mech) < (MIN_TAKEOFF_SPEED * MP1)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're moving too slowly to take off!");
    return;
  }
  if (battle_map_is_underground(map)) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "Realize the ceiling in this grotto is a bit to low for that!");
    return;
  }
  if (land_data_entry(i)->launchtime > 0)
    mech_notify(mech, MECHALL,
                "Launch sequence initiated.. type 'land' to abort it.");
  dropship_notification_broadcast_if_due(mech,
                                         "starts warming engines for liftoff!");
  if (mech_is_dropship(mech))
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_DS_INFO, "%s",
        tprintf("DS #%ld has started takeoff at %d %d on map #%ld",
                mech_dbref(mech), mech_position_x(mech), mech_position_y(mech),
                battle_map_dbref(map)));
  if (mech_condition_summary(mech).hidden) {
    mech_notify(mech, MECHALL, "You break your cover to takeoff!");
    mech_los_broadcast(mech, "breaks its cover as it begins takeoff.");
    mech_hidden_set(mech, false);
  }
  mech_event_cancel(mech, EVENT_HIDE);
  mech_event_schedule(mech, EVENT_TAKEOFF, aero_takeoff_event, 1,
                      (void *)j ? j : land_data_entry(i)->launchtime);
}
void dropship_exhaust_blast(const DropshipExhaustBlastRequest *request) {
  Mech *mech = request->dropship;
  const char *hitmsg = request->direct_message;
  const char *hitmsg1 = request->direct_observer_message;
  const char *nearhitmsg = request->nearby_message;
  const char *nearhitmsg1 = request->nearby_observer_message;
  const char *treehitmsg = request->tree_message;
  const int DAMAGE = request->damage;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int x = mech_position_x(mech), y = mech_position_y(mech),
      z = mech_position_z(mech);
  int x1, y1, x2, y2, d;
  int rng = (DAMAGE > 100 ? 5 : 3);
  for (x1 = x - rng; x1 <= (x + rng); x1++)
    for (y1 = y - rng; y1 <= (y + rng); y1++) {
      x2 = bounded(0, x1, battle_map_width(map) - 1);
      y2 = bounded(0, y1, battle_map_height(map) - 1);
      if (x1 != x2 || y1 != y2)
        continue;
      d = map_hex_distance(&(HexDistanceRequest){
          .start = {.x = x, .y = y},
          .end = {.x = x1, .y = y1},
          .correction = 0,
      });
      if (d > rng)
        continue;
      d = max(1, d);
      switch (map_real_terrain_get(map, x1, y1)) {
      case BATTLE_TERRAIN_LIGHT_FOREST:
      case BATTLE_TERRAIN_HEAVY_FOREST:
        if (!find_decorations(map, x1, y1)) {
          hex_los_broadcast(
              map, x1, y1,
              tprintf("[fg=red bold]The trees in $h %s[reset]", treehitmsg));
          if ((DAMAGE / d) > 100) {
            map_terrain_set(map, x1, y1, BATTLE_TERRAIN_ROUGH);
          } else {
            add_decoration(&(MapDecorationRequest){
                .map = map,
                .position = {.x = x1, .y = y1},
                .type = MAP_DECORATION_TYPE_FIRE,
                .terrain_marker = MAP_DECORATION_FIRE_MARKER,
                .duration =
                    btech_random_range_int(battle_map_context(map), 60, 180),
            });
          }
        }
        break;
      }
    }
  mech_position_hex_z_set(mech, z + 6);
  BlastRealAreaRequest blast = {
      .center =
          {
              .map = map,
              .damage = {.total = DAMAGE, .hit_size = 5, .heat = DAMAGE / 2},
              .impact = {.x = mech_position_real_x(mech),
                         .y = mech_position_real_y(mech)},
              .source = {.x = mech_position_real_x(mech),
                         .y = mech_position_real_y(mech)},
              .messages = {.target = hitmsg, .observers = hitmsg1},
              .safety = {.above = 4, .below = 4, .underwater = true},
          },
      .neighbor_messages = {.target = nearhitmsg, .observers = nearhitmsg1},
      .neighbor_radius = rng,
  };
  blast_hit_real_area(&blast);
  mech_position_hex_z_set(mech, z);
}
enum { NO_ERROR, INVALID_TERRAIN, UNEVEN_TERRAIN, BLOCKED_LZ };
static const char *const REASONS[] = {"Improper terrain", "Uneven ground",
                                      "Blocked landing zone"};
const char *aero_landing_reason(int index) {
  if (index < 0)
    abort();
  const char *const *reason = (const char *const *)checked_storage_at_const(
      (const void *)REASONS, sizeof(REASONS) / sizeof(*REASONS),
      sizeof(*REASONS), (size_t)index);
  return *reason;
}
typedef struct LandingZoneCheck LandingZoneCheck;
struct LandingZoneCheck {
  int height;
  int matching_neighbors;
};
static void improper_lz_callback(BattleMap *map, int x, int y, void *context) {
  LandingZoneCheck *check = context;
  if (battle_map_hex_elevation(map, x, y) != check->height)
    check->matching_neighbors = 0;
  else
    check->matching_neighbors++;
}
static int aero_current_landing_zone_check(Mech *mech) {
  return aero_landing_zone_check(mech, mech_position_x(mech),
                                 mech_position_y(mech));
}
int aero_landing_zone_check(Mech *mech, int x, int y) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  LandingZoneCheck check = {
      .height = battle_map_hex_elevation(map, x, y),
  };
  if (map_real_terrain_get(map, x, y) != BATTLE_TERRAIN_GRASSLAND &&
      map_real_terrain_get(map, x, y) != BATTLE_TERRAIN_ROAD)
    if (btech_context_landing_zone_mode(mech_context(mech)) == 0)
      return INVALID_TERRAIN;
  visit_neighbor_hexes(map, x, y, improper_lz_callback, &check);
  if (check.matching_neighbors != 6)
    if (btech_context_landing_zone_mode(mech_context(mech)) == 0)
      return UNEVEN_TERRAIN;
  if (is_blocked_lz(mech, map, x, y))
    return BLOCKED_LZ;
  return NO_ERROR;
}
void aero_land(DbRef player, void *data, const char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int i, t;
  double horiz = 0.0;
  if (mech_class(mech) != CLASS_VTOL && mech_class(mech) != CLASS_AERO &&
      !mech_is_dropship(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't land this type of vehicle.");
    return;
  }
  if (mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0 &&
      !mech_aero_has_free_fuel(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You lack fuel to maneuver for landing!");
    return;
  }
  for (i = 0; i < (int)(NUM_LAND_TYPES); i++)
    if (mech_class(mech) == land_data_entry(i)->type)
      break;
  if (i == (int)(NUM_LAND_TYPES))
    return;
  if ((mech_condition_summary(mech).fallen) &&
      (mech_class(mech) == CLASS_VTOL)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The rotor's dead!");
    return;
  }
  if ((mech_condition_summary(mech).fallen) &&
      (mech_class(mech) != CLASS_VTOL)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The engines are dead!");
    return;
  }
  if (mech_is_landed(mech)) {
    if (mech_event_count(mech, EVENT_TAKEOFF)) {
      mech_printf(
          mech, MECHALL, "Launch aborted by %s.",
          game_object_name(btech_context_database(mech_context(mech)), player));
      if (mech_is_dropship(mech))
        btech_channel_send(mech_context(mech), BTECH_CHANNEL_DS_INFO, "%s",
                           tprintf("DS #%ld aborted takeoff at %d %d "
                                   "on map #%ld",
                                   mech_dbref(mech), mech_position_x(mech),
                                   mech_position_y(mech),
                                   battle_map_dbref(map)));
      mech_event_cancel(mech, EVENT_TAKEOFF);
      return;
    }
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're already landed!");
    return;
  }
  if (mech_position_z(mech) > mech_position_surface_elevation(mech) + 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are too high to land here.");
    return;
  }
  horiz = my_sqrtm((double)mech_desired_speed(mech),
                   (double)mech_vertical_speed(mech));
  if (horiz >= (1.0 + land_data_entry(i)->maxhoriz)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're moving too fast to land.");
    return;
  }
  if (horiz < land_data_entry(i)->minhoriz) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're moving too slowly to land.");
    return;
  }
  const float VERTICAL_SPEED = mech_vertical_speed(mech);
  const float CURRENT_SPEED = mech_current_speed(mech);
  if ((double)VERTICAL_SPEED > land_data_entry(i)->maxvertup ||
      (double)VERTICAL_SPEED < land_data_entry(i)->maxvertdown) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are moving too fast to land. ");
    return;
  }
  if ((double)CURRENT_SPEED < land_data_entry(i)->minhoriz) {
    if (mech_motion_vector_z(mech) <= 0)
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "You're falling, not landing! Pick up some horizontal speed first.");
    else
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You're climbing not landing!");
    return;
  }
  MechHex hex;
  if (!mech_hex_get(mech, &hex)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't land without a valid map hex.");
    return;
  }
  t = (unsigned char)hex.real_terrain;
  if (!(t == BATTLE_TERRAIN_GRASSLAND || t == BATTLE_TERRAIN_ROAD ||
        (mech_class(mech) == CLASS_VTOL && t == BATTLE_TERRAIN_BUILDING))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't land on this type of terrain.");
    return;
  }
  if (mech_class(mech) != CLASS_VTOL && aero_current_landing_zone_check(mech)) {
    mech_notify(mech, MECHALL, "This location is no good for landing!");
    return;
  }
  if (mech_is_dropship(mech))
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_DS_INFO, "%s",
                       tprintf("DS #%ld has landed at %d %d on map #%ld",
                               mech_dbref(mech), mech_position_x(mech),
                               mech_position_y(mech), battle_map_dbref(map)));
  mech_notify(mech, MECHALL, land_data_entry(i)->landmsg);
  mech_los_broadcast(mech, land_data_entry(i)->landmsg_others);
  mech_position_z_set(mech, mech_position_surface_elevation(mech));
  mech_landed_set(mech, true);
  mech_current_speed_set(mech, 0.0F);
  mech_vertical_speed_set(mech, 0.0F);
  mech_motion_vector_reset(mech);
  notify_event(btech_context_evaluation(mech_context(mech)), NULL,
               mech_dbref(mech), mech_dbref(mech), mech_dbref(mech),
               LUA_EVENT_AERO_LAND, (char **)NULL, 0);
  mine_field_trigger(mech, MINE_LAND);
}
void aero_control_effect(Mech *mech) {
  if (mech_condition_summary(mech).spinning)
    return;
  if (mech_is_destroyed(mech))
    return;
  if (mech_is_landed(mech))
    return;
  mech_notify(mech, MECHALL, "You lose control of your craft!");
  mech_los_broadcast(mech, "spins out of control!");
  mech_spinning_set(mech, true);
  mech_spin_start_tick_set(mech, btech_context_now(mech_context(mech)));
}
void dropship_bridge_hit(Mech *mech) {
  /* Implementation: Kill all players on bridge :-) */
  if (mech_is_destroyed(mech))
    return;
  if (is_in_character(btech_context_database(mech_context(mech)),
                      mech_dbref(mech)))
    mech_notify(mech, MECHALL,
                "DUCK! The shot seems to be coming straight for the bridge!");
  mech_contents_kill_if_in_character(mech);
}
static float degrees_sine(float angle) {
  return sinf(angle * (float)M_PI / 180.0F);
}
static float degrees_cosine(float angle) {
  return cosf(angle * (float)M_PI / 180.0F);
}
static float length_hypotenuse_float(float x, float y) {
  return sqrtf(x * x + y * y);
}
void aero_speed_update(Mech *mech) {
  float xypart;
  float wx, wy, wz;
  float nx, ny, nz;
  float nh;
  float dx, dy, dz;
  float vlen, mod;
  float ab = 0.7F;
  float m = 1.0F;
  if (mech_condition_summary(mech).spinning) {
    const int SPEED_ADJUSTMENT =
        btech_random_range_int(mech_context(mech), 1, 10);
    const float RANDOMIZED_SPEED =
        mech_desired_speed(mech) + (float)SPEED_ADJUSTMENT;
    const float BOUNDED_SPEED = fminf(fmaxf(0.0F, RANDOMIZED_SPEED),
                                      mech_effective_maximum_speed(mech));
    mech_desired_speed_set(mech, BOUNDED_SPEED);
    mech_desired_angle_set(
        mech, max(-90, mech_desired_angle(mech) -
                           btech_random_range_int(mech_context(mech), 1, 15)));
    mech_desired_heading_set(
        mech,
        acceptable_degree(mech_desired_heading_degrees(mech) +
                          btech_random_range_int(mech_context(mech), -3, 3)));
  }
  const int DESIRED_ANGLE = mech_desired_angle(mech);
  wz = mech_desired_speed(mech) * degrees_sine((float)DESIRED_ANGLE);
  if (mech_class(mech) == CLASS_AERO)
    ab = 2.5F;
  if (mech_position_z(mech) < ATMO_Z)
    ab /= 2.0F;
  /* First, we calculate the vector we want to be going */
  xypart = mech_desired_speed(mech) * degrees_cosine((float)DESIRED_ANGLE);
  if (mech_fuel(mech) < 0) {
    wz /= 5.0F;
    xypart /= 5.0F;
  }
  if (xypart < 0.0F)
    xypart = -xypart;
  m = (float)ACCEL_MOD;
  MapRealPosition wind = map_vector_components(&(MapPolarVector){
      .magnitude = m * xypart, .bearing = mech_desired_heading_degrees(mech)});
  wx = wind.x;
  wy = wind.y;
  wz = wz * m;
  /* Then, we calculate the present heading / speed */
  nx = mech_motion_vector_x(mech);
  ny = mech_motion_vector_y(mech);
  nz = mech_motion_vector_z(mech);
  /* Ok, we've present heading / speed */
  /* Next, we make vector from n[xyz] -> w[xyz] */
  dx = wx - nx;
  dy = wy - ny;
  dz = wz - nz;
  vlen = length_hypotenuse_float(length_hypotenuse_float(dx, dy), dz);
  if (!(vlen > 0.0F))
    return;
  if (vlen >
      (m * ab * mech_effective_maximum_speed(mech) / (float)AERO_SECS_THRUST)) {
    mod = ab * m * mech_effective_maximum_speed(mech) /
          (float)AERO_SECS_THRUST / vlen;
    dx *= mod;
    dy *= mod;
    dz *= mod;
    /* Ok.. we've a new modified speed vector */
  }
  nx += dx;
  ny += dy;
  nz += dz;
  /* Then, we need to calculate present heading / speed / verticalspeed */
  nh = atan2f(ny, nx) * 180.0F / (float)M_PI;
  if (!(mech_class(mech) == CLASS_SPHEROID_DS))
    mech_heading_set(mech, acceptable_degree((int)nh + 90));
  xypart = length_hypotenuse_float(nx, ny);
  mech_current_speed_set(mech, xypart);
  mech_vertical_speed_set(mech, nz);
  if (!(mech_class(mech) == CLASS_SPHEROID_DS) &&
      fabsf(mech_current_speed(mech)) < MP1)
    mech_heading_set(mech, mech_desired_heading_degrees(mech));
  mech_motion_vector_set(mech, (MapSpatialPosition){.x = nx, .y = ny, .z = nz});
}
int aero_fuel_check(Mech *mech) {
  int fuelcost = 1;
  /* We don't do anything particularly nasty to shutdown things */
  if (!mech_is_started(mech))
    return 0;
  if (mech_aero_has_free_fuel(mech))
    return 0;
  if (fabsf(mech_current_speed(mech)) > mech_effective_maximum_speed(mech)) {
    if (mech_position_z(mech) < ATMO_Z) {
      const float FUEL_RATIO =
          fabsf(mech_current_speed(mech) / mech_effective_maximum_speed(mech));
      fuelcost = (int)FUEL_RATIO;
    }
  } else if (fabsf(mech_current_speed(mech)) < MP1 &&
             fabsf(mech_vertical_speed(mech)) < MP2) {
    if (btech_random_range_int(mech_context(mech), 0, 1) == 0)
      return 0; /* Approximately half of the time free */
  }
  if (mech_fuel(mech) > 0) {
    if (mech_fuel(mech) <= fuelcost)
      mech_fuel_set(mech, 0);
    else
      mech_fuel_decrement(mech, fuelcost);
    return 0;
  }
  /* DropShips do not need crash ; they switch to (VERY SLOW) secondary
     power source. */
  if (mech_is_dropship(mech)) {
    if (mech_fuel(mech) < 0)
      return 0;
    mech_fuel_decrement(mech, 1);
    mech_notify(mech, MECHALL,
                "As the fuel runs out, the engines switch to backup power.");
    return 0;
  }
  if (mech_fuel(mech) < 0)
    return 1;
  /* Now, the true nastiness begins ;) */
  mech_fuel_decrement(mech, 1);
  if (!(mech_fuel(mech) % 100) && mech_fuel(mech) >= mech_original_fuel(mech))
    mech_cargo_weight_recalculate(mech);
  if (mech_class(mech) == CLASS_VTOL) {
    mech_los_broadcast(mech, "'s rotors suddenly stop!");
    mech_notify(mech, MECHALL, "The sound of rotors slowly stops..");
  } else {
    mech_los_broadcast(mech, "'s engines die suddenly..");
    mech_notify(mech, MECHALL, "Your engines die suddenly..");
  }
  mech_movement_stop(mech);
  if (!mech_is_landed(mech)) {
    mech_notify(mech, MECHALL,
                "You ponder F = ma, S = F/m, S = at^2 => S=agt^2 in relation "
                "to the ground..");
    /* Start free-fall */
    mech_vertical_speed_set(mech, 0.0F);
    /* Hmm. This _can_ be ugly if things crash in middle of fall. Oh well. */
    mech_notify(mech, MECHALL, "You start free-fall.. Enjoy the ride!");
    mech_event_schedule(mech, EVENT_FALL, mech_fall_event, FALL_TICK, -1);
  }
  return 1;
}
void aero_update(Mech *mech) {
  if (mech_is_destroyed(mech))
    return;
  if (mech_is_started(mech) || mech_pilot_is_unconscious(mech)) {
    mech_piloting_update(mech);
  }
  if (mech_is_started(mech) || mech_added_heat(mech) > 0.0F)
    mech_heat_update(mech);
  if (!(btech_context_now(mech_context(mech)) / 3 % 5)) {
    if (!mech_condition_summary(mech).spinning)
      return;
    if (mech_is_destroyed(mech))
      return;
    if (mech_is_landed(mech))
      return;
    time_t const SPIN_DURATION =
        mech_spin_start_tick(mech) - btech_context_now(mech_context(mech));
    int const SPIN_MODIFIER = clamp_intptr_to_int((intptr_t)SPIN_DURATION);
    if (made_pilot_skill_roll(mech, SPIN_MODIFIER / 15 + 8)) {
      mech_notify(mech, MECHALL, "You recover control of your craft.");
      mech_spinning_set(mech, false);
    }
  }
  if (mech_is_started(mech))
    mech_sensor_visibility_modifier_set(
        mech, bounded(0,
                      mech_sensor_visibility_modifier(mech) +
                          btech_random_range_int(mech_context(mech), -40, 40),
                      100));
  mech_ecm_check(mech);
  mech_tag_check(mech);
  end_lite_check(mech);
}
static const char *colorstr(int serious) {
  if (serious == 1)
    return "[fg=red bold]";
  if (serious == 0)
    return "[fg=yellow bold]";
  return "";
}
void dropship_land_warning(Mech *mech, int serious) {
  int ilz = aero_current_landing_zone_check(mech);
  if (!ilz)
    return;
  ilz--;
  mech_printf(mech, MECHALL, "%sWARNING: %s - %s[reset]", colorstr(serious),
              aero_landing_reason(ilz),
              serious == 1   ? "CLIMB UP NOW!!!"
              : serious == 0 ? "No further descent is advisable."
                             : "Please do not even consider landing here.");
}
