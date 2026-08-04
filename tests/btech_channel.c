#include "btech_channel.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "btech_context.h"
#include "mux/communication/comsys.h"

static int send_count;
static char sent_channel[32];
static char sent_message[128];

EvaluationContext *btech_context_evaluation(BtechContext *context) {
  return (EvaluationContext *)context;
}

void send_channel_v(EvaluationContext *evaluation, const char *channel,
                    const char *format, va_list arguments) {
  (void)evaluation;
  send_count++;
  snprintf(sent_channel, sizeof(sent_channel), "%s", channel);
  vsnprintf(sent_message, sizeof(sent_message), format, arguments);
}

int main(void) {
  BtechContext *context = (BtechContext *)1;

  if (strcmp(btech_channel_name(BTECH_CHANNEL_MECH_ERRORS), "MechErrors") ||
      strcmp(btech_channel_name(BTECH_CHANNEL_TAC_INFO), "TACInfo") ||
      btech_channel_name((BtechChannel)-1) != nullptr ||
      btech_channel_name(BTECH_CHANNEL_COUNT) != nullptr) {
    return 1;
  }

  btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "unit %d: %s", 17,
                     "ready");
  if (send_count != 1 || strcmp(sent_channel, "MechDebugInfo") ||
      strcmp(sent_message, "unit 17: ready")) {
    return 1;
  }

  btech_channel_send(context, BTECH_CHANNEL_COUNT, "ignored");
  return send_count == 1 ? 0 : 1;
}
