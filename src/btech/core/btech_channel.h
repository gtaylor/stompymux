/* Typed access to the diagnostic channels used by BTech. */

#pragma once

// IWYU pragma: no_include "context.h"

typedef struct BtechContext BtechContext;

typedef enum BtechChannel {
  BTECH_CHANNEL_SCEN_ERRORS,
  BTECH_CHANNEL_SCEN_STATUS,
  BTECH_CHANNEL_MECH_AI,
  BTECH_CHANNEL_MECH_CUSTOM,
  BTECH_CHANNEL_DB_INFO,
  BTECH_CHANNEL_MECH_DEBUG,
  BTECH_CHANNEL_MECH_DEATHS,
  BTECH_CHANNEL_MECH_ECON,
  BTECH_CHANNEL_MECH_ERRORS,
  BTECH_CHANNEL_MAP_ERRORS,
  BTECH_CHANNEL_EVENT_INFO,
  BTECH_CHANNEL_MECH_SENSOR,
  BTECH_CHANNEL_MINE_TRIGGERS,
  BTECH_CHANNEL_MECH_XP,
  BTECH_CHANNEL_DS_INFO,
  BTECH_CHANNEL_MECH_ATTACK_EMITS,
  BTECH_CHANNEL_MECH_ATTACKS,
  BTECH_CHANNEL_MECH_ATTACK_XP,
  BTECH_CHANNEL_MECH_FREQS,
  BTECH_CHANNEL_MECH_PILOT_XP,
  BTECH_CHANNEL_MECH_TECH_XP,
  BTECH_CHANNEL_TAC_INFO,
  BTECH_CHANNEL_COUNT,
} BtechChannel;

const char *btech_channel_name(BtechChannel channel);
void btech_channel_send(BtechContext *context, BtechChannel channel,
                        const char *format, ...)
    __attribute__((format(printf, 3, 4)));
