#include "mech_radio_api.h"

#include <string.h>

#include "btconfig.h"
#include "mech_internal.h"

static bool mech_radio_channel_valid(int channel) {
  return channel >= 0 && channel < FREQS;
}

int mech_radio_frequency(const Mech *mech, int channel) {
  return mech_radio_channel_valid(channel) ? mech->freq[channel] : 0;
}

int mech_radio_mode(const Mech *mech, int channel) {
  return mech_radio_channel_valid(channel) ? mech->freqmodes[channel] : 0;
}

int mech_radio_channel_count(const Mech *mech) {
  return mech->ud.radioinfo % FREQS;
}

int mech_radio_capabilities(const Mech *mech) {
  return mech->ud.radioinfo / FREQS;
}

const char *mech_radio_title(const Mech *mech, int channel) {
  return mech_radio_channel_valid(channel) ? mech->chantitle[channel] : "";
}

void mech_radio_frequency_set(Mech *mech, int channel, int frequency) {
  if (mech_radio_channel_valid(channel))
    mech->freq[channel] = frequency;
}

void mech_radio_mode_set(Mech *mech, int channel, int mode) {
  if (mech_radio_channel_valid(channel))
    mech->freqmodes[channel] = mode;
}

void mech_radio_title_set(Mech *mech, int channel, const char *title) {
  if (!mech_radio_channel_valid(channel))
    return;
  strncpy(mech->chantitle[channel], title, CHTITLELEN);
  mech->chantitle[channel][CHTITLELEN] = '\0';
}
