/* Declares the BattleTech unit notify API. */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"

typedef struct BattleMap BattleMap;
typedef struct EvaluationContext EvaluationContext;

typedef enum MechNotifyAudience : int {
  MECHPILOT,
  MECHSTARTED,
  MECHALL,
} MechNotifyAudience;

typedef struct MechDisplayId {
  char text[SBUF_SIZE];
} MechDisplayId;

/* mech.notify.c */
const char *get_ammo_desc_model_mode(int model, int mode);
char get_weapon_ammo_mode_letter_model_mode(int model, unsigned int mode);
char get_weapon_fire_mode_letter_model_mode(int model, int mode);
char get_weapon_ammo_mode_letter(Mech *mech, int loop, int crit);
char get_weapon_fire_mode_letter(Mech *mech, int loop, int crit);
const char *get_move_type_id(int movetype);
typedef struct MechFlagDisplayRequest {
  EvaluationContext *evaluation;
  DbRef player;
  Mech *mech;
  int indentation;
  int detail_level;
} MechFlagDisplayRequest;
void mech_show_flags(const MechFlagDisplayRequest *request);
const char *get_arc_id(Mech *mech, int arc);
MechDisplayId mech_to_mech_display_id_base(Mech *see, Mech *mech, int inlos);
MechDisplayId mech_to_mech_display_id(Mech *see, Mech *mech);
MechDisplayId mech_display_id(Mech *mech);
void mech_set_channelfreq(DbRef player, Mech *mech, char *buffer);
void mech_set_channeltitle(DbRef player, Mech *mech, char *buffer);
void mech_set_channelmode(DbRef player, Mech *mech, char *buffer);
void mech_list_freqs(DbRef player, Mech *mech, char *buffer);
void mech_sendchannel(DbRef player, Mech *mech, char *buffer);
bool common_checks(DbRef player, Mech *mech, int flag);
void sendchannelstuff(Mech *mech, int freq, char *msg);
void mech_radio(DbRef player, Mech *mech, char *buffer);
void mech_broadcast(Mech *mech, Mech *target, BattleMap *mech_map,
                    char *buffer);
void mech_los_broadcast(Mech *mech, const char *message);
void mech_los_broadcastf(Mech *mech, const char *format, ...)
    __attribute__((format(printf, 2, 3)));
int mech_sees_hex_f(Mech *mech, BattleMap *map, float x, float y, int ix,
                    int iy);
int mech_sees_hex(Mech *mech, BattleMap *map, int x, int y);
void hex_los_broadcast(BattleMap *mech_map, int x, int y, const char *message);
void mech_los_broadcast_unit(Mech *mech, Mech *target, const char *message);
void map_broadcast(BattleMap *map, char *message);
void mech_fire_broadcast(Mech *mech, Mech *target, int x, int y,
                         BattleMap *mech_map, const char *weapname, int is_hit);
void mech_notify(Mech *mech, MechNotifyAudience audience, const char *buffer);
void mech_printf(Mech *mech, MechNotifyAudience audience, const char *format,
                 ...) __attribute__((format(printf, 3, 4)));
