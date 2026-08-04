#include "mech_sensor_internal.h"

typedef struct SensorModeText {
  char text[MBUF_SIZE];
} SensorModeText;

static SensorModeText sensor_mode_text(Mech *mech, int sn, int full,
                                       int verbose) {
  SensorModeText mode = {0};
  char *buf = mode.text;

  if (sn < 0 || (size_t)sn >= NUM_SENSORS) {
    snprintf(buf, sizeof(mode.text), "None");
    return mode;
  }

  if (sensors[sn].fullvision) {
    snprintf(buf, sizeof(mode.text), "%s ", sensors[sn].sensorname);
    add_sensor_info(buf, sizeof(mode.text), mech, sn, verbose);
  } else {
    if (full || Sees360(mech))
      snprintf(buf, sizeof(mode.text), "%s in 360 degree scanning mode ",
               sensors[sn].sensorname);
    else
      snprintf(buf, sizeof(mode.text),
               "%s in 120 degree scanning mode (Forward arc) ",
               sensors[sn].sensorname);
    add_sensor_info(buf, sizeof(mode.text), mech, sn, verbose);
  }
  return mode;
}
static void sensor_mode(Mech *mech, char *msg, DbRef player, int p, int s,
                        int verbose) {
  char buf[MBUF_SIZE];
  size_t i;

  if (p != s) {
    for (i = 0; i < strlen(msg); i++)
      buf[i] = '-';
    buf[strlen(msg)] = 0;
    notify(btech_context_evaluation(mech->xcode.context), player, msg);
    notify(btech_context_evaluation(mech->xcode.context), player, buf);
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Primary:   %s", sensor_mode_text(mech, p, 0, verbose).text);
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Secondary: %s", sensor_mode_text(mech, s, 0, verbose).text);
  } else
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "%s: %s", msg, sensor_mode_text(mech, p, 1, verbose).text);
}

typedef struct SensorSelection SensorSelection;
struct SensorSelection {
  int primary;
  int secondary;
  bool found;
};

static void sensor_selection_read(MuxEvent *event, void *data) {
  SensorSelection *selection = data;
  long encoded = (long)event->data2;

  selection->primary = encoded / NUM_SENSORS;
  selection->secondary = encoded % NUM_SENSORS;
  selection->found = true;
}

static const char SensorInf[] = "vliesrbVLIESRB";

char *mechSensorInfo(Mech *mech, char buffer[static LBUF_SIZE]) {
  SensorSelection selection = {0};

  buffer[0] = SensorInf[(short)MechSensor(mech)[0]];
  buffer[1] = SensorInf[(short)MechSensor(mech)[1]];
  if (mech_event_count(mech, EVENT_SCHANGE)) {
    mux_event_visit_type_data(mech->xcode.context->events, EVENT_SCHANGE, mech,
                              sensor_selection_read, &selection);
    if (selection.found) {
      buffer[2] = SensorInf[selection.primary + NUM_SENSORS];
      buffer[3] = SensorInf[selection.secondary + NUM_SENSORS];
      buffer[4] = '\0';
      return buffer;
    }
  }
  buffer[2] = '\0';
  return buffer;
}

static void show_sensor(DbRef player, Mech *mech, int verbose) {
  SensorSelection selection = {0};

  sensor_mode(mech, "Sensors", player, MechSensor(mech)[0], MechSensor(mech)[1],
              verbose);
  if (mech_event_count(mech, EVENT_SCHANGE)) {
    mux_event_visit_type_data(mech->xcode.context->events, EVENT_SCHANGE, mech,
                              sensor_selection_read, &selection);
    if (selection.found)
      sensor_mode(mech, "Wanted", player, selection.primary,
                  selection.secondary, 0);
  }
}

static void mech_sensorchange_event(MuxEvent *e) {
  long d = (long)e->data2;
  Mech *mech = (Mech *)e->data;
  int prim = d / NUM_SENSORS;
  int sec = d % NUM_SENSORS;

  if (!Started(mech))
    return;
  MechSensor(mech)[0] = prim;
  MechSensor(mech)[1] = sec;
  mech_notify(mech, MECHALL, "As your sensors change, your lock clears.");
  MechTarget(mech) = -1;
  MarkForLOSUpdate(mech);
}

int CanChangeTo(Mech *mech, int s) {
  BattleMap *map;
  int i;

  if (!(map = btech_context_get_map(mech->xcode.context, mech->mapindex))) {
    mech_notify(mech, MECHALL, "Where are you? ;-)");
    return 0;
  }
  /* < 0, means you you don't have the sensors if you _do_ have the bit
     > 0, means you have the sensors if you have the bit */
  if ((i = sensors[s].required_special)) {
    /* original specials struct */
    if (sensors[s].specials_set == 1) {
      if ((i > 0) == ((!(MechSpecials(mech) & abs(i))) != 0)) {
        mech_printf(mech, MECHALL, "You lack the %s sensors!",
                    sensors[s].sensorname);
        return 0;
      }
    } else {
      if ((i > 0) == ((!(MechSpecials2(mech) & abs(i))) != 0)) {
        mech_printf(mech, MECHALL, "You lack the %s sensors!",
                    sensors[s].sensorname);
        return 0;
      }
    }
  }

  if (sensors[s].min_light >= 0 && sensors[s].min_light > map->maplight) {
    if (!Destroyed(mech) && Started(mech))
      mech_printf(mech, MECHALL, "It's now too dark to use %s!",
                  sensors[s].sensorname);
    return 0;
  }
  if (sensors[s].max_light >= 0 && sensors[s].max_light < map->maplight) {
    if (!Destroyed(mech) && Started(mech))
      mech_printf(mech, MECHALL, "The light's kinda too bright now to use %s!",
                  sensors[s].sensorname);
    return 0;
  }

  switch (sensors[s].attributeCheck) {
  case SENSOR_ATTR_SEISMIC:
    if ((MechType(mech) == CLASS_MW) || (MechType(mech) == CLASS_BSUIT) ||
        (MechType(mech) == CLASS_VEH_NAVAL) || (MechMove(mech) == MOVE_HOVER)) {
      mech_printf(mech, MECHALL, "You lack the %s sensors!",
                  sensors[s].sensorname);
      return 0;
    }

    break;
  }

  return 1;
}

void sensor_light_availability_check(Mech *mech) {
  int p = MechSensor(mech)[0], s = MechSensor(mech)[1];
  int same = (p == s);

  if (sensors[p].min_light >= 0 || sensors[p].max_light >= 0)
    if (!CanChangeTo(mech, p))
      MechSensor(mech)[0] = 0;

  if (!same && (sensors[s].min_light >= 0 || sensors[s].max_light >= 0))
    if (!CanChangeTo(mech, s))
      MechSensor(mech)[1] = 0;
}

static int set_sensor(Mech *mech, char ps, char ss) {
  int prim = -1, sec = -1;
  int i;

  if (!Started(mech))
    return 0;
  for (i = 0; i < (int)NUM_SENSORS; i++) {
    if (sensors[i].matchletter[0] == ps)
      prim = i;
    if (sensors[i].matchletter[0] == ss)
      sec = i;
  }
  if (prim < 0 || sec < 0)
    return -1;
  if (prim != MechSensor(mech)[0] || sec != MechSensor(mech)[1]) {
    if (!CanChangeTo(mech, prim))
      return -1;
    if (!CanChangeTo(mech, sec))
      return -1;
    mech_event_cancel(mech, EVENT_SCHANGE);
    mech_event_schedule(mech, EVENT_SCHANGE, mech_sensorchange_event,
                        SCHANGE_TICK, ((prim * NUM_SENSORS) + sec));
  }
  return 0;
}

void mech_sensor(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[3];
  int argc;

  if (!mech)
    return;
  DOCHECK_CONTEXT(
      mech->xcode.context, MechType(mech) == CLASS_MW,
      "You're using your eyes, and nothing you can do changes that!");
  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(mech->xcode.context, argc > 2,
                  "Invalid number of arguments!");
  switch (argc) {
  case 0:
    show_sensor(player, mech, 0);
    break;
  case 1:
    show_sensor(player, mech, 1);
    break;
  case 2:
    DOCHECK_CONTEXT(mech->xcode.context,
                    set_sensor(mech, toupper(args[0][0]), toupper(args[1][0])) <
                        0,
                    "Invalid arguments!");
    show_sensor(player, mech, 0);
    break;
  }
}

void possibly_see_mech(Mech *mech) {
  BattleMap *map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  int i, j;
  Mech *seer;
  int mapvis;
  int maplight;
  float range;
  int num = mech->mapnumber;

  if (!map)
    return;
  mapvis = map->mapvis;
  maplight = map->maplight;
  /* This is quiet ; no message for noticing foe etc */
  /* Basically, this is a 'bonus' effect in addition to the movement-caused
     effects, but just done once / move of the guy */
  for (i = 0; i < map->first_free; i++)
    if (i != num && (j = map->mechsOnMap[i]) >= 0) {
      if (!(seer = btech_context_get_mech(mech->xcode.context, j)))
        continue;
      if (seer->mapindex != map->mynum) {
        btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                           tprintf("Mech #%ld was on map #%ld but with "
                                   "incorrect mapindex (%ld)",
                                   seer->mynum, map->mynum, seer->mapindex));
        map->mechsOnMap[i] = -1;
        continue;
      }
      range = FaMechRange(seer, mech);
      map->LOSinfo[i][num] =
          CalculateLOSFlag(seer, mech, map, MechX(mech), MechY(mech),
                           map->LOSinfo[i][num], (float)range);
    /* Then, we update the SEES* */
    /* seeanew used to be 2 ; I want them to know they notice
       it first not to bug me 'bout it, though */
#ifdef ADVANCED_LOS
      Sensor_DoWeSeeNow(seer, &map->LOSinfo[i][num], range, -1, -1, mech,
                        mapvis, maplight, map->cloudbase, 2, 0);
#endif
    }
}

static void mech_unblind_event(MuxEvent *e) {
  Mech *m = (Mech *)e->data;

  MechStatus(m) &= ~BLINDED;
  if (!Uncon(m))
    mech_notify(m, MECHALL, "Your sight recovers.");
}

void ScrambleInfraAndLiteAmp(Mech *mech, int time, int chance, char *inframsg,
                             char *liteampmsg) {
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  int i;
  Mech *tempMech;

  possibly_see_mech(mech);
  for (i = 0; i < mech_map->first_free; i++)
    if (mech_map->mechsOnMap[i] != -1 && mech_map->mechsOnMap[i] != mech->mynum)
      if ((tempMech = btech_context_get_mech(mech->xcode.context,
                                             mech_map->mechsOnMap[i])))
        if (InLineOfSight(tempMech, mech, MechX(mech), MechY(mech),
                          FaMechRange(tempMech, mech))) {
          if (Blinded(tempMech) || Uncon(tempMech))
            continue;
          if (sensors[(int)MechSensor(tempMech)[0]].matchletter[0] == 'I' ||
              sensors[(int)MechSensor(tempMech)[0]].matchletter[1] == 'I') {
            if (chance)
              if (btech_random_range(mech->xcode.context, 1, 100) > chance)
                continue;
            /* Infra effect */
            mech_notify(tempMech, MECHALL, inframsg);
          } else if (sensors[(int)MechSensor(tempMech)[0]].matchletter[0] ==
                         'L' ||
                     sensors[(int)MechSensor(tempMech)[0]].matchletter[1] ==
                         'L') {
            if (chance)
              if (btech_random_range(mech->xcode.context, 1, 100) > chance)
                continue;
            /* Liteamp effect */
            mech_notify(tempMech, MECHALL, liteampmsg);
          } else
            continue;
          MechStatus(tempMech) |= BLINDED;
          mech_event_schedule(tempMech, EVENT_BLINDREC, mech_unblind_event,
                              time, 0);
        }
}
