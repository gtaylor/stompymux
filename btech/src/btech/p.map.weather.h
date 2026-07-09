/* p.map.weather.h */

#pragma once

/* map.weather.c */

int validateWeatherConditions(int curConditions);
int calcWeatherEffects(MAP *map);
int calcWeatherGunEffects(MAP *map, int weapindx);
int calcWeatherPilotEffects(MECH *mech);
void setWeatherHeatEffects(MAP *map, MECH *mech);
void meltSnowAndIce(MAP *map, int x, int y, int depth, int emit, int makeSteam);
void growSnow(MAP *map, int lowDepth, int highDepth);
