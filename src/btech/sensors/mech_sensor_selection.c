#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "map_conditions_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor.h"
#include "mech_sensor_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/network/network_output.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

#include <stdio.h>
#include <string.h>

typedef struct SensorModeText {
  char text[MBUF_SIZE];
} SensorModeText;

void mech_sensors_disable_requiring(Mech *mech, int technology) {
  int primary = mech_sensor_index(mech, 0);
  int secondary = mech_sensor_index(mech, 1);
  bool primary_requires =
      mech_sensor_definition(primary)->required_special == technology &&
      mech_sensor_definition(primary)->specials_set == 1;
  bool secondary_requires =
      mech_sensor_definition(secondary)->required_special == technology &&
      mech_sensor_definition(secondary)->specials_set == 1;

  if (!primary_requires && !secondary_requires)
    return;
  mech_sensors_set(mech, primary_requires ? 0 : primary,
                   secondary_requires ? 0 : secondary);
  MarkForLOSUpdate(mech);
}

typedef struct SensorModeTextRequest {
  Mech *mech;
  int sensor;
  bool full_arc;
  bool verbose;
} SensorModeTextRequest;

static SensorModeText sensor_mode_text(const SensorModeTextRequest *request) {
  Mech *mech = request->mech;
  const int sn = request->sensor;
  SensorModeText mode = {0};
  char *buf = mode.text;

  if (sn < 0 || (size_t)sn >= NUM_SENSORS) {
    (void)snprintf(buf, sizeof(mode.text), "None");
    return mode;
  }

  if (mech_sensor_definition(sn)->full_vision) {
    (void)snprintf(buf, sizeof(mode.text), "%s ",
                   mech_sensor_definition(sn)->sensor_name);
    MechSensorDescriptionRequest description = {
        .buffer = buf,
        .capacity = sizeof(mode.text),
        .mech = mech,
        .sensor = sn,
        .verbose = request->verbose,
    };
    mech_sensor_description_append(&description);
  } else {
    if (request->full_arc || mech_movement_type(mech) == MOVE_NONE ||
        mech_class(mech) == CLASS_BSUIT)
      (void)snprintf(buf, sizeof(mode.text), "%s in 360 degree scanning mode ",
                     mech_sensor_definition(sn)->sensor_name);
    else
      (void)snprintf(buf, sizeof(mode.text),
                     "%s in 120 degree scanning mode (Forward arc) ",
                     mech_sensor_definition(sn)->sensor_name);
    MechSensorDescriptionRequest description = {
        .buffer = buf,
        .capacity = sizeof(mode.text),
        .mech = mech,
        .sensor = sn,
        .verbose = request->verbose,
    };
    mech_sensor_description_append(&description);
  }
  return mode;
}
static void sensor_mode(Mech *mech, const char *msg, DbRef player, int p, int s,
                        int verbose) {
  char buf[MBUF_SIZE];

  if (p != s) {
    const size_t message_length = strlen(msg);
    const size_t line_length =
        message_length < sizeof(buf) - 1 ? message_length : sizeof(buf) - 1;
    memset(buf, '-', line_length);
    *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char), line_length) =
        '\0';
    mecha_notify(btech_context_evaluation(mech_context(mech)), player, msg);
    mecha_notify(btech_context_evaluation(mech_context(mech)), player, buf);
    notify_printf(
        btech_context_evaluation(mech_context(mech)), player, "Primary:   %s",
        sensor_mode_text(&(SensorModeTextRequest){
                             .mech = mech, .sensor = p, .verbose = verbose})
            .text);
    notify_printf(
        btech_context_evaluation(mech_context(mech)), player, "Secondary: %s",
        sensor_mode_text(&(SensorModeTextRequest){
                             .mech = mech, .sensor = s, .verbose = verbose})
            .text);
  } else
    notify_printf(btech_context_evaluation(mech_context(mech)), player,
                  "%s: %s", msg,
                  sensor_mode_text(&(SensorModeTextRequest){.mech = mech,
                                                            .sensor = p,
                                                            .full_arc = true,
                                                            .verbose = verbose})
                      .text);
}

typedef struct SensorSelection SensorSelection;
struct SensorSelection {
  int primary;
  int secondary;
  bool found;
};

static void sensor_selection_read(MuxEvent *event, void *data) {
  SensorSelection *selection = data;
  const long encoded = (long)event->data2;
  const long primary = encoded / NUM_SENSORS;
  const long secondary = encoded % NUM_SENSORS;

  if (primary < 0 || primary >= NUM_SENSORS || secondary < 0 ||
      secondary >= NUM_SENSORS)
    return;
  selection->primary = (int)primary;
  selection->secondary = (int)secondary;
  selection->found = true;
}

static const char SensorInf[] = "vliesrbVLIESRB";

char *mech_sensor_info(Mech *mech, char buffer[static LBUF_SIZE]) {
  SensorSelection selection = {0};

  *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), 0) =
      *checked_string_suffix(SensorInf, (size_t)mech_sensor_index(mech, 0));
  *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), 1) =
      *checked_string_suffix(SensorInf, (size_t)mech_sensor_index(mech, 1));
  if (mech_event_count(mech, EVENT_SCHANGE)) {
    mech_event_visit(mech, EVENT_SCHANGE, sensor_selection_read, &selection);
    if (selection.found) {
      *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), 2) =
          *checked_string_suffix(SensorInf, (size_t)selection.primary +
                                                (size_t)NUM_SENSORS);
      *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), 3) =
          *checked_string_suffix(SensorInf, (size_t)selection.secondary +
                                                (size_t)NUM_SENSORS);
      *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), 4) = '\0';
      return buffer;
    }
  }
  *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), 2) = '\0';
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
  const long d = (long)e->data2;
  Mech *mech = (Mech *)e->data;
  const long primary = d / NUM_SENSORS;
  const long secondary = d % NUM_SENSORS;

  if (primary < 0 || primary >= NUM_SENSORS || secondary < 0 ||
      secondary >= NUM_SENSORS)
    return;
  const int prim = (int)primary;
  const int sec = (int)secondary;

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

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map) {
    mech_notify(mech, MECHALL, "Where are you? ;-)");
    return 0;
  }
  /* < 0, means you you don't have the sensors if you _do_ have the bit
     > 0, means you have the sensors if you have the bit */
  i = mech_sensor_definition(s)->required_special;
  if (i) {
    if (!mech_supports_sensor_requirement(&(SensorCapabilityRequest){
            .mech = mech,
            .capability_set = mech_sensor_definition(s)->specials_set,
            .signed_capability = i})) {
      mech_printf(mech, MECHALL, "You lack the %s sensors!",
                  mech_sensor_definition(s)->sensor_name);
      return 0;
    }
  }

  if (mech_sensor_definition(s)->min_light >= 0 &&
      mech_sensor_definition(s)->min_light > battle_map_light(map)) {
    if (!mech_is_destroyed(mech) && mech_is_started(mech))
      mech_printf(mech, MECHALL, "It's now too dark to use %s!",
                  mech_sensor_definition(s)->sensor_name);
    return 0;
  }
  if (mech_sensor_definition(s)->max_light >= 0 &&
      mech_sensor_definition(s)->max_light < battle_map_light(map)) {
    if (!mech_is_destroyed(mech) && mech_is_started(mech))
      mech_printf(mech, MECHALL, "The light's kinda too bright now to use %s!",
                  mech_sensor_definition(s)->sensor_name);
    return 0;
  }

  switch (mech_sensor_definition(s)->attribute_check) {
  case SENSOR_ATTR_SEISMIC:
    if (mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT ||
        mech_class(mech) == CLASS_VEH_NAVAL ||
        mech_movement_type(mech) == MOVE_HOVER) {
      mech_printf(mech, MECHALL, "You lack the %s sensors!",
                  mech_sensor_definition(s)->sensor_name);
      return 0;
    }

    break;
  }

  return 1;
}

void sensor_light_availability_check(Mech *mech) {
  int p = mech_sensor_index(mech, 0), s = mech_sensor_index(mech, 1);
  int same = (p == s);

  if (mech_sensor_definition(p)->min_light >= 0 ||
      mech_sensor_definition(p)->max_light >= 0)
    if (!mech_sensor_can_change_to(mech, p))
      p = 0;

  if (!same && (mech_sensor_definition(s)->min_light >= 0 ||
                mech_sensor_definition(s)->max_light >= 0))
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
    if (*mech_sensor_definition(i)->match_letter == ps)
      prim = i;
    if (*mech_sensor_definition(i)->match_letter == ss)
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
  if (mech_class(mech) == CLASS_MW) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "You're using your eyes, and nothing you can do changes that!");
    return;
  }
  argc = mech_parseattributes(buffer, args, 2);
  if (argc > 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments!");
    return;
  }
  switch (argc) {
  case 0:
    show_sensor(player, mech, 0);
    break;
  case 1:
    show_sensor(player, mech, 1);
    break;
  case 2:
    const char primary = ascii_to_upper(*checked_string_suffix(args[0], 0));
    const char secondary = ascii_to_upper(*checked_string_suffix(args[1], 0));
    if (set_sensor(mech, primary, secondary) < 0) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid arguments!");
      return;
    }
    show_sensor(player, mech, 0);
    break;
  }
}
