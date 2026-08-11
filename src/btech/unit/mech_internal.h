/* Defines the internal BattleTech unit representation. */

#pragma once

#include "mech_state_types.h"
#include "special_object.h"
typedef struct Mech {
  BtechSpecialObject xcode; /* XCODE base class field */

  char id[2];                            /* Only for internal use */
  char brief;                            /* toggle brievity */
  char chantitle[FREQS][CHTITLELEN + 1]; /* Channel titles */
  DbRef mynum;                           /* My dbref */
  int mapnumber;                         /* My number on the map */
  DbRef mapindex;                        /* 0..MAX_MAPS (dbref of map object) */
  unsigned long tic[NUM_TICS][TICLONGS]; /* tics.. */
  int freq[FREQS];                       /* channel frequencies */
  int freqmodes[FREQS];                  /* flags for the freq */
  MechDefinitionState ud;                /* UnitData (mostly not bzero'able) */
  MechPositionState pd; /* PositionData(mostly not bzero'able) */
  MechRuntimeState rd;  /* RSdata (mostly bzero'able) */
  MechNetworkState sd;  /* SpecialsData (mostly not bzero'able) */

} Mech;

struct MechSpotData {
  float tar_fx;
  float tar_fy;
  float mech_fx;
  float mech_fy;
  Mech *target;
};

struct RepairData {
  int delta;
  int time;
  int target;
  int code;
};
