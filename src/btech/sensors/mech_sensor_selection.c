#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map_conditions_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor.h"
#include "mech_sensor_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/network/network_output.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"

#include <ctype.h>
#include <stdio.h>
#include <string.h>

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
    if (full || mech_movement_type(mech) == MOVE_NONE ||
        mech_class(mech) == CLASS_BSUIT)
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
    notify(btech_context_evaluation(mech_context(mech)), player, msg);
    notify(btech_context_evaluation(mech_context(mech)), player, buf);
    notify_printf(btech_context_evaluation(mech_context(mech)), player,
                  "Primary:   %s", sensor_mode_text(mech, p, 0, verbose).text);
    notify_printf(btech_context_evaluation(mech_context(mech)), player,
                  "Secondary: %s", sensor_mode_text(mech, s, 0, verbose).text);
  } else
    notify_printf(btech_context_evaluation(mech_context(mech)), player,
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

char *mech_sensor_info(Mech *mech, char buffer[static LBUF_SIZE]) {
  SensorSelection selection = {0};

  buffer[0] = SensorInf[(short)mech_sensor_index(mech, 0)];
  buffer[1] = SensorInf[(short)mech_sensor_index(mech, 1)];
  if (mech_event_count(mech, EVENT_SCHANGE)) {
    mech_event_visit(mech, EVENT_SCHANGE, sensor_selection_read, &selection);
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

  sensor_mode(mech, "Sensors", player, mech_sensor_index(mech, 0),
              mech_sensor_index(mech, 1), verbose);
  if (mech_event_count(mech, EVENT_SCHANGE)) {
    mech_event_visit(mech, EVENT_SCHANGE, sensor_selection_read, &selection);
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

  if (!mech_is_started(mech))
    return;
  mech_sensors_set(mech, prim, sec);
  mech_notify(mech, MECHALL, "As your sensors change, your lock clears.");
  mech_targeting_target_clear(mech);
  MarkForLOSUpdate(mech);
}

int mech_sensor_can_change_to(Mech *mech, int s) {
  BattleMap *map;
  int i;

  if (!(map =
            btech_context_get_map(mech_context(mech), mech_map_dbref(mech)))) {
    mech_notify(mech, MECHALL, "Where are you? ;-)");
    return 0;
  }
  /* < 0, means you you don't have the sensors if you _do_ have the bit
     > 0, means you have the sensors if you have the bit */
  if ((i = sensors[s].required_special)) {
    if (!mech_supports_sensor_requirement(mech, sensors[s].specials_set, i)) {
      mech_printf(mech, MECHALL, "You lack the %s sensors!",
                  sensors[s].sensorname);
      return 0;
    }
  }

  if (sensors[s].min_light >= 0 &&
      sensors[s].min_light > battle_map_light(map)) {
    if (!mech_is_destroyed(mech) && mech_is_started(mech))
      mech_printf(mech, MECHALL, "It's now too dark to use %s!",
                  sensors[s].sensorname);
    return 0;
  }
  if (sensors[s].max_light >= 0 &&
      sensors[s].max_light < battle_map_light(map)) {
    if (!mech_is_destroyed(mech) && mech_is_started(mech))
      mech_printf(mech, MECHALL, "The light's kinda too bright now to use %s!",
                  sensors[s].sensorname);
    return 0;
  }

  switch (sensors[s].attributeCheck) {
  case SENSOR_ATTR_SEISMIC:
    if (mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT ||
        mech_class(mech) == CLASS_VEH_NAVAL ||
        mech_movement_type(mech) == MOVE_HOVER) {
      mech_printf(mech, MECHALL, "You lack the %s sensors!",
                  sensors[s].sensorname);
      return 0;
    }

    break;
  }

  return 1;
}

void sensor_light_availability_check(Mech *mech) {
  int p = mech_sensor_index(mech, 0), s = mech_sensor_index(mech, 1);
  int same = (p == s);

  if (sensors[p].min_light >= 0 || sensors[p].max_light >= 0)
    if (!mech_sensor_can_change_to(mech, p))
      p = 0;

  if (!same && (sensors[s].min_light >= 0 || sensors[s].max_light >= 0))
    if (!mech_sensor_can_change_to(mech, s))
      s = 0;
  mech_sensors_set(mech, p, s);
}

static int set_sensor(Mech *mech, char ps, char ss) {
  int prim = -1, sec = -1;
  int i;

  if (!mech_is_started(mech))
    return 0;
  for (i = 0; i < (int)NUM_SENSORS; i++) {
    if (sensors[i].matchletter[0] == ps)
      prim = i;
    if (sensors[i].matchletter[0] == ss)
      sec = i;
  }
  if (prim < 0 || sec < 0)
    return -1;
  if (prim != mech_sensor_index(mech, 0) || sec != mech_sensor_index(mech, 1)) {
    if (!mech_sensor_can_change_to(mech, prim))
      return -1;
    if (!mech_sensor_can_change_to(mech, sec))
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
      mech_context(mech), mech_class(mech) == CLASS_MW,
      "You're using your eyes, and nothing you can do changes that!");
  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(mech_context(mech), argc > 2, "Invalid number of arguments!");
  switch (argc) {
  case 0:
    show_sensor(player, mech, 0);
    break;
  case 1:
    show_sensor(player, mech, 1);
    break;
  case 2:
    DOCHECK_CONTEXT(mech_context(mech),
                    set_sensor(mech, toupper(args[0][0]), toupper(args[1][0])) <
                        0,
                    "Invalid arguments!");
    show_sensor(player, mech, 0);
    break;
  }
}
