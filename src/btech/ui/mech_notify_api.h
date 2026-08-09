/* Declares the BattleTech unit notify API. */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"

typedef struct BattleMap BattleMap;
typedef struct EvaluationContext EvaluationContext;

typedef enum MechNotifyAudience {
  MECHPILOT,
  MECHSTARTED,
  MECHALL,
} MechNotifyAudience;

typedef struct MechDisplayId {
  char text[SBUF_SIZE];
} MechDisplayId;

/* mech.notify.c */
const char *GetAmmoDesc_Model_Mode(int model, int mode);
char GetWeaponAmmoModeLetter_Model_Mode(int model, unsigned int mode);
char GetWeaponFireModeLetter_Model_Mode(int model, int mode);
char GetWeaponAmmoModeLetter(Mech *mech, int loop, int crit);
char GetWeaponFireModeLetter(Mech *mech, int loop, int crit);
const char *GetMoveTypeID(int movetype);
void Mech_ShowFlags(EvaluationContext *evaluation, DbRef player, Mech *mech,
                    int spaces, int level);
const char *GetArcID(Mech *mech, int arc);
MechDisplayId mech_to_mech_display_id_base(Mech *see, Mech *mech, int inlos);
MechDisplayId mech_to_mech_display_id(Mech *see, Mech *mech);
MechDisplayId mech_display_id(Mech *mech);
void mech_set_channelfreq(DbRef player, void *data, char *buffer);
void mech_set_channeltitle(DbRef player, void *data, char *buffer);
void mech_set_channelmode(DbRef player, void *data, char *buffer);
void mech_list_freqs(DbRef player, void *data, char *buffer);
void mech_sendchannel(DbRef player, void *data, char *buffer);
int common_checks(DbRef player, Mech *mech, int flag);
void sendchannelstuff(Mech *mech, int freq, char *msg);
void mech_radio(DbRef player, void *data, char *buffer);
void MechBroadcast(Mech *mech, Mech *target, BattleMap *mech_map, char *buffer);
void mech_los_broadcast(Mech *mech, const char *message);
int MechSeesHexF(Mech *mech, BattleMap *map, float x, float y, int ix, int iy);
int MechSeesHex(Mech *mech, BattleMap *map, int x, int y);
void HexLOSBroadcast(BattleMap *mech_map, int x, int y, const char *message);
void mech_los_broadcast_unit(Mech *mech, Mech *target, const char *message);
void MapBroadcast(BattleMap *map, char *message);
void MechFireBroadcast(Mech *mech, Mech *target, int x, int y,
                       BattleMap *mech_map, const char *weapname, int IsHit);
void mech_notify(Mech *mech, MechNotifyAudience audience, const char *buffer);
void mech_printf(Mech *mech, MechNotifyAudience audience, const char *format,
                 ...) __attribute__((format(printf, 3, 4)));
