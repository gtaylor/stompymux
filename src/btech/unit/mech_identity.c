#include "autopilot.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "ds_bay_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_startup_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mech_utils_internal.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mux/world/move.h"
#include "random.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include <math.h>
#include <stddef.h>
#include <string.h>
#include <strings.h>

static const char *const MECHTYPENAMES[CLASS_LAST + 1] = {
    "mech", "tank", "VTOL", "vessel", "aerofighter", "DropShip"};

const char *mechtypename(Mech *foo) {
  UnitClass unit_class = mech_class(foo);
  const char *const *name = (const char *const *)checked_storage_at_const(
      (const void *)MECHTYPENAMES, CLASS_LAST + 1, sizeof(*MECHTYPENAMES),
      (size_t)unit_class);
  return *name;
}

int mech_armorpoints(Mech *mech) {
  int i;
  int points = 0;

  for (i = 0; i < NUM_SECTIONS; i++) {
    points += mech_section_armor(mech, i);
    points += mech_section_rear_armor(mech, i);
  }

  return points;
}

int mech_intpoints(Mech *mech) {
  int i;
  int points = 0;

  for (i = 0; i < NUM_SECTIONS; i++) {
    points += mech_section_internal(mech, i);
  }

  return points;
}

int round_to_halfton(int weight) {
  int over = weight % 512;
  if (!over)
    return weight;
  if (over < 2)
    return weight - over;
  return weight + (512 - over);
}

int round_to_quarterton(int weight) {
  int over = weight % 256;
  if (!over)
    return weight;
  if (over < 2)
    return weight - over;

  return weight + (256 - over);
}

int m_number(Mech *mech, int low, int high) {
  if ((mech->xcode.context->events->tick / RANDOM_TICK) !=
      ((mech)->rd.lastrndu)) {
    ((mech)->rd.rnd) = (int)btech_random_i31(&mech->xcode.context->random);
    ((mech)->rd.lastrndu) = mech->xcode.context->events->tick / RANDOM_TICK;
  }
  return (low + (((mech)->rd.rnd) % (high - low + 1)));
}

MechId mech_id(Mech *mech, bool lowercase) {
  MechId id;

  if (mech) {
    id.text[0] = ((mech)->id)[0];
    id.text[1] = ((mech)->id)[1];
  } else {
    id.text[0] = '*';
    id.text[1] = '*';
  }
  id.text[2] = '\0';

  if (lowercase) {
    id.text[0] = ascii_to_lower(id.text[0]);
    id.text[1] = ascii_to_lower(id.text[1]);
  }
  return id;
}

char *my_to_upper(char *string) {
  if (*string)
    *string = ascii_to_upper(*string);
  return string;
}

int crits_in_loc(Mech *mech, int index) {
  if (((mech)->ud.type) == CLASS_MECH) {
    switch (index) {
    case HEAD:
    case RLEG:
    case LLEG:
      return 6;
    case RARM:
    case LARM:
      if (mech_is_quad(mech))
        return 6;
    }
  } else if (((mech)->ud.type) == CLASS_MW) {
    return 2;
  }
  return NUM_CRITICALS;
}

int sect_has_busy_weap(Mech *mech, int sect) {
  int i = 0;
  int count;
  int critical[MAX_WEAPS_SECTION];
  unsigned char weaptype[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];

  count = find_weapons_advanced(mech, sect, weaptype, weapdata, critical, 1);
  for (i = 0; i < count; i++) {
    const int *critical_index = checked_storage_at_const(
        critical, MAX_WEAPS_SECTION, sizeof(*critical), (size_t)i);
    if (mech_weapon_is_recycling_at(mech, sect, *critical_index))
      return 1;
  }
  return 0;
}

BattleMap *valid_map(const MapValidationRequest *request) {
  BtechContext *context = request->context;
  DbRef player = request->player;
  DbRef map = request->map;
  char *str;
  BattleMap *maps;

  if (!is_good_obj(context->database, map)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Index out of range!");
    return nullptr;
  }
  str = btech_attribute_read(context->database, map, A_XTYPE,
                             (char[LBUF_SIZE]){0});
  if (!str || !*str) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That is not a valid map! (no XTYPE!)");
    return nullptr;
  }
  if (strcmp("MAP", str)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That is not a valid map!");
    return nullptr;
  }
  maps = btech_context_get_map(context, map);
  if (!maps) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The map has not been allocated!!");
    return nullptr;
  }
  return maps;
}

DbRef find_mech_on_map(BattleMap *map, const char *mechid) {
  Mech *temp_mech;

  for (int loop = 0; loop < battle_map_unit_count(map); loop++) {
    DbRef candidate = battle_map_unit_dbref(map, loop);
    if (candidate != -1) {
      temp_mech = btech_context_get_mech(map->xcode.context, candidate);
      if (temp_mech && !strncasecmp(((temp_mech)->id), mechid, 2))
        return temp_mech->mynum;
    }
  }
  return -1;
}

DbRef find_target_dbref_from_map_number(Mech *mech, const char *mapnum) {
  BattleMap *map;

  if (mech->mapindex == -1)
    return -1;
  map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  if (!map) {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("FTDBREFFMN:invalid map:Mech: %ld  Index: %ld",
                               mech->mynum, mech->mapindex));
    mech->mapindex = -1;
    return -1;
  }
  return find_mech_on_map(map, mapnum);
}

MapRealPosition map_vector_components(const MapPolarVector *vector) {
  float angle = (float)vector->bearing + 90.0F;
  MapRealPosition result = {
      .x = vector->magnitude * cosf((float)M_PI / 180.0F * angle),
      .y = vector->magnitude * sinf((float)M_PI / 180.0F * angle),
  };
  result.x = -result.x; /* because 90 is to the right */
  result.y = -result.y; /* because y increases downwards */
  return result;
}

static int leave_hangar(BattleMap *map, Mech *mech) {
  Mech *car = NULL;
  DbRef mapob;
  MapObject *mapo;

  /* For now, leaving leads to finding yourself on the new map
     at a predetermined position */
  mapob = mech->mapindex;
  if (((mech)->rd.carrying) > 0)
    car = btech_context_get_mech(mech->xcode.context, ((mech)->rd.carrying));
  if (!map->cf) {
    mech_notify(mech, MECHALL, "The entrance is still filled with rubble!");
    return 0;
  }
  mech_los_broadcast(mech, "has left the hangar.");
  mech_rsetmapindex(GOD, (void *)mech,
                    tprintf("%ld", map->map_object[TYPE_LEAVE]->obj));
  if (car)
    mech_rsetmapindex(GOD, (void *)car,
                      tprintf("%ld", map->map_object[TYPE_LEAVE]->obj));
  map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  if (mech->mapindex == mapob) {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("#%ld %s attempted to leave, but no target map?",
                               mech->mynum, mech_display_id(mech).text));
    mech_notify(mech, MECHALL,
                "Exit of this map is.. fubared. Please contact a wizard");
    return 0;
  }
  mapo = find_entrance_by_target(map, mapob);
  if (!mapo) {
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("#%ld %s attempted to leave, but no target place was "
                "found? setting the mech at 0,0 at %ld.",
                mech->mynum, mech_display_id(mech).text, mech->mapindex));
    mech_notify(mech, MECHALL,
                "Weird bug happened during leave. Please contact a wizard. ");
    return 1;
  }

  bsuit_swarmers_stop(
      btech_context_find_object(mech->xcode.context, mech->mapindex), mech, 1);
  mech_printf(mech, MECHALL, "You have left %s.",
              structure_name(mech->xcode.context->database, mapo).text);
  mech_rsetxy(GOD, (void *)mech, tprintf("%d %d", mapo->x, mapo->y));
  mech_continue_flying(mech);
  if (car)
    mech_position_mirror(car, mech, 0);
  mech_los_broadcast(
      mech, tprintf("has left %s at %d,%d.",
                    structure_name(mech->xcode.context->database, mapo).text,
                    ((mech)->pd.x), ((mech)->pd.y)));
  move_via_teleport(&(ObjectMovementRequest){
      .evaluation = btech_context_evaluation(mech->xcode.context),
      .object = mech->mynum,
      .destination = mech->mapindex,
      .cause = 1});
  if (car) {
    move_via_teleport(&(ObjectMovementRequest){
        .evaluation = btech_context_evaluation(mech->xcode.context),
        .object = car->mynum,
        .destination = mech->mapindex,
        .cause = 1});
  }
  if (is_in_character(mech->xcode.context->database, mech->mynum) &&
      game_object_location(mech->xcode.context->database,
                           mech_pilot_dbref(mech)) != mech->mynum) {
    mech_notify(
        mech, MECHALL,
        "[fg=red bold blink inverse]INTRUDER ALERT! INTRUDER ALERT![reset]");
    mech_notify(mech, MECHALL,
                "[fg=red bold blink]Automatic self-destruct sequence "
                "initiated...[reset]");
    mech_shutdown(GOD, (void *)mech, "");
  }
  auto_cal_mapindex(mech->xcode.context, mech);
  if (((mech)->rd.speed) > mech_effective_maximum_speed(mech))
    ((mech)->rd.speed) = mech_effective_maximum_speed(mech);
  return 1;
}

void check_edge_of_map(Mech *mech) {
  int pinned = 0;
  int linked;
  BattleMap *map;

  map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  if (!map) {
    mech_notify(mech, MECHPILOT, "You are on an invalid map! Map index reset!");
    mech_shutdown(mech_pilot_dbref(mech), (void *)mech, "");
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("CheckEdgeofMap:invalid map:Mech: %ld  Index: %ld", mech->mynum,
                mech->mapindex));
    mech->mapindex = -1;
    return;
  }
  linked = map_linked(mech->xcode.context, mech->mapindex);
  /* Prevents you from going off the map */
  /* Eventually this could wrap and all that.. */
  if (((mech)->pd.x) < 0) {
    if (linked) {
      ((mech)->pd.x) += map->map_width;
      pinned = -1;
    } else {
      ((mech)->pd.x) = 0;
      pinned = 4;
    }
  } else if (((mech)->pd.x) >= map->map_width) {
    if (linked) {
      ((mech)->pd.x) -= map->map_width;
      pinned = -1;
    } else {
      ((mech)->pd.x) = map->map_width - 1;
      pinned = 2;
    }
  }
  if (((mech)->pd.y) < 0) {
    if (linked) {
      pinned = -1;
      ((mech)->pd.y) += map->map_height;
    } else {
      ((mech)->pd.y) = 0;
      pinned = 1;
    }
  } else if (((mech)->pd.y) >= map->map_height) {
    if (linked) {
      pinned = -1;
      ((mech)->pd.y) -= map->map_height;
    } else {
      ((mech)->pd.y) = map->map_height - 1;
      pinned = 3;
    }
  }
  if (pinned > 0) {
    /* This is a DS bay. First, we need to check if the bay's doors are
       blocked, one way or another.
     */
    if (map->onmap && btech_context_is_mech(map->xcode.context, map->onmap)) {
      if (dropship_leave(map, mech))
        return;
    } else if (map->flags & MAPFLAG_MAPO && map->map_object[TYPE_LEAVE]) {
      if (leave_hangar(map, mech))
        return;
    }
  }
  if (pinned) {
    map_coord_to_real_coord(((mech)->pd.x), ((mech)->pd.y), &((mech)->pd.fx),
                            &((mech)->pd.fy));
    if (pinned > 0) {
      mech_notify(mech, MECHALL, "You cannot move off this map!");
      if (mech_is_jumping(mech) && !mech_is_aerospace_unit(mech))
        mech_jump_land(mech);
      ((mech)->rd.cocoon) = 0;
      ((mech)->rd.speed) = 0.0;
      ((mech)->rd.desired_speed) = 0.0;
      if (mech_is_aerospace_unit(mech)) {
        ((mech)->rd.startfx) = 0.0;
        ((mech)->rd.startfy) = 0.0;
        ((mech)->rd.startfz) = 0.0;
        if (!mech_is_landed(mech))
          mech_maybe_move(mech);
      }
    }
  }
}
int map_vertical_bearing(const MapSpatialSegment *segment) {
  float adj;
  float opp;
  float deg;

  adj = map_real_range(&(MapRealSegment){
      .start = {.x = segment->start.x, .y = segment->start.y},
      .end = {.x = segment->end.x, .y = segment->end.y},
  });
  /*
   * XXX: Why can't opp be negative?  If z1 < z0, shouldn't Z-bearing
   * also be negative?  Also, why no range clamping on the value of deg?
   */
  opp = fabsf(segment->end.z - segment->start.z) / (float)SCALEMAP;
  deg = radians_to_degrees(atan2f(opp, adj));
  return clamp_float_to_int(ceilf(deg));
}

int map_bearing(const MapRealSegment *segment) {
  const float DX = segment->end.x - segment->start.x;
  const float DY = segment->end.y - segment->start.y;

  float rads;
  int degrees;

  /*
   * atan2() doesn't need this check because we never actually divide by
   * dx, but we handle it specially for consistency with existing code.
   */
  if (DX == 0.F) {
    return (DY < 0.F) ? 0 : 180;
  }

  rads = atan2f(-DX, DY);

  /* Round off degrees.  */
  float scaled_degrees = radians_to_degrees(10.0F * rads);
  degrees = (clamp_float_to_int(scaled_degrees) + 5) / 10;

  return acceptable_degree(degrees + 180);
}

int in_weapon_arc(Mech *mech, float x, float y) {
  int relat;
  int bearing_to_target;
  int res = NOARC;

  bearing_to_target = map_bearing(
      &(MapRealSegment){.start = {.x = ((mech)->pd.fx), .y = ((mech)->pd.fy)},
                        .end = {.x = x, .y = y}});
  relat = mech_heading_degrees(mech) - bearing_to_target;
  if (((mech)->ud.type) == CLASS_MECH || ((mech)->ud.type) == CLASS_MW ||
      ((mech)->ud.type) == CLASS_BSUIT) {
    if (((mech)->rd.status) & TORSO_RIGHT)
      relat += 59;
    else if (((mech)->rd.status) & TORSO_LEFT)
      relat -= 59;
  }
  relat = acceptable_degree(relat);
  if (relat >= 300 || relat <= 60)
    res |= FORWARDARC;
  if (relat > 120 && relat < 240)
    res |= REARARC;
  if (relat >= 240 && relat < 300)
    res |= RSIDEARC;
  if (relat > 60 && relat <= 120)
    res |= LSIDEARC;

  if ((mech_class(mech) == CLASS_VEH_GROUND ||
       mech_class(mech) == CLASS_VEH_NAVAL || mech_class(mech) == CLASS_VTOL) &&
      mech_section_original_internal(mech, TURRET)) {
    relat = acceptable_degree(
        (mech_heading_degrees(mech) + ((mech)->rd.turretfacing)) -
        bearing_to_target);
    if (relat >= 330 || relat <= 30)
      res |= TURRETARC;
  }
  if (res == NOARC)
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("NoArc: #%ld: BearingToTarget:%d Facing:%d",
                               mech->mynum, bearing_to_target,
                               mech_heading_degrees(mech)));
  return res;
}

const char *find_gunnery_skill_name(Mech *mech, int weapindx) {
  if (!mech->xcode.context->configuration->btech_extended_gunnery) {
    switch (((mech)->ud.type)) {
    case CLASS_BSUIT:
      return "Gunnery-BSuit";
    case CLASS_MECH:
      return "Gunnery-Battlemech";
    case CLASS_VEH_GROUND:
    case CLASS_VEH_NAVAL:
      return "Gunnery-Conventional";
    case CLASS_VTOL:
    case CLASS_AERO:
      return "Gunnery-Aerospace";
    case CLASS_SPHEROID_DS:
    case CLASS_DS:
      return "Gunnery-Spacecraft";
    case CLASS_MW:
      if (weapindx >= 0) {
        if (!strcmp(weapon_catalogue_name(weapindx), "PC.Sword"))
          return "Blade";
        if (!strcmp(weapon_catalogue_name(weapindx), "PC.Vibroblade"))
          return "Blade";
      }
      return "Small_Arms";
    }
  } else {

    if (weapindx < 0)
      return nullptr;
    if (((mech)->ud.type) == CLASS_MW) {
      if (weapindx >= 0) {
        if (!strcmp(weapon_catalogue_name(weapindx), "PC.Blade"))
          return "Blade";
        if (!strcmp(weapon_catalogue_name(weapindx), "PC.Vibroblade"))
          return "Blade";
        if (!strcmp(weapon_catalogue_name(weapindx), "PC.Blazer"))
          return "Support_Weapons";
        if (!strcmp(weapon_catalogue_name(weapindx), "PC.HeavyGyrojetGun"))
          return "Support_Weapons";
        return "Small_Arms";
      }
    } else if (weapon_catalogue_is_artillery(weapindx)) {
      return "Gunnery-Artillery";
    } else if (weapon_catalogue_is_missile(weapindx)) {
      return "Gunnery-Missile";
    } else if (weapon_catalogue_is_ballistic(weapindx)) {
      return "Gunnery-Ballistic";
    } else if (weapon_catalogue_is_energy(weapindx)) {
      return "Gunnery-Laser";
    } else if (weapon_catalogue_is_flamer(weapindx)) {
      return "Gunnery-Flamer";
    }
  }
  return NULL;
}

const char *find_piloting_skill_name(Mech *mech) {
  if (!mech->xcode.context->configuration->btech_extended_piloting) {
    switch (((mech)->ud.type)) {
    case CLASS_MW:
      return "Running";
    case CLASS_BSUIT:
      return "Piloting-BSuit";
    case CLASS_MECH:
      return "Piloting-Battlemech";
    case CLASS_VEH_GROUND:
    case CLASS_VEH_NAVAL:
      return "Drive";
    case CLASS_VTOL:
    case CLASS_AERO:
      return "Piloting-Aerospace";
    case CLASS_SPHEROID_DS:
    case CLASS_DS:
      return "Piloting-Spacecraft";
    }
  } else {

    if (((mech)->ud.type) == CLASS_MW && mech_real_terrain_get(mech) == WATER)
      return "Swimming";
    switch (((mech)->ud.type)) {
    case CLASS_MW:
      return "Running";
    case CLASS_BSUIT:
      return "Piloting-Bsuit";
    case CLASS_VEH_NAVAL:
      return "Piloting-Naval";
    case CLASS_DS:
    case CLASS_SPHEROID_DS:
      return "Piloting-Spacecraft";
    case CLASS_VTOL:
    case CLASS_AERO:
      return "Piloting-Aerospace";
    }
    switch (((mech)->ud.move)) {
    case MOVE_BIPED:
      return "Piloting-Biped";
    case MOVE_QUAD:
      return "Piloting-Quad";
    case MOVE_TRACK:
      return "Piloting-Tracked";
    case MOVE_HOVER:
      return "Piloting-Hover";
    case MOVE_WHEEL:
      return "Piloting-Wheeled";
    }
  }
  return NULL;
}

int find_pilot_piloting(Mech *mech) {
  const char *str;

  if (mech_has_active_pilot(mech)) {
    str = find_piloting_skill_name(mech);
    if (str)
      return char_getskilltarget(mech->xcode.context, mech_pilot_dbref(mech),
                                 str, 0);
  }
  return DEFAULT_PILOTING;
}

int find_s_pilot_piloting(Mech *mech) {
  return find_pilot_piloting(mech) + (((mech)->ud.move) == MOVE_QUAD ? -2 : 0);
}

int find_pilot_spotting(Mech *mech) {
  if (mech_has_active_pilot(mech))
    return (char_getskilltarget(mech->xcode.context, mech_pilot_dbref(mech),
                                "Gunnery-Spotting", 0));
  return DEFAULT_SPOTTING;
}

int find_pilot_arty_gun(Mech *mech) {
  if (mech_has_active_gunner(mech))
    return (char_getskilltarget(mech->xcode.context, mech_gunner_dbref(mech),
                                "Gunnery-Artillery", 0));
  return DEFAULT_ARTILLERY;
}

int find_pilot_gunnery(Mech *mech, int weapindx) {
  const char *str;

  if (mech_has_active_gunner(mech)) {
    str = find_gunnery_skill_name(mech, weapindx);
    if (str)
      return char_getskilltarget(mech->xcode.context, mech_gunner_dbref(mech),
                                 str, 0);
  }
  return DEFAULT_GUNNERY;
}

const char *find_tech_skill_name(Mech *mech) {
  switch (((mech)->ud.type)) {
  case CLASS_MECH:
  case CLASS_BSUIT:
    return "Technician-Battlemech";
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
    return "Technician-Mechanic";
  case CLASS_AERO:
  case CLASS_VTOL:
  case CLASS_SPHEROID_DS:
  case CLASS_DS:
    return "Technician-Aerospace";
  }
  return NULL;
}

int find_tech_skill(DbRef player, Mech *mech) {
  const char *skname;

  skname = find_tech_skill_name(mech);
  if (skname)
    return (char_getskilltarget(mech->xcode.context, player, skname, 0));
  return 18;
}

int made_pilot_skill_roll(Mech *mech, int mods) {
  return mech_pilot_skill_roll(&(PilotSkillRollRequest){
      .mech = mech, .modifier = mods, .succeed_when_fallen = true});
}

int mech_pilot_skill_roll_target(Mech *mech, int mods) {
  mods += find_s_pilot_piloting(mech) + mech_pilot_skill_modifier(mech);
  if (((mech)->rd.specials2) & SMALLCOCKPIT_TECH)
    mods++;

  if (is_in_character(mech->xcode.context->database, mech->mynum) &&
      game_object_location(mech->xcode.context->database,
                           mech_pilot_dbref(mech)) != mech->mynum)
    mods += 5;
  return mods;
}

int mech_pilot_skill_roll_without_experience(
    const PilotSkillRollRequest *request) {
  Mech *mech = request->mech;
  int mods = request->modifier;
  int roll;
  int roll_needed;

  if (mech_is_fallen(mech) && request->succeed_when_fallen)
    return 1;
  if (mech_pilot_is_unconscious(mech) || !mech_is_started(mech) ||
      mech_is_blinded(mech))
    return 0;
  roll = btech_random_roll(mech->xcode.context);
  roll_needed = mech_pilot_skill_roll_target(mech, mods);

  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("Attempting to make pilot skill roll. "
                             "SPilot: %d, mods: %d, MechPilot: %d, BTH: %d",
                             find_s_pilot_piloting(mech), mods,
                             mech_pilot_skill_modifier(mech), roll_needed));

  mech_notify(mech, MECHPILOT, "You make a piloting skill roll!");
  mech_printf(mech, MECHPILOT, "Modified Pilot Skill: BTH %d\tRoll: %d",
              roll_needed, roll);
  if (roll >= roll_needed) {
    return 1;
  }
  return 0;
}

int mech_pilot_skill_roll(const PilotSkillRollRequest *request) {
  Mech *mech = request->mech;
  int mods = request->modifier;
  int roll;
  int roll_needed;

  if (mech_is_fallen(mech) && request->succeed_when_fallen)
    return 1;
  if (mech_pilot_is_unconscious(mech) || !mech_is_started(mech) ||
      mech_is_blinded(mech))
    return 0;
  roll = btech_random_roll(mech->xcode.context);
  roll_needed = mech_pilot_skill_roll_target(mech, mods);

  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("Attempting to make pilot (noxp) skill roll. "
                             "SPilot: %d, mods: %d, MechPilot: %d, BTH: %d",
                             find_s_pilot_piloting(mech), mods,
                             mech_pilot_skill_modifier(mech), roll_needed));

  mech_notify(mech, MECHPILOT, "You make a piloting skill roll!");
  mech_printf(mech, MECHPILOT, "Modified Pilot Skill: BTH %d\tRoll: %d",
              roll_needed, roll);
  if (roll >= roll_needed) {
    if (roll_needed > 2) {
      piloting_experience_award(&(PilotingExperienceAward){
          .pilot = mech_pilot_dbref(mech),
          .mech = mech,
          .reason = bounded(1, roll_needed - 7, max(2, 1 + mods)),
          .unconditional = true,
      });
    }
    return 1;
  }
  return 0;
}

MapRealPosition map_project_position(const MapProjection *projection) {
  float xscale;
  float correction;

  /* XXX: Something to do with ranges with actual number of hexes? */
  correction = (float)(projection->bearing % 60) / 60.0F;
  if (correction > 0.5F)
    correction = 1.0F - correction;
  correction = -correction * 2.0F; /* 0 - 1 correction */
  xscale = (1.0F + ((float)XSCALE * correction)) * (float)SCALEMAP;

  float radians = degrees_to_radians((float)projection->bearing);
  return (MapRealPosition){
      .x = projection->origin.x + (projection->range * sinf(radians) * xscale),
      .y = projection->origin.y -
           (projection->range * cosf(radians) * (float)SCALEMAP)};
}

/* Computes hex range between Cartesian (x0, y0, z0) and (x1, y1, z1).  */
