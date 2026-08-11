/* Function declarations / skill list for btechstats.c */

#pragma once

#include <time.h>

#include "mux/server/platform.h"

typedef struct CharacterValue {
  const char *name;
  char type;
  int flag;
  int xpthreshold;
} CharacterValue;

typedef struct BtechContext BtechContext;

typedef struct CharacterValueRequest {
  BtechContext *context;
  DbRef player;
  int code;
} CharacterValueRequest;

typedef struct CharacterValueChange {
  CharacterValueRequest target;
  int value;
} CharacterValueChange;

typedef struct CharacterExperienceChange {
  CharacterValueRequest target;
  int amount;
  bool override_interval;
} CharacterExperienceChange;

typedef struct CharacterSkillCheck {
  BtechContext *context;
  DbRef player;
  const char *name;
  int modifier;
  bool loud;
} CharacterSkillCheck;

enum { NUM_CHARVALUES = 119 };

constexpr int NUM_CHARLEVELS = 5;
constexpr int NUM_CHARTYPES = 6;
constexpr int NUM_CHARPACKAGES = 9;

extern const char *btech_charskillflag_names[4];
extern const char *char_levels[NUM_CHARLEVELS];
extern const char *char_types[NUM_CHARTYPES];
extern const char *char_packages[NUM_CHARPACKAGES];

/*
    XP is added only if the player is online AND
    the skill is marked SK_XP OR the last xp-gain is 30 seconds or more ago.
 */

typedef struct {
  DbRef db_ref;
  unsigned char value_storage[NUM_CHARVALUES];
  time_t last_use_storage[NUM_CHARVALUES];
  int xp_storage[NUM_CHARVALUES];
} PSTATS;
