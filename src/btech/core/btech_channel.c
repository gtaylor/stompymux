/* Typed access to the diagnostic channels used by BTech. */

#include "btech_channel.h"

#include <stdarg.h>

#include "btech/context.h"
#include "mux/communication/comsys.h"

const char *btech_channel_name(BtechChannel channel) {
  switch (channel) {
  case BTECH_CHANNEL_SCEN_ERRORS:
    return "ScenErrors";
  case BTECH_CHANNEL_SCEN_STATUS:
    return "ScenStatus";
  case BTECH_CHANNEL_MECH_AI:
    return "MechAI";
  case BTECH_CHANNEL_MECH_CUSTOM:
    return "MechCustom";
  case BTECH_CHANNEL_DB_INFO:
    return "DBInfo";
  case BTECH_CHANNEL_MECH_DEBUG:
    return "MechDebugInfo";
  case BTECH_CHANNEL_MECH_DEATHS:
    return "MechDeaths";
  case BTECH_CHANNEL_MECH_ECON:
    return "MechEconInfo";
  case BTECH_CHANNEL_MECH_ERRORS:
    return "MechErrors";
  case BTECH_CHANNEL_MAP_ERRORS:
    return "MapErrors";
  case BTECH_CHANNEL_EVENT_INFO:
    return "EventInfo";
  case BTECH_CHANNEL_MECH_SENSOR:
    return "MechSensor";
  case BTECH_CHANNEL_MINE_TRIGGERS:
    return "MineTriggers";
  case BTECH_CHANNEL_MECH_XP:
    return "MechXP";
  case BTECH_CHANNEL_DS_INFO:
    return "DSInfo";
  case BTECH_CHANNEL_MECH_ATTACK_EMITS:
    return "MechAttackEmits";
  case BTECH_CHANNEL_MECH_ATTACKS:
    return "MechAttacks";
  case BTECH_CHANNEL_MECH_ATTACK_XP:
    return "MechAttackXP";
  case BTECH_CHANNEL_MECH_FREQS:
    return "MechFreqs";
  case BTECH_CHANNEL_MECH_PILOT_XP:
    return "MechPilotXP";
  case BTECH_CHANNEL_MECH_TECH_XP:
    return "MechTechXP";
  case BTECH_CHANNEL_TAC_INFO:
    return "TACInfo";
  case BTECH_CHANNEL_COUNT:
    return nullptr;
  }
  return nullptr;
}

void btech_channel_send(BtechContext *context, BtechChannel channel,
                        const char *format, ...) {
  va_list arguments;
  const char *channel_name = btech_channel_name(channel);

  if (channel_name == nullptr) {
    return;
  }

  va_start(arguments, format);
  send_channel_v(
      &(ChannelMessageTarget){.evaluation = btech_context_evaluation(context),
                              .channel = channel_name},
      format, arguments);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  va_end(arguments);
}
