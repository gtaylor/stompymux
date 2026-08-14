#include "mech_notify_api.h"
#include "mux/support/alloc.h"
#include "registry_api.h"

#include <stddef.h>
#include <string.h>

static EvaluationContext *captured_evaluation;
static Mech *captured_mech;
static DbRef captured_player;
static MechaNotificationExclusion captured_exclusion;
static char captured_message[LBUF_SIZE];

static void capture_message(const char *message) {
  (void)snprintf(captured_message, sizeof(captured_message), "%s", message);
}

void mecha_notify(EvaluationContext *evaluation, DbRef player,
                  const char *message) {
  captured_evaluation = evaluation;
  captured_player = player;
  capture_message(message);
}

void mech_los_broadcast(Mech *mech, const char *message) {
  captured_mech = mech;
  capture_message(message);
}

void mecha_notify_except(const MechaNotificationExclusion *notification) {
  captured_exclusion = *notification;
  capture_message(notification->message);
  captured_exclusion.message = captured_message;
}

static bool long_message_is_truncated(void (*notify_long)(const char *),
                                      char fill) {
  char source[LBUF_SIZE + 64];
  memset(source, fill, sizeof(source) - 1);
  source[sizeof(source) - 1] = '\0';

  notify_long(source);
  return strlen(captured_message) == LBUF_SIZE - 1 &&
         captured_message[LBUF_SIZE - 2] == fill &&
         captured_message[LBUF_SIZE - 1] == '\0';
}

static EvaluationContext *test_evaluation(void) {
  static _Alignas(max_align_t) char storage;
  return (EvaluationContext *)&storage;
}

static Mech *test_mech(void) {
  static _Alignas(max_align_t) char storage;
  return (Mech *)&storage;
}

static void mecha_notify_long(const char *source) {
  mecha_notifyf(test_evaluation(), 42, "%s", source);
}

static void los_broadcast_long(const char *source) {
  mech_los_broadcastf(test_mech(), "%s", source);
}

static void notify_except_long(const char *source) {
  mecha_notify_exceptf(
      &(MechaNotificationExclusion){
          .evaluation = test_evaluation(),
          .location = 12,
          .actor = 13,
          .exception = 14,
          .message = "ignored",
      },
      "%s", source);
}

int main(void) {
  mecha_notifyf(test_evaluation(), 42, "unit %d at %s: 100%%", 7, "A1");
  if (captured_evaluation != test_evaluation() || captured_player != 42 ||
      strcmp(captured_message, "unit 7 at A1: 100%") != 0 ||
      !long_message_is_truncated(mecha_notify_long, 'm'))
    return 1;

  mech_los_broadcastf(test_mech(), "moves %d hexes: 50%%", 3);
  if (captured_mech != test_mech() ||
      strcmp(captured_message, "moves 3 hexes: 50%") != 0 ||
      !long_message_is_truncated(los_broadcast_long, 'l'))
    return 2;

  mecha_notify_exceptf(
      &(MechaNotificationExclusion){
          .evaluation = test_evaluation(),
          .location = 12,
          .actor = 13,
          .exception = 14,
          .message = "ignored",
      },
      "damage %d from %s: 25%%", 9, "B2");
  if (captured_exclusion.evaluation != test_evaluation() ||
      captured_exclusion.location != 12 || captured_exclusion.actor != 13 ||
      captured_exclusion.exception != 14 ||
      captured_exclusion.message != captured_message ||
      strcmp(captured_message, "damage 9 from B2: 25%") != 0 ||
      !long_message_is_truncated(notify_except_long, 'e'))
    return 3;
  return 0;
}
