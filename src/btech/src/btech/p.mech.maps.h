
/*
   p.mech.maps.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:14 CET 1999 from mech.maps.c */

#pragma once

typedef struct MapText MapText;

/* mech.maps.c */
void mech_findcenter(DbRef player, void *data, char *buffer);
const char *GetTerrainName_base(int t);
const char *GetTerrainName(MAP *map, int x, int y);
void mech_navigate(DbRef player, void *data, char *buffer);
char GetLRSMechChar(MECH *mech, MECH *tempMech);
void mech_lrsmap(DbRef player, void *data, char *buffer);
char *TerrainColor(char terrain, int elev);
void TacMapTerr(MAP *mech_map, int x, int y, char *terr, char *elev,
                int isdown);
MapText *map_text_create(DbRef player, MECH *mech, MAP *mech_map, int x, int y,
                         int xw, int yw, int labels, int dohexlos);
char *const *map_text_lines(const MapText *text);
void map_text_destroy(MapText *text);
void mech_tacmap(DbRef player, void *data, char *buffer);
void mech_enterbase(DbRef player, void *data, char *buffer);
