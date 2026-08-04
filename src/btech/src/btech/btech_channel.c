/* Typed access to the diagnostic channels used by BTech. */

#include "btech_channel.h"

#include <stdarg.h>

#include "btech/btech_context.h"
#include "mux/communication/comsys.h"

static const char *const channel_names[BTECH_CHANNEL_COUNT] = {
    [BTECH_CHANNEL_SCEN_ERRORS] = "ScenErrors",
    [BTECH_CHANNEL_SCEN_STATUS] = "ScenStatus",
    [BTECH_CHANNEL_MECH_AI] = "MechAI",
    [BTECH_CHANNEL_MECH_CUSTOM] = "MechCustom",
    [BTECH_CHANNEL_DB_INFO] = "DBInfo",
    [BTECH_CHANNEL_MECH_DEBUG] = "MechDebugInfo",
    [BTECH_CHANNEL_MECH_DEATHS] = "MechDeaths",
    [BTECH_CHANNEL_MECH_ECON] = "MechEconInfo",
    [BTECH_CHANNEL_MECH_ERRORS] = "MechErrors",
    [BTECH_CHANNEL_MAP_ERRORS] = "MapErrors",
    [BTECH_CHANNEL_EVENT_INFO] = "EventInfo",
    [BTECH_CHANNEL_MECH_SENSOR] = "MechSensor",
    [BTECH_CHANNEL_MINE_TRIGGERS] = "MineTriggers",
    [BTECH_CHANNEL_MECH_XP] = "MechXP",
    [BTECH_CHANNEL_DS_INFO] = "DSInfo",
    [BTECH_CHANNEL_MECH_ATTACK_EMITS] = "MechAttackEmits",
    [BTECH_CHANNEL_MECH_ATTACKS] = "MechAttacks",
    [BTECH_CHANNEL_MECH_ATTACK_XP] = "MechAttackXP",
    [BTECH_CHANNEL_MECH_BTH_DEBUG] = "MechBTHDebug",
    [BTECH_CHANNEL_MECH_FREQS] = "MechFreqs",
    [BTECH_CHANNEL_MECH_PILOT_XP] = "MechPilotXP",
    [BTECH_CHANNEL_MECH_TECH_XP] = "MechTechXP",
    [BTECH_CHANNEL_TAC_INFO] = "TACInfo",
};

const char *btech_channel_name(BtechChannel channel) {
  if (channel < 0 || channel >= BTECH_CHANNEL_COUNT) {
    return nullptr;
  }
  return channel_names[channel];
}

void btech_channel_send(BtechContext *context, BtechChannel channel,
                        const char *format, ...) {
  va_list arguments;
  const char *channel_name = btech_channel_name(channel);

  if (channel_name == nullptr) {
    return;
  }

  va_start(arguments, format);
  send_channel_v(btech_context_evaluation(context), channel_name, format,
                 arguments);
  va_end(arguments);
}
