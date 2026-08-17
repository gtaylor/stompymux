/* Declares the BattleTech unit tech API. */

#pragma once

#include "mux/server/platform.h"

typedef struct BtechContext BtechContext;

typedef struct TechTimeAddition {
  BtechContext *context;
  DbRef player;
  int units;
} TechTimeAddition;

typedef enum TechPartParseStatus : int {
  TECH_PART_PARSE_OK,
  TECH_PART_PARSE_INVALID,
  TECH_PART_PARSE_INVALID_POSITION,
} TechPartParseStatus;

typedef struct TechPartParseRequest {
  Mech *mech;
  char *text;
  bool parse_position;
  bool parse_extra;
  bool allow_rear;
} TechPartParseRequest;

typedef struct TechPartParseResult {
  TechPartParseStatus status;
  int location;
  int position;
  int extra;
} TechPartParseResult;

/* mech.tech.c */
int game_lag(BtechContext *context);
int game_lag_time(BtechContext *context, int duration);
int player_techtime(BtechContext *context, DbRef player);
int tech_roll(DbRef player, Mech *mech, int diff);
int tech_weapon_roll(DbRef player, Mech *mech, int diff);
int tech_addtechtime(const TechTimeAddition *addition);
int tech_time_scaled_seconds(BtechContext *context, int units);
int tech_adjusted_time_for_roll(BtechContext *context, int base_units,
                                int roll);
TechPartParseResult tech_part_parse(const TechPartParseRequest *request);
int tech_parsegun(Mech *mech, char *buffer, int *loc, int *pos, int *brand);
int figure_latest_tech_event(Mech *mech);
int tech_proper_armor_part(const Mech *mech);
int tech_proper_internal_part(const Mech *mech);
