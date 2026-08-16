/* Declares the BattleTech unit maps API. */

#pragma once

#include <stddef.h>

#include "mux/server/platform.h"

typedef struct MapText MapText;

typedef struct MapTextRequest {
  DbRef player;
  Mech *mech;
  BattleMap *map;
  int center_x;
  int center_y;
  int width;
  int height;
  int labels;
  bool calculate_los;
} MapTextRequest;

/* mech.maps.c */
void mech_findcenter(DbRef player, Mech *mech, char *buffer);
const char *get_terrain_name_base(int t);
const char *get_terrain_name(BattleMap *map, int x, int y);
void mech_navigate(DbRef player, Mech *mech, char *buffer);
char get_lrs_mech_char(Mech *mech, Mech *other);
void mech_lrsmap(DbRef player, Mech *mech, char *buffer);
char *terrain_color(char terrain, int elev);
void tac_map_terr(BattleMap *mech_map, int x, int y, char *terr, char *elev,
                  int isdown);
MapText *map_text_create(const MapTextRequest *request);
char *const *map_text_lines(const MapText *text);
size_t map_text_line_count(const MapText *text);
const char *map_text_line(const MapText *text, size_t index);
void map_text_destroy(MapText *text);
void mech_tacmap(DbRef player, Mech *mech, char *buffer);
void mech_enterbase(DbRef player, Mech *mech, char *buffer);
