/* Event operations that do not depend on BTech object layouts. */

#include <stddef.h>
#include <stdint.h>

#include "btech_event.h"
#include "mux/network/mux_event.h"

static const char *const event_names[] = {
    "NONAME",     "Move",        "DHIT",       "Startup",    "Lock",
    "Stand",      "Jump",        "Recycle",    "JumpSt",     "PRecov",
    "SChange",    "DecRemv",     "SpotLck",    "PLos",       "ChkRng",
    "Takeoff",    "Fall",        "BRegen",     "BRebuild",   "Dump",
    "MASCF",      "MASCR",       "AmmoWarn",   "AutoGoto",   "AutoLeave",
    "AutoCo",     "AutoGun",     "AutoSensor", "AutoFollow", "AutoEnter",
    "AutoReply",  "AutoProfile", "AutoRoam",   "MRec",       "BlindR",
    "Burn",       "SixthS",      "Hidin",      "OOD",        "Misc",
    "Lateral",    "SelfExp",     "DigIn",      "TRepl",      "TReplG",
    "TReat",      "TRelo",       "TFix",       "TFixI",      "TScrL",
    "TScrP",      "TScrG",       "TRepaG",     "TRepaP",     "TMoB",
    "TUMoB",      "TRese",       "TRepSuit",   "TRepNHCrit", "59",
    "StandF",     "SliteC",      "HeatCutOff", "VechBurn",   "UnStunCrew",
    "StunCrew",   "UnJamTurret", "UnJamAmmo",  "StArmor",    "NSS",
    "TagRecycle", "RemPods",     "Extinguish", "EntHangar",  "Hulldown",
    "75",         "SchFail",     "SchRegen",   "CkStagger",  "MoveMode",
    "Sideslip",   nullptr};

static void *event_payload(intptr_t data) { return (void *)data; }

const char *btech_event_name(int type) {
  constexpr size_t event_name_count =
      sizeof(event_names) / sizeof(event_names[0]) - 1;
  if (type < 0 || (size_t)type >= event_name_count) {
    return event_names[0];
  }
  return event_names[type];
}

void btech_event_schedule(MuxEventScheduler *events, void *object, int type,
                          MuxEventCallback callback, int delay, intptr_t data) {
  mux_event_add(events, delay, 0, type, callback, object, event_payload(data));
}

int btech_event_count(MuxEventScheduler *events, const void *object, int type) {
  return mux_event_count_type_data(events, type, object);
}

int btech_event_count_data(MuxEventScheduler *events, const void *object,
                           int type, intptr_t data) {
  return mux_event_count_type_data_data(events, type, object,
                                        event_payload(data));
}

long btech_event_first_delay(MuxEventScheduler *events, const void *object,
                             int type) {
  return mux_event_count_type_data_firstev(events, type, object);
}

int btech_event_last_delay(MuxEventScheduler *events, const void *object,
                           int type) {
  return mux_event_last_type_data(events, type, object);
}

long btech_event_data(MuxEventScheduler *events, const void *object, int type) {
  long data = 0;
  mux_event_get_type_data(events, type, object, &data);
  return data;
}

void btech_event_cancel(MuxEventScheduler *events, void *object, int type) {
  mux_event_remove_type_data(events, type, object);
}

void btech_event_cancel_data(MuxEventScheduler *events, void *object, int type,
                             intptr_t data) {
  mux_event_remove_type_data_data(events, type, object, event_payload(data));
}

void btech_events_cancel_all(MuxEventScheduler *events, void *object) {
  mux_event_remove_data(events, object);
}
