#pragma once

#include <stdbool.h>
#include <time.h>

#include "btech/context.h"
#include "btechstats.h"
#include "character_value_settings.h"
#include "mux/server/platform.h"

int character_value_by_code(const CharacterValueRequest *request);
void character_stats_clear(PSTATS *stats);
void character_stats_retrieve(BtechContext *context, DbRef player, int modes,
                              PSTATS *stats);
void character_stats_store(BtechContext *context, DbRef player, PSTATS *stats,
                           int modes);
int character_xp_to_next_level(BtechContext *context, DbRef target, int code);
PSTATS *character_stats_create(void);
const CharacterValue *character_value_definition(int code);
const char *character_value_type_name(int type);
unsigned char character_stats_value_get(const PSTATS *stats, int code);
typedef struct CharacterStatsValueChange {
  PSTATS *stats;
  int code;
  int value;
} CharacterStatsValueChange;
void character_stats_value_set(const CharacterStatsValueChange *change);
int character_stats_xp_get(const PSTATS *stats, int code);
typedef struct CharacterStatsExperienceChange {
  PSTATS *stats;
  int code;
  int value;
} CharacterStatsExperienceChange;
void character_stats_xp_set(const CharacterStatsExperienceChange *change);
time_t character_stats_last_use_get(const PSTATS *stats, int code);
typedef struct CharacterStatsLastUseChange {
  PSTATS *stats;
  int code;
  time_t value;
} CharacterStatsLastUseChange;
void character_stats_last_use_set(const CharacterStatsLastUseChange *change);
int char_getstatvalue(PSTATS *stats, const char *name);
void char_setstatvalue(PSTATS *stats, const char *name, int value);
int figure_xp_bonus(BtechContext *context, DbRef player, PSTATS *stats,
                    int code);
void character_value_set_by_code(const CharacterValueChange *change);

enum { MAX_PLAYERS_ON = 10000 };
