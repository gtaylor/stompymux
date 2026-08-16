/* Declares the BattleTech unit status API. */

#include "btech_text_result.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"

#pragma once

typedef struct BtechContext BtechContext;
typedef struct EvaluationContext EvaluationContext;

typedef struct PartDisplayName {
  char text[SBUF_SIZE];
  bool valid;
} PartDisplayName;

typedef enum MechPhysicalWeaponType : int {
  PHY_AXE = 1,
  PHY_SWORD,
  PHY_MACE,
  PHY_SAW,
  PHY_CLAW,
} MechPhysicalWeaponType;

/*
 * Armor status flags for ArmorEvaluateSerious().
 *
 * TODO: Can probably coalesce some of these with other subsystems.
 */
constexpr int ARMOR_TYPE_MASK = 0x07;
constexpr int ARMOR_FRONT = 0x00;    /* front armor */
constexpr int ARMOR_INTERNAL = 0x01; /* internal armor */
constexpr int ARMOR_REAR = 0x02;     /* rear armor */

constexpr int ARMOR_FLAG_OWNED = 0x10;     /* armor status by owner */
constexpr int ARMOR_FLAG_SHOW_DEST = 0x20; /* show destroyed sections */
constexpr int ARMOR_FLAG_DIVIDE_10 = 0x40; /* divide displayed value by 10 */

/*
 * Armor levels returned by ArmorEvaluateSerious().
 */
typedef enum ArmorLevel : int {
  ARMOR_LEVEL_GREAT = 0,
  ARMOR_LEVEL_GOOD = 1,
  ARMOR_LEVEL_LOW = 2,
  ARMOR_LEVEL_CRITICAL = 3,
  ARMOR_LEVEL_OPEN = 4,
  ARMOR_LEVEL_REPAIRING = 5,
} ArmorLevel;

typedef struct ArmorEvaluation {
  ArmorLevel level;
  int value;
} ArmorEvaluation;

typedef struct ArmorEvaluationRequest {
  Mech *mech;
  int section;
  int flags;
} ArmorEvaluationRequest;

typedef struct PhysicalWeaponRequest {
  Mech *mech;
  int section;
  MechPhysicalWeaponType type;
} PhysicalWeaponRequest;

static_assert((ARMOR_LEVEL_GREAT == 0 && ARMOR_LEVEL_REPAIRING == 5) != 0);

/* mech.status.c */
void display_target(EvaluationContext *evaluation, DbRef player, Mech *mech);
void show_miscbrands(Mech *mech, DbRef player);
void print_generic_status(EvaluationContext *evaluation, DbRef player,
                          Mech *mech, bool use_model_reference);
void print_heat_bar(EvaluationContext *evaluation, DbRef player, Mech *mech);
void print_info_status(EvaluationContext *evaluation, DbRef player, Mech *mech,
                       int own);
void print_short_info(EvaluationContext *evaluation, DbRef player, Mech *mech);
void mech_status(DbRef player, Mech *mech, const char *buffer);
void mech_critstatus(DbRef player, Mech *mech, char *buffer);
PartDisplayName part_name(BtechContext *context, int type, int brand);
PartDisplayName part_name_long(BtechContext *context, int type, int brand);
PartDisplayName pos_part_name(Mech *mech, int index, int loop);
void mech_weaponspecs(DbRef player, Mech *mech, const char *buffer);
typedef struct MechStatusTextRequest {
  Mech *mech;
  const char *argument;
  char *buffer;
} MechStatusTextRequest;

typedef struct CriticalSlotTextRequest {
  Mech *mech;
  const char *section;
  const char *critical;
  const char *field;
  char *buffer;
} CriticalSlotTextRequest;

BtechTextResult critstatus_func(const MechStatusTextRequest *request);
BtechTextResult sectstatus_func(const MechStatusTextRequest *request);
BtechTextResult armorstatus_func(const MechStatusTextRequest *request);
BtechTextResult weaponstatus_func(const MechStatusTextRequest *request);
BtechTextResult critslot_func(const CriticalSlotTextRequest *request);
void critical_status(EvaluationContext *evaluation, DbRef player, Mech *mech,
                     int index);
const char *evaluate_ammo_amount(int now, int max);
void print_weapon_status_summary(EvaluationContext *evaluation, Mech *mech,
                                 DbRef player);
ArmorEvaluation armor_evaluate(const ArmorEvaluationRequest *request);
void print_armor_status(EvaluationContext *evaluation, DbRef player, Mech *mech,
                        int owner);
bool has_physical(const PhysicalWeaponRequest *request);
bool can_use_physical(const PhysicalWeaponRequest *request);
