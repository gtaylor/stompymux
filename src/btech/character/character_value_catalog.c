#include <stddef.h>
#include <stdlib.h>

#include "btechstats.h"
#include "btechstats_global.h"
#include "btechstats_internal.h"
#include "character_value_settings.h"
#include "context_internal.h" // IWYU pragma: keep
#include "mux/support/checked_storage.h"

static const char *const BTECH_CHARVALUETYPE_NAMES[] = {
    "Char_value", "Char_skill", "Char_advantage", "Char_attribute"};

static const CharacterValue CHARACTER_VALUES[NUM_CHARVALUES] = {
    {"XP", CHAR_VALUE, 0, 0},
    {"MaxXP", CHAR_VALUE, 0, 0},
    {"Type", CHAR_VALUE, 0, 0},
    {"Level", CHAR_VALUE, 0, 0},
    {"Package", CHAR_VALUE, 0, 0},
    {"Lives", CHAR_VALUE, 0, 0},
    {"Bruise", CHAR_VALUE, 0, 0},
    {"Lethal", CHAR_VALUE, 0, 0},
    {"Unused1", CHAR_VALUE, 0, 0},
    {"ShotsFired", CHAR_VALUE, 0, 0},
    {"ShotsMissed", CHAR_VALUE, 0, 0},
    {"ShotsHit", CHAR_VALUE, 0, 0},
    {"DamageTaken", CHAR_VALUE, 0, 0},
    {"DamageGiven", CHAR_VALUE, 0, 0},

    {"Ambidextrous", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Bloodname", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Combat_Sense", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Contact", CHAR_ADVANTAGE, CHAR_ADV_VALUE, 0},
    {"Dropship", CHAR_ADVANTAGE, CHAR_ADV_VALUE, 0},
    {"EI_Implant", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Exceptional_Attribute", CHAR_ADVANTAGE, CHAR_ADV_EXCEPT, 0},
    {"Extra_Edge", CHAR_ADVANTAGE, CHAR_ADV_VALUE, 0},
    {"Land_Grant", CHAR_ADVANTAGE, CHAR_ADV_VALUE, 0},
    {"Reputation", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Sixth_Sense", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Title", CHAR_ADVANTAGE, CHAR_ADV_VALUE, 0},
    {"Toughness", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Wealth", CHAR_ADVANTAGE, CHAR_ADV_VALUE, 0},
    {"Well-Connected", CHAR_ADVANTAGE, CHAR_ADV_VALUE, 0},
    {"Well_Equipped", CHAR_ADVANTAGE, CHAR_ADV_VALUE, 0},
    {"Dodge_Maneuver", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Maneuvering_Ace", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Melee_Specialist", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Pain_Resistance", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Speed_Demon", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},
    {"Tech_Aptitude", CHAR_ADVANTAGE, CHAR_ADV_BOOL, 0},

    {"Build", CHAR_ATTRIBUTE, 0, 0},
    {"Reflexes", CHAR_ATTRIBUTE, 0, 0},
    {"Intuition", CHAR_ATTRIBUTE, 0, 0},
    {"Learn", CHAR_ATTRIBUTE, 0, 0},
    {"Charisma", CHAR_ATTRIBUTE, 0, 0},

    {"Acrobatics", CHAR_SKILL, CHAR_ATHLETIC, 50},
    {"Administration", CHAR_SKILL, CHAR_MENTAL, 50},
    {"Alternate_Identity", CHAR_SKILL, CHAR_MENTAL, 50},
    {"Appraisal", CHAR_SKILL, CHAR_MENTAL, 50},
    {"Archery", CHAR_SKILL, CHAR_ATHLETIC, 50},
    {"Blade", CHAR_SKILL, CHAR_ATHLETIC | CAREER_MISC, 50},
    {"Bureaucracy", CHAR_SKILL, CHAR_SOCIAL | CAREER_MISC, 50},
    {"Climbing", CHAR_SKILL, CHAR_ATHLETIC, 50},
    {"Comm-Conventional", CHAR_SKILL, CHAR_MENTAL | CAREER_TECH, 150},
    {"Comm-Hyperpulse", CHAR_SKILL, CHAR_MENTAL | CAREER_TECH, 50},
    {"Computer", CHAR_SKILL, CHAR_MENTAL | CAREER_TECH, 50},
    {"Cryptography", CHAR_SKILL, CHAR_MENTAL | CAREER_TECH, 50},
    {"Demolitions", CHAR_SKILL, CHAR_MENTAL, 50},
    {"Disguise", CHAR_SKILL, CHAR_MENTAL | CAREER_RECON, 50},
    {"Drive", CHAR_SKILL, CHAR_PHYSICAL | CAREER_CAVALRY, 3000},
    {"Drive-Naval", CHAR_SKILL, CHAR_PHYSICAL, 3000},
    {"Engineering", CHAR_SKILL, CHAR_MENTAL | CAREER_TECH, 50},
    {"Escape_Artist", CHAR_SKILL, CHAR_PHYSICAL | CAREER_RECON, 50},
    {"Forgery", CHAR_SKILL, CHAR_MENTAL, 50},
    {"Gambling", CHAR_SKILL, CHAR_MENTAL, 50},

    {"Gunnery-Aerospace", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_AERO,
     1000},
    {"Gunnery-Artillery", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_ARTILLERY,
     500},
    {"Gunnery-Battlemech", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_BMECH,
     3000},
    {"Gunnery-BSuit", CHAR_SKILL, SK_XP | CHAR_PHYSICAL, 500},
    {"Gunnery-Conventional", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_CAVALRY,
     3000},
    {"Gunnery-Spacecraft", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_DROPSHIP,
     50},
    {"Gunnery-Spotting", CHAR_SKILL, CHAR_PHYSICAL | CAREER_ARTILLERY, 50},
    {"Gunnery-Ballistic", CHAR_SKILL, SK_XP | CHAR_PHYSICAL, 2500},
    {"Gunnery-Flamer", CHAR_SKILL, SK_XP | CHAR_PHYSICAL, 500},
    {"Gunnery-Laser", CHAR_SKILL, SK_XP | CHAR_PHYSICAL, 2500},
    {"Gunnery-Missile", CHAR_SKILL, SK_XP | CHAR_PHYSICAL, 2500},

    {"Impersonation", CHAR_SKILL, CHAR_SOCIAL, 50},
    {"Interrogation", CHAR_SKILL, CHAR_SOCIAL | CAREER_RECON, 50},
    {"Jump_Pack", CHAR_SKILL, CHAR_ATHLETIC, 50},
    {"Leadership", CHAR_SKILL, CHAR_SOCIAL | CAREER_ACADMISC, 50},
    {"Medtech", CHAR_SKILL, CHAR_MENTAL | CAREER_MISC, 300},
    {"Navigation", CHAR_SKILL, CHAR_MENTAL, 25},
    {"Negotiation", CHAR_SKILL, CHAR_SOCIAL, 25},
    {"Perception", CHAR_SKILL, CHAR_MENTAL | CAREER_RECON, 150},

    {"Piloting-Aerospace", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_AERO,
     2500},
    {"Piloting-Battlemech", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_BMECH,
     3000},
    {"Piloting-BSuit", CHAR_SKILL, SK_XP | CHAR_ATHLETIC, 3000},
    {"Piloting-Spacecraft", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_DROPSHIP,
     50},
    {"Piloting-Biped", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_BMECH, 3000},
    {"Piloting-Hover", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_CAVALRY,
     3000},
    {"Piloting-Naval", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_CAVALRY,
     3000},
    {"Piloting-Quad", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_BMECH, 3000},
    {"Piloting-Tracked", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_CAVALRY,
     3000},
    {"Piloting-Wheeled", CHAR_SKILL, SK_XP | CHAR_PHYSICAL | CAREER_CAVALRY,
     3000},
    {"Protocol", CHAR_SKILL, CHAR_SOCIAL, 50},
    {"Quickdraw", CHAR_SKILL, CHAR_PHYSICAL, 50},
    {"Research", CHAR_SKILL, CHAR_MENTAL | CAREER_TECH, 100},
    {"Running", CHAR_SKILL, SK_XP | CHAR_ATHLETIC, 100},
    {"Riding", CHAR_SKILL, CHAR_ATHLETIC, 50},
    {"Scrounge", CHAR_SKILL, CHAR_SOCIAL | CAREER_TECH, 50},
    {"Security_Systems", CHAR_SKILL, CHAR_MENTAL | CAREER_RECON, 50},
    {"Seduction", CHAR_SKILL, CHAR_SOCIAL, 50},
    {"Small_Arms", CHAR_SKILL, CHAR_PHYSICAL | CAREER_MISC, 50},
    {"Stealth", CHAR_SKILL, CHAR_PHYSICAL | CAREER_RECON, 50},
    {"Strategy", CHAR_SKILL, CHAR_MENTAL | CAREER_ACADMISC, 50},
    {"Streetwise", CHAR_SKILL, CHAR_SOCIAL, 50},
    {"Support_Weapons", CHAR_SKILL, CHAR_PHYSICAL | CAREER_MISC, 50},
    {"Survival", CHAR_SKILL, CHAR_MENTAL, 50},
    {"Swimming", CHAR_SKILL, CHAR_ATHLETIC, 50},
    {"Tactics", CHAR_SKILL, CHAR_MENTAL | CAREER_ACADMISC, 50},
    {"Technician-Aerospace", CHAR_SKILL, SK_XP | CHAR_MENTAL | CAREER_TECHVEH,
     50},
    {"Technician-Battlemech", CHAR_SKILL, SK_XP | CHAR_MENTAL | CAREER_TECHMECH,
     600},
    {"Technician-Battlesuit", CHAR_SKILL, SK_XP | CHAR_MENTAL, 300},
    {"Technician-Electronics", CHAR_SKILL, SK_XP | CHAR_MENTAL | CAREER_TECH,
     50},
    {"Technician-Mechanic", CHAR_SKILL, SK_XP | CHAR_MENTAL | CAREER_TECHVEH,
     400},
    {"Technician-Weapons", CHAR_SKILL, SK_XP | CHAR_MENTAL | CAREER_TECH, 300},
    {"Technician-Spacecraft", CHAR_SKILL, SK_XP | CHAR_MENTAL, 50},
    {"Throwing_Weapons", CHAR_SKILL, CHAR_PHYSICAL, 50},
    {"Tinker", CHAR_SKILL, CHAR_MENTAL | CAREER_TECH, 50},
    {"Tracking", CHAR_SKILL, CHAR_MENTAL | CAREER_RECON, 50},
    {"Training", CHAR_SKILL, CHAR_SOCIAL, 50},
    {"Unarmed_Combat", CHAR_SKILL, CHAR_ATHLETIC | CAREER_MISC, 50},
    {"Zero-G_Operations", CHAR_SKILL, CHAR_PHYSICAL, 50},
};

static size_t character_value_index(int code) {
  if (code < 0 || code >= NUM_CHARVALUES)
    abort();
  return (size_t)code;
}

static int *
character_value_threshold_slot(BtechCharacterValueSettings *settings,
                               int code) {
  return checked_storage_at(settings->xp_thresholds, NUM_CHARVALUES,
                            sizeof(*settings->xp_thresholds),
                            character_value_index(code));
}

static const int *
character_value_threshold_at(const BtechCharacterValueSettings *settings,
                             int code) {
  return checked_storage_at_const(settings->xp_thresholds, NUM_CHARVALUES,
                                  sizeof(*settings->xp_thresholds),
                                  character_value_index(code));
}

const CharacterValue *character_value_definition(int code) {
  return checked_storage_at_const(CHARACTER_VALUES, NUM_CHARVALUES,
                                  sizeof(*CHARACTER_VALUES),
                                  character_value_index(code));
}

const char *character_value_type_name(int type) {
  if (type < CHAR_VALUE || type > CHAR_ATTRIBUTE)
    return "Unknown";
  const char *const *name = (const char *const *)checked_storage_at_const(
      (const void *)BTECH_CHARVALUETYPE_NAMES,
      sizeof(BTECH_CHARVALUETYPE_NAMES) / sizeof(*BTECH_CHARVALUETYPE_NAMES),
      sizeof(*BTECH_CHARVALUETYPE_NAMES), (size_t)type);
  return *name;
}

void btech_character_value_settings_initialize(
    BtechCharacterValueSettings *settings) {
  if (settings->initialized)
    return;
  for (int code = 0; code < NUM_CHARVALUES; code++)
    *character_value_threshold_slot(settings, code) =
        character_value_definition(code)->default_xp_threshold;
  settings->initialized = true;
}

int character_value_xp_threshold(const BtechContext *context, int code) {
  if (context == nullptr || !context->character_values.initialized)
    abort();
  return *character_value_threshold_at(&context->character_values, code);
}

void character_value_xp_threshold_set(const CharacterValueThreshold *value) {
  if (value == nullptr || value->context == nullptr ||
      !value->context->character_values.initialized)
    abort();
  *character_value_threshold_slot(&value->context->character_values,
                                  value->code) = value->threshold;
}
