#include "equipment_types.h"
#include "mech_radio_api.h"

#include <stdio.h>

#include "mech_internal.h"
#include "mux/support/checked_storage.h"

static bool mech_radio_channel_valid(int channel) {
  return channel >= 0 && channel < FREQS;
}

static int *mech_radio_int_slot(int *values, int channel) {
  return checked_storage_at(values, FREQS, sizeof(*values), (size_t)channel);
}

static const int *mech_radio_int_slot_const(const int *values, int channel) {
  return checked_storage_at_const(values, FREQS, sizeof(*values),
                                  (size_t)channel);
}

static char *mech_radio_title_slot(Mech *mech, int channel) {
  char (*title)[CHTITLELEN + 1] = checked_storage_at(
      mech->chantitle, FREQS, sizeof(*mech->chantitle), (size_t)channel);
  return *title;
}

static const char *mech_radio_title_slot_const(const Mech *mech, int channel) {
  const char (*title)[CHTITLELEN + 1] = checked_storage_at_const(
      mech->chantitle, FREQS, sizeof(*mech->chantitle), (size_t)channel);
  return *title;
}

int mech_radio_frequency(const Mech *mech, int channel) {
  return mech_radio_channel_valid(channel)
             ? *mech_radio_int_slot_const(mech->freq, channel)
             : 0;
}

int mech_radio_mode(const Mech *mech, int channel) {
  return mech_radio_channel_valid(channel)
             ? *mech_radio_int_slot_const(mech->freqmodes, channel)
             : 0;
}

int mech_radio_channel_count(const Mech *mech) {
  return mech->ud.radioinfo % FREQS;
}

int mech_radio_capabilities(const Mech *mech) {
  return mech->ud.radioinfo / FREQS;
}

const char *mech_radio_title(const Mech *mech, int channel) {
  return mech_radio_channel_valid(channel)
             ? mech_radio_title_slot_const(mech, channel)
             : "";
}

void mech_radio_frequency_set(Mech *mech, int channel, int frequency) {
  if (mech_radio_channel_valid(channel))
    *mech_radio_int_slot(mech->freq, channel) = frequency;
}

void mech_radio_frequency_add(Mech *mech, int channel, int amount) {
  if (mech_radio_channel_valid(channel))
    *mech_radio_int_slot(mech->freq, channel) += amount;
}

void mech_radio_mode_set(Mech *mech, int channel, int mode) {
  if (mech_radio_channel_valid(channel))
    *mech_radio_int_slot(mech->freqmodes, channel) = mode;
}

void mech_radio_title_set(Mech *mech, int channel, const char *title) {
  if (!mech_radio_channel_valid(channel))
    return;
  (void)snprintf(mech_radio_title_slot(mech, channel), CHTITLELEN + 1, "%.*s",
                 CHTITLELEN, title);
}
