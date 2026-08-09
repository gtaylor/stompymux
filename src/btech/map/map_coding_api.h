/* Declares the BattleTech map coding API. */

#pragma once

/* map.coding.c */
typedef struct MapCodingRegistry MapCodingRegistry;

int map_coding_get_index(MapCodingRegistry *registry, char terrain,
                         char elevation);
char map_coding_get_elevation(const MapCodingRegistry *registry, int index);
char map_coding_get_terrain(const MapCodingRegistry *registry, int index);
