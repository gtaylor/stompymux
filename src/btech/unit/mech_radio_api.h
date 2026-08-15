#pragma once

#include "mech_api_types.h"

extern const char RADIO_COLORSTR[];

int mech_radio_frequency(const Mech *mech, int channel);
int mech_radio_mode(const Mech *mech, int channel);
int mech_radio_channel_count(const Mech *mech);
int mech_radio_capabilities(const Mech *mech);
const char *mech_radio_title(const Mech *mech, int channel);
void mech_radio_frequency_set(Mech *mech, int channel, int frequency);
void mech_radio_frequency_add(Mech *mech, int channel, int amount);
void mech_radio_mode_set(Mech *mech, int channel, int mode);
void mech_radio_title_set(Mech *mech, int channel, const char *title);
