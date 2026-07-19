
/*
   p.mech.status.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:16 CET 1999 from mech.status.c */

#include "mux/server/platform.h"

#pragma once

typedef struct BtechContext BtechContext;
typedef struct EvaluationContext EvaluationContext;

typedef struct PartDisplayName {
  char text[SBUF_SIZE];
  bool valid;
} PartDisplayName;

/*
 * Armor status flags for ArmorEvaluateSerious().
 *
 * TODO: Can probably coalesce some of these with other subsystems.
 */
#define ARMOR_TYPE_MASK 0x07
#define ARMOR_FRONT 0x00    /* front armor */
#define ARMOR_INTERNAL 0x01 /* internal armor */
#define ARMOR_REAR 0x02     /* rear armor */

#define ARMOR_FLAG_OWNED 0x10     /* armor status by owner */
#define ARMOR_FLAG_SHOW_DEST 0x20 /* show destroyed sections */
#define ARMOR_FLAG_DIVIDE_10 0x40 /* divide displayed value by 10 */

/*
 * Armor levels returned by ArmorEvaluateSerious().
 */
#define ARMOR_LEVEL_GREAT 0
#define ARMOR_LEVEL_GOOD 1
#define ARMOR_LEVEL_LOW 2
#define ARMOR_LEVEL_CRITICAL 3
#define ARMOR_LEVEL_OPEN 4
#define ARMOR_LEVEL_REPAIRING 5

/* mech.status.c */
void DisplayTarget(EvaluationContext *evaluation, DbRef player, MECH *mech);
void show_miscbrands(MECH *mech, DbRef player);
void PrintGenericStatus(EvaluationContext *evaluation, DbRef player, MECH *mech,
                        int own, int usex);
void PrintHeatBar(EvaluationContext *evaluation, DbRef player, MECH *mech);
void PrintInfoStatus(EvaluationContext *evaluation, DbRef player, MECH *mech,
                     int own);
void PrintShortInfo(EvaluationContext *evaluation, DbRef player, MECH *mech);
void mech_status(DbRef player, void *data, char *buffer);
void mech_critstatus(DbRef player, void *data, char *buffer);
PartDisplayName part_name(BtechContext *context, int type, int brand);
PartDisplayName part_name_long(BtechContext *context, int type, int brand);
PartDisplayName pos_part_name(MECH *mech, int index, int loop);
void mech_weaponspecs(DbRef player, void *data, char *buffer);
char *critstatus_func(MECH *mech, char *arg, char buffer[static MBUF_SIZE]);
char *sectstatus_func(MECH *mech, char *arg, char buffer[static MBUF_SIZE]);
char *armorstatus_func(MECH *mech, char *arg, char buffer[static MBUF_SIZE]);
char *weaponstatus_func(MECH *mech, char *arg, char buffer[static MBUF_SIZE]);
char *critslot_func(MECH *mech, char *buf_section, char *buf_critnum,
                    char *buf_flag, char buffer[static MBUF_SIZE]);
void CriticalStatus(EvaluationContext *evaluation, DbRef player, MECH *mech,
                    int index);
char *evaluate_ammo_amount(int now, int max);
void PrintWeaponStatus(EvaluationContext *evaluation, MECH *mech, DbRef player);
int ArmorEvaluateSerious(MECH *mech, int loc, int flag, int *opt);
void PrintArmorStatus(EvaluationContext *evaluation, DbRef player, MECH *mech,
                      int owner);
int hasPhysical(MECH *objMech, int wLoc, int wPhysType);
int canUsePhysical(MECH *objMech, int wLoc, int wPhysType);
