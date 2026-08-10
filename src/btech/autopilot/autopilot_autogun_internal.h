#pragma once

typedef struct Autopilot Autopilot;
typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

void autopilot_autogun_fire(Autopilot *autopilot, Mech *mech, BattleMap *map,
                            Mech *target);
