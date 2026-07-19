
/*
   p.map.coding.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:42 CET 1999 from map.coding.c */

#pragma once

/* map.coding.c */
typedef struct MapCodingRegistry MapCodingRegistry;

int map_coding_get_index(MapCodingRegistry *registry, char terrain,
                         char elevation);
char map_coding_get_elevation(const MapCodingRegistry *registry, int index);
char map_coding_get_terrain(const MapCodingRegistry *registry, int index);
