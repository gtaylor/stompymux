#include "mech_api_types.h"
#include "mech_notify_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "registry_api.h"

#include <stdarg.h>
#include <stdio.h>

static void format_message(char message[LBUF_SIZE], const char *format,
                           va_list arguments)
    __attribute__((format(printf, 2, 0)));

static void format_message(char message[LBUF_SIZE], const char *format,
                           va_list arguments) {
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(message, LBUF_SIZE, format, arguments);
}

void mecha_notifyf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...) {
  char message[LBUF_SIZE];
  va_list arguments;

  va_start(arguments, format);
  format_message(message, format, arguments);
  va_end(arguments);
  mecha_notify(evaluation, player, message);
}

void mech_los_broadcastf(Mech *mech, const char *format, ...) {
  char message[LBUF_SIZE];
  va_list arguments;

  va_start(arguments, format);
  format_message(message, format, arguments);
  va_end(arguments);
  mech_los_broadcast(mech, message);
}

void mecha_notify_exceptf(const MechaNotificationExclusion *notification,
                          const char *format, ...) {
  char message[LBUF_SIZE];
  va_list arguments;

  va_start(arguments, format);
  format_message(message, format, arguments);
  va_end(arguments);
  MechaNotificationExclusion formatted = *notification;
  formatted.message = message;
  mecha_notify_except(&formatted);
}
