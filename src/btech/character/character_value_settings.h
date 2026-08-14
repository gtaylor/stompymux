#pragma once

#include <stdbool.h>

#include "btechstats.h"

typedef struct BtechCharacterValueSettings {
  int xp_thresholds[NUM_CHARVALUES];
  bool initialized;
} BtechCharacterValueSettings;

typedef struct BtechContext BtechContext;
typedef struct CharacterValueThreshold {
  BtechContext *context;
  int code;
  int threshold;
} CharacterValueThreshold;

void btech_character_value_settings_initialize(
    BtechCharacterValueSettings *settings);
int character_value_xp_threshold(const BtechContext *context, int code);
void character_value_xp_threshold_set(const CharacterValueThreshold *value);
