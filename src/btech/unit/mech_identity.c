#include "mech_utils_internal.h"

static char *const mechtypenames[CLASS_LAST + 1] = {
    "mech", "tank", "VTOL", "vessel", "aerofighter", "DropShip"};

const char *mechtypename(Mech *foo) {
  return mechtypenames[(int)MechType(foo)];
}

int mech_armorpoints(Mech *mech) {
  int i;
  int points = 0;

  for (i = 0; i < NUM_SECTIONS; i++) {
    points += GetSectArmor(mech, i);
    points += GetSectRArmor(mech, i);
  }

  return points;
}

int mech_intpoints(Mech *mech) {
  int i;
  int points = 0;

  for (i = 0; i < NUM_SECTIONS; i++) {
    points += GetSectInt(mech, i);
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

int MNumber(Mech *mech, int low, int high) {
  if ((mech->xcode.context->events->tick / RANDOM_TICK) != MechLastRndU(mech)) {
    MechRnd(mech) = (int)btech_random_i31(&mech->xcode.context->random);
    MechLastRndU(mech) = mech->xcode.context->events->tick / RANDOM_TICK;
  }
  return (low + MechRnd(mech) % (high - low + 1));
}

MechId mech_id(Mech *mech, bool lowercase) {
  MechId id;

  if (mech) {
    id.text[0] = MechID(mech)[0];
    id.text[1] = MechID(mech)[1];
  } else {
    id.text[0] = '*';
    id.text[1] = '*';
  }
  id.text[2] = '\0';

  if (lowercase) {
    id.text[0] = tolower((unsigned char)id.text[0]);
    id.text[1] = tolower((unsigned char)id.text[1]);
  }
  return id;
}

char *MyToUpper(char *string) {
  if (*string)
    *string = toupper(*string);
  return string;
}

int CritsInLoc(Mech *mech, int index) {
  if (MechType(mech) == CLASS_MECH)
    switch (index) {
    case HEAD:
    case RLEG:
    case LLEG:
      return 6;
    case RARM:
    case LARM:
      if (MechIsQuad(mech))
        return 6;
    }
  else if (MechType(mech) == CLASS_MW)
    return 2;
  return NUM_CRITICALS;
}

int SectHasBusyWeap(Mech *mech, int sect) {
  int i = 0, count, critical[MAX_WEAPS_SECTION];
  unsigned char weaptype[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];

  count = FindWeapons(mech, sect, weaptype, weapdata, critical);
  for (i = 0; i < count; i++)
    if (WpnIsRecycling(mech, sect, critical[i]))
      return 1;
  return 0;
}

BattleMap *ValidMap(BtechContext *context, DbRef player, DbRef map) {
  char *str;
  BattleMap *maps;

  DOCHECKN_CONTEXT(context, !is_good_obj(context->database, map),
                   "Index out of range!");
  str = btech_attribute_read(context->database, map, A_XTYPE,
                             (char[LBUF_SIZE]){0});
  DOCHECKN_CONTEXT(context, !str || !*str,
                   "That is not a valid map! (no XTYPE!)");
  DOCHECKN_CONTEXT(context, strcmp("MAP", str), "That is not a valid map!");
  DOCHECKN_CONTEXT(context, !(maps = btech_context_get_map(context, map)),
                   "The map has not been allocated!!");
  return maps;
}

DbRef FindMechOnMap(BattleMap *map, char *mechid) {
  int loop;
  Mech *tempMech;

  for (loop = 0; loop < map->first_free; loop++)
    if (map->mechsOnMap[loop] != -1) {
      tempMech =
          btech_context_get_mech(map->xcode.context, map->mechsOnMap[loop]);
      if (tempMech && !strncasecmp(MechID(tempMech), mechid, 2))
        return tempMech->mynum;
    }
  return -1;
}

DbRef FindTargetDBREFFromMapNumber(Mech *mech, char *mapnum) {
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
  return FindMechOnMap(map, mapnum);
}

void FindComponents(float magnitude, int degrees, float *x, float *y) {
  *x = magnitude * fcos((float)(TWOPIOVER360 * (degrees + 90)));
  *y = magnitude * fsin((float)(TWOPIOVER360 * (degrees + 90)));
  *x = -(*x); /* because 90 is to the right */
  *y = -(*y); /* because y increases downwards */
}

static int Leave_Hangar(BattleMap *map, Mech *mech) {
  Mech *car = NULL;
  int mapob;
  MapObject *mapo;

  /* For now, leaving leads to finding yourself on the new map
     at a predetermined position */
  mapob = mech->mapindex;
  if (MechCarrying(mech) > 0)
    car = btech_context_get_mech(mech->xcode.context, MechCarrying(mech));
  DOCHECKMA0(!map->cf, "The entrance is still filled with rubble!");
  mech_los_broadcast(mech, "has left the hangar.");
  mech_Rsetmapindex(GOD, (void *)mech,
                    tprintf("%d", (int)map->MapObject[TYPE_LEAVE]->obj));
  if (car)
    mech_Rsetmapindex(GOD, (void *)car,
                      tprintf("%d", (int)map->MapObject[TYPE_LEAVE]->obj));
  map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  if (mech->mapindex == mapob) {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("#%ld %s attempted to leave, but no target map?",
                               mech->mynum, mech_display_id(mech).text));
    mech_notify(mech, MECHALL,
                "Exit of this map is.. fubared. Please contact a wizard");
    return 0;
  }
  if (!(mapo = find_entrance_by_target(map, mapob))) {
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
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", mapo->x, mapo->y));
  mech_continue_flying(mech);
  if (car)
    MirrorPosition(mech, car, 0);
  mech_los_broadcast(
      mech, tprintf("has left %s at %d,%d.",
                    structure_name(mech->xcode.context->database, mapo).text,
                    MechX(mech), MechY(mech)));
  move_via_teleport(btech_context_evaluation(mech->xcode.context), mech->mynum,
                    mech->mapindex, 1, 0);
  if (car)
    move_via_teleport(btech_context_evaluation(mech->xcode.context), car->mynum,
                      mech->mapindex, 1, 0);
  if (is_in_character(mech->xcode.context->database, mech->mynum) &&
      game_object_location(mech->xcode.context->database, MechPilot(mech)) !=
          mech->mynum) {
    mech_notify(
        mech, MECHALL,
        "[fg=red bold blink inverse]INTRUDER ALERT! INTRUDER ALERT![reset]");
    mech_notify(mech, MECHALL,
                "[fg=red bold blink]Automatic self-destruct sequence "
                "initiated...[reset]");
    mech_shutdown(GOD, (void *)mech, "");
  }
  auto_cal_mapindex(mech->xcode.context, mech);
  if (MechSpeed(mech) > MMaxSpeed(mech))
    MechSpeed(mech) = MMaxSpeed(mech);
  return 1;
}

void CheckEdgeOfMap(Mech *mech) {
  int pinned = 0;
  int linked;
  BattleMap *map;

  map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  if (!map) {
    mech_notify(mech, MECHPILOT, "You are on an invalid map! Map index reset!");
    mech_shutdown(MechPilot(mech), (void *)mech, "");
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
  if (MechX(mech) < 0) {
    if (linked) {
      MechX(mech) += map->map_width;
      pinned = -1;
    } else {
      MechX(mech) = 0;
      pinned = 4;
    }
  } else if (MechX(mech) >= map->map_width) {
    if (linked) {
      MechX(mech) -= map->map_width;
      pinned = -1;
    } else {
      MechX(mech) = map->map_width - 1;
      pinned = 2;
    }
  }
  if (MechY(mech) < 0) {
    if (linked) {
      pinned = -1;
      MechY(mech) += map->map_height;
    } else {
      MechY(mech) = 0;
      pinned = 1;
    }
  } else if (MechY(mech) >= map->map_height) {
    if (linked) {
      pinned = -1;
      MechY(mech) -= map->map_height;
    } else {
      MechY(mech) = map->map_height - 1;
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
    } else if (map->flags & MAPFLAG_MAPO && map->MapObject[TYPE_LEAVE])
      if (Leave_Hangar(map, mech))
        return;
  }
  if (pinned) {
    MapCoordToRealCoord(MechX(mech), MechY(mech), &MechFX(mech), &MechFY(mech));
    if (pinned > 0) {
      mech_notify(mech, MECHALL, "You cannot move off this map!");
      if (Jumping(mech) && !is_aero(mech))
        mech_jump_land(mech);
      MechCocoon(mech) = 0;
      MechSpeed(mech) = 0.0;
      MechDesiredSpeed(mech) = 0.0;
      if (is_aero(mech)) {
        MechStartFX(mech) = 0.0;
        MechStartFY(mech) = 0.0;
        MechStartFZ(mech) = 0.0;
        if (!Landed(mech))
          mech_maybe_move(mech);
      }
    }
  }
}
int FindZBearing(float x0, float y0, float z0, float x1, float y1, float z1) {
  float adj, opp, deg;

  adj = FindXYRange(x0, y0, x1, y1);
  /*
   * XXX: Why can't opp be negative?  If z1 < z0, shouldn't Z-bearing
   * also be negative?  Also, why no range clamping on the value of deg?
   */
  opp = (float)(1. / SCALEMAP) * fabsf(z1 - z0);
  /* TODO: Use atan2f(), if we've got it.  */
  deg = RAD2DEG(atan2(opp, adj));
  return ceilf(deg);
}

int FindBearing(float x0, float y0, float x1, float y1) {
  const float dx = x1 - x0;
  const float dy = y1 - y0;

  float rads;
  int degrees;

  /*
   * atan2() doesn't need this check because we never actually divide by
   * dx, but we handle it specially for consistency with existing code.
   */
  if (dx == 0.f) {
    return (dy < 0.f) ? 0 : 180;
  }

  /* TODO: Use atan2f(), if we've got it.  */
  rads = (float)atan2(-dx, dy);

  /* Round off degrees.  */
  degrees = ((int)RAD2DEG(10.f * rads) + 5) / 10;

  return AcceptableDegree(degrees + 180);
}

int InWeaponArc(Mech *mech, float x, float y) {
  int relat;
  int bearingToTarget;
  int res = NOARC;

  bearingToTarget = FindBearing(MechFX(mech), MechFY(mech), x, y);
  relat = MechFacing(mech) - bearingToTarget;
  if (MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW ||
      MechType(mech) == CLASS_BSUIT) {
    if (MechStatus(mech) & TORSO_RIGHT)
      relat += 59;
    else if (MechStatus(mech) & TORSO_LEFT)
      relat -= 59;
  }
  relat = AcceptableDegree(relat);
  if (relat >= 300 || relat <= 60)
    res |= FORWARDARC;
  if (relat > 120 && relat < 240)
    res |= REARARC;
  if (relat >= 240 && relat < 300)
    res |= RSIDEARC;
  if (relat > 60 && relat <= 120)
    res |= LSIDEARC;

  if (MechHasTurret(mech)) {
    relat = AcceptableDegree((MechFacing(mech) + MechTurretFacing(mech)) -
                             bearingToTarget);
    if (relat >= 330 || relat <= 30)
      res |= TURRETARC;
  }
  if (res == NOARC)
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("NoArc: #%ld: BearingToTarget:%d Facing:%d",
                               mech->mynum, bearingToTarget, MechFacing(mech)));
  return res;
}

char *FindGunnerySkillName(Mech *mech, int weapindx) {
  if (!mech->xcode.context->configuration->btech_extended_gunnery) {
    switch (MechType(mech)) {
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
        if (!strcmp(MechWeapons[weapindx].name, "PC.Sword"))
          return "Blade";
        if (!strcmp(MechWeapons[weapindx].name, "PC.Vibroblade"))
          return "Blade";
      }
      return "Small_Arms";
    }
  } else {

    if (weapindx < 0)
      return NULL;
    if (MechType(mech) == CLASS_MW) {
      if (weapindx >= 0) {
        if (!strcmp(MechWeapons[weapindx].name, "PC.Blade"))
          return "Blade";
        if (!strcmp(MechWeapons[weapindx].name, "PC.Vibroblade"))
          return "Blade";
        if (!strcmp(MechWeapons[weapindx].name, "PC.Blazer"))
          return "Support_Weapons";
        if (!strcmp(MechWeapons[weapindx].name, "PC.HeavyGyrojetGun"))
          return "Support_Weapons";
        return "Small_Arms";
      }
    } else if (IsArtillery(weapindx))
      return "Gunnery-Artillery";
    else if (IsMissile(weapindx))
      return "Gunnery-Missile";
    else if (IsBallistic(weapindx))
      return "Gunnery-Ballistic";
    else if (IsEnergy(weapindx))
      return "Gunnery-Laser";
    else if (IsFlamer(weapindx))
      return "Gunnery-Flamer";
  }
  return NULL;
}

char *FindPilotingSkillName(Mech *mech) {
  if (!mech->xcode.context->configuration->btech_extended_piloting) {
    switch (MechType(mech)) {
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

    if (MechType(mech) == CLASS_MW && mech_real_terrain_get(mech) == WATER)
      return "Swimming";
    switch (MechType(mech)) {
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
    switch (MechMove(mech)) {
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

int FindPilotPiloting(Mech *mech) {
  char *str;

  if (mech_has_active_pilot(mech))
    if ((str = FindPilotingSkillName(mech)))
      return char_getskilltarget(mech->xcode.context, MechPilot(mech), str, 0);
  return DEFAULT_PILOTING;
}

int FindSPilotPiloting(Mech *mech) {
  return FindPilotPiloting(mech) + (MechMove(mech) == MOVE_QUAD ? -2 : 0);
}

int FindPilotSpotting(Mech *mech) {
  if (mech_has_active_pilot(mech))
    return (char_getskilltarget(mech->xcode.context, MechPilot(mech),
                                "Gunnery-Spotting", 0));
  return DEFAULT_SPOTTING;
}

int FindPilotArtyGun(Mech *mech) {
  if (mech_has_active_gunner(mech))
    return (char_getskilltarget(mech->xcode.context, GunPilot(mech),
                                "Gunnery-Artillery", 0));
  return DEFAULT_ARTILLERY;
}

int FindPilotGunnery(Mech *mech, int weapindx) {
  char *str;

  if (mech_has_active_gunner(mech))
    if ((str = FindGunnerySkillName(mech, weapindx)))
      return char_getskilltarget(mech->xcode.context, GunPilot(mech), str, 0);
  return DEFAULT_GUNNERY;
}

char *FindTechSkillName(Mech *mech) {
  switch (MechType(mech)) {
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

int FindTechSkill(DbRef player, Mech *mech) {
  char *skname;

  if ((skname = FindTechSkillName(mech)))
    return (char_getskilltarget(mech->xcode.context, player, skname, 0));
  return 18;
}

int MadePilotSkillRoll(Mech *mech, int mods) {
  return MadePilotSkillRoll_Advanced(mech, mods, 1);
}

int mech_pilot_skill_roll_target(Mech *mech, int mods) {
  mods += FindSPilotPiloting(mech) + MechPilotSkillBase(mech);
  if (MechSpecials2(mech) & SMALLCOCKPIT_TECH)
    mods++;

  if (is_in_character(mech->xcode.context->database, mech->mynum) &&
      game_object_location(mech->xcode.context->database, MechPilot(mech)) !=
          mech->mynum)
    mods += 5;
  return mods;
}

int MadePilotSkillRoll_NoXP(Mech *mech, int mods, int succeedWhenFallen) {
  int roll, roll_needed;

  if (Fallen(mech) && succeedWhenFallen)
    return 1;
  if (Uncon(mech) || !Started(mech) || Blinded(mech))
    return 0;
  roll = btech_random_roll(mech->xcode.context);
  roll_needed = mech_pilot_skill_roll_target(mech, mods);

  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("Attempting to make pilot skill roll. "
                             "SPilot: %d, mods: %d, MechPilot: %d, BTH: %d",
                             FindSPilotPiloting(mech), mods,
                             MechPilotSkillBase(mech), roll_needed));

  mech_notify(mech, MECHPILOT, "You make a piloting skill roll!");
  mech_printf(mech, MECHPILOT, "Modified Pilot Skill: BTH %d\tRoll: %d",
              roll_needed, roll);
  if (roll >= roll_needed) {
    return 1;
  }
  return 0;
}

int MadePilotSkillRoll_Advanced(Mech *mech, int mods, int succeedWhenFallen) {
  int roll, roll_needed;

  if (Fallen(mech) && succeedWhenFallen)
    return 1;
  if (Uncon(mech) || !Started(mech) || Blinded(mech))
    return 0;
  roll = btech_random_roll(mech->xcode.context);
  roll_needed = mech_pilot_skill_roll_target(mech, mods);

  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("Attempting to make pilot (noxp) skill roll. "
                             "SPilot: %d, mods: %d, MechPilot: %d, BTH: %d",
                             FindSPilotPiloting(mech), mods,
                             MechPilotSkillBase(mech), roll_needed));

  mech_notify(mech, MECHPILOT, "You make a piloting skill roll!");
  mech_printf(mech, MECHPILOT, "Modified Pilot Skill: BTH %d\tRoll: %d",
              roll_needed, roll);
  if (roll >= roll_needed) {
    if (roll_needed > 2)
      AccumulatePilXP(MechPilot(mech), mech,
                      BOUNDED(1, roll_needed - 7, MAX(2, 1 + mods)), 1);
    return 1;
  }
  return 0;
}

void FindXY(float x0, float y0, int bearing, float range, float *x1,
            float *y1) {
  float xscale, correction;

  /* XXX: Something to do with ranges with actual number of hexes? */
  correction = (float)(bearing % 60) / 60.0;
  if (correction > 0.5)
    correction = 1.0 - correction;
  correction = -correction * 2.0; /* 0 - 1 correction */
  xscale = (1.0 + XSCALE * correction) * SCALEMAP;

  /* TODO: Use sinf()/cosf(), if we've got them.  */
  *x1 = x0 + range * (float)sin(DEG2RAD(bearing)) * xscale;
  *y1 = y0 - range * (float)cos(DEG2RAD(bearing)) * SCALEMAP;
}

/* Computes hex range between Cartesian (x0, y0, z0) and (x1, y1, z1).  */
