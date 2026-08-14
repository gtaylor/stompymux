#include <stdarg.h>
#include <string.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_internal.h"
#include "mech_notify_api.h"
#include "mech_radio_api.h"
#include "mux/network/network_output.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

static int rejected_notifications;
static int accepted_notifications;

EvaluationContext *btech_context_evaluation(BtechContext *context) {
  return (EvaluationContext *)context;
}

BattleMap *btech_context_get_map(BtechContext *context, DbRef dbref) {
  (void)context;
  (void)dbref;
  return nullptr;
}

void *btech_context_find_object(BtechContext *context, DbRef dbref) {
  (void)context;
  (void)dbref;
  return nullptr;
}

void mecha_notify(EvaluationContext *evaluation, DbRef player,
                  const char *message) {
  (void)evaluation;
  (void)player;
  (void)message;
  rejected_notifications++;
}

void notify_printf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...) {
  (void)evaluation;
  (void)player;
  (void)format;
  accepted_notifications++;
}

void btech_channel_send(BtechContext *context, BtechChannel channel,
                        const char *format, ...) {
  (void)context;
  (void)channel;
  (void)format;
}

int mech_team(const Mech *mech) {
  (void)mech;
  return 0;
}

int battle_map_unit_count(const BattleMap *map) {
  (void)map;
  return 0;
}

DbRef battle_map_unit_dbref(const BattleMap *map, int index) {
  (void)map;
  (void)index;
  return NOTHING;
}

static bool frequency_is(Mech *mech, const char *input, int expected,
                         int rejected, int accepted) {
  char buffer[32];
  char *argument = nullptr;
  if (input) {
    strncpy(buffer, input, sizeof(buffer) - 1);
    buffer[sizeof(buffer) - 1] = '\0';
    argument = buffer;
  }
  mech_set_channelfreq(1, mech, argument);
  return mech_radio_frequency(mech, 0) == expected &&
         rejected_notifications == rejected &&
         accepted_notifications == accepted;
}

int main(void) {
  Mech mech = {.xcode.context = (BtechContext *)1,
               .mapindex = NOTHING,
               .ud.radioinfo = 8};
  mech_radio_frequency_set(&mech, 0, 77);

  if (!frequency_is(&mech, nullptr, 77, 1, 0) ||
      !frequency_is(&mech, "", 77, 2, 0) ||
      !frequency_is(&mech, "   ", 77, 3, 0) ||
      !frequency_is(&mech, "A=0", 0, 3, 1))
    return 1;

  mech_radio_frequency_set(&mech, 0, 77);
  if (!frequency_is(&mech, "A=+0", 77, 4, 1) ||
      !frequency_is(&mech, "A=-0", 77, 5, 1) ||
      !frequency_is(&mech, "A=12suffix", 77, 6, 1) ||
      !frequency_is(&mech, " A = 42", 42, 6, 2) ||
      !frequency_is(&mech, "A=-2", 42, 7, 2))
    return 2;

  return 0;
}
