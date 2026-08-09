/* Declares the BattleTech unit maps API. */

#pragma once

#include <stddef.h>

#include "mux/server/platform.h"

typedef struct MapText MapText;

/* mech.maps.c */
void mech_findcenter(DbRef player, void *data, char *buffer);
const char *GetTerrainName_base(int t);
const char *GetTerrainName(BattleMap *map, int x, int y);
void mech_navigate(DbRef player, void *data, char *buffer);
char GetLRSMechChar(Mech *mech, Mech *tempMech);
void mech_lrsmap(DbRef player, void *data, char *buffer);
char *TerrainColor(char terrain, int elev);
void TacMapTerr(BattleMap *mech_map, int x, int y, char *terr, char *elev,
                int isdown);
MapText *map_text_create(DbRef player, Mech *mech, BattleMap *mech_map, int x,
                         int y, int xw, int yw, int labels, int dohexlos);
char *const *map_text_lines(const MapText *text);
size_t map_text_line_count(const MapText *text);
const char *map_text_line(const MapText *text, size_t index);
void map_text_destroy(MapText *text);
void mech_tacmap(DbRef player, void *data, char *buffer);
void mech_enterbase(DbRef player, void *data, char *buffer);
