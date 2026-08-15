/* Builds weapon profiles for autonomous combat decisions. */

#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "autopilot_autogun_api.h"
#include "autopilot_combat_policy_api.h"
#include "autopilot_weapon_profile_api.h"
#include "btech/context.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_identity_api.h"
#include "mech_runtime_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

static RedBlackTree *autopilot_weapon_profile_slot(Autopilot *autopilot,
                                                   int range) {
  if (range < 0)
    abort();
  return (RedBlackTree *)checked_storage_at(
      (void *)autopilot->profile, AUTO_PROFILE_MAX_SIZE, sizeof(RedBlackTree),
      (size_t)range);
}

void autopilot_weapon_profiles_initialize(Autopilot *autopilot) {
  memset((void *)autopilot->profile, 0, sizeof(autopilot->profile));
}

void autopilot_weapon_profiles_clear(Autopilot *autopilot) {
  for (int range = 0; range < AUTO_PROFILE_MAX_SIZE; range++) {
    RedBlackTree *profile = autopilot_weapon_profile_slot(autopilot, range);
    if (*profile != nullptr)
      red_black_tree_destroy(*profile);
    *profile = nullptr;
  }
}

RedBlackTree autopilot_weapon_profile_get(const Autopilot *autopilot,
                                          int range) {
  if (range < 0)
    abort();
  const RedBlackTree *profile = (const RedBlackTree *)checked_storage_at_const(
      (const void *)autopilot->profile, AUTO_PROFILE_MAX_SIZE,
      sizeof(RedBlackTree), (size_t)range);
  return *profile;
}

void autopilot_weapon_profile_set(Autopilot *autopilot, int range,
                                  RedBlackTree profile) {
  *autopilot_weapon_profile_slot(autopilot, range) = profile;
}

int *autopilot_weapon_range_score_key(AutopilotWeapon *weapon, int range) {
  if (range < 0)
    abort();
  return checked_storage_at(weapon->range_scores, AUTO_PROFILE_MAX_SIZE,
                            sizeof(int), (size_t)range);
}

static unsigned char autopilot_weapon_number_at(const unsigned char *weapons,
                                                int index) {
  if (index < 0)
    abort();
  const unsigned char *weapon = checked_storage_at_const(
      weapons, MAX_WEAPS_SECTION, sizeof(unsigned char), (size_t)index);
  return *weapon;
}

static int autopilot_weapon_critical_at(const int *criticals, int index) {
  if (index < 0)
    abort();
  const int *critical = checked_storage_at_const(criticals, MAX_WEAPS_SECTION,
                                                 sizeof(int), (size_t)index);
  return *critical;
}

typedef struct AutopilotWeaponRequest {
  int weapon_number;
  int catalogue_index;
  CriticalSlotReference slot;
} AutopilotWeaponRequest;

static AutopilotWeapon *
auto_create_weapon_node(const AutopilotWeaponRequest *request) {

  AutopilotWeapon *temp;

  temp = checked_storage_try_allocate(sizeof(AutopilotWeapon));

  if (temp == nullptr) {
    return nullptr;
  }

  memset(temp, 0, sizeof(AutopilotWeapon));

  temp->weapon_number = request->weapon_number;
  temp->weapon_db_number = request->catalogue_index;
  temp->section = request->slot.section;
  temp->critical = request->slot.critical;

  return temp;
}

/*
 * Destroy weapon node
 */
static void auto_destroy_weapon_node(AutopilotWeapon *victim) { free(victim); }

/*
 * Create a target node for the target list
 */
AutopilotTarget *
auto_create_target_node(const AutopilotTargetRequest *request) {

  AutopilotTarget *temp;

  temp = checked_storage_try_allocate(sizeof(AutopilotTarget));

  if (temp == nullptr) {
    return nullptr;
  }

  memset(temp, 0, sizeof(AutopilotTarget));

  temp->target_score = request->score;
  temp->target_dbref = request->target;

  return temp;
}

/*
 * Destroy a target node
 */
static void auto_destroy_target_node(AutopilotTarget *victim) { free(victim); }

/*
 * Destroy autopilot's weaplist
 */
void auto_destroy_weaplist(Autopilot *autopilot) {

  AutopilotWeapon *temp_weapon_node;

  /* Check to make sure there is a weapon list */
  if (!(autopilot->weaplist))
    return;

  /* There is a weapon list - lets kill it */
  if (doubly_linked_list_size(autopilot->weaplist) > 0) {

    while (doubly_linked_list_size(autopilot->weaplist)) {
      temp_weapon_node =
          (AutopilotWeapon *)doubly_linked_list_remove_node_at_pos(
              autopilot->weaplist, 1);
      auto_destroy_weapon_node(temp_weapon_node);
    }
  }

  /* Finally destroying the list */
  doubly_linked_list_destroy_list(autopilot->weaplist);
  autopilot->weaplist = nullptr;
}

/*
 * Callback function to destroy target list
 */
int auto_targets_callback(const RedBlackTreeVisitCall *call) {
  void *data = call->data;

  AutopilotTarget *temp;

  temp = (AutopilotTarget *)data;
  auto_destroy_target_node(temp);

  return 1;
}

/*
 * RedBlackTree generic compare function
 */
int auto_generic_compare(const RedBlackTreeCompareCall *call) {
  void *a = call->lhs;
  void *b = call->rhs;

  int *one;
  int *two;

  one = (int *)a;
  two = (int *)b;

  return (*one - *two);
}

/*
 * How we score a given weapon based on range, heat and damage
 */
typedef struct AutopilotWeaponScoreRequest {
  BtechContext *context;
  int weapon_index;
  int range;
} AutopilotWeaponScoreRequest;

static int auto_calc_weapon_score(const AutopilotWeaponScoreRequest *request) {
  BtechContext *context = request->context;
  const int WEAPON_DB_NUMBER = request->weapon_index;
  const int RANGE = request->range;

  int weapon_damage;
  const WeaponRangeProfile WEAPON_RANGES =
      weapon_catalogue_ranges(WEAPON_DB_NUMBER);

  /* Simple Calc */

  /* For the modifiers I assumed best was approx 550
   *
   * So for SR, chance of hitting is roughly 92% which is 506 rounded to 500
   * For MR, its 72%, so 390 and LR its 41% its 225 */

  /* Assume default values */
  weapon_damage = 0;

  /* Get the damage for the weapon */
  if (weapon_catalogue_is_missile(WEAPON_DB_NUMBER)) {
    const MissileHitEntry *entry = missile_hit_registry_find_weapon(
        &context->missile_hits, WEAPON_DB_NUMBER);

    /* Its a missile weapon so lookup in the Missile table get the max
     * number of missiles it can hit with, and multiply by the damage
     * per missile */
    /* To make it more fair going to use the avg # of missile hits
     * which is when they would roll a 7, which becomes slot #
     * 5 */
    if (entry != nullptr)
      weapon_damage =
          entry->num_missiles[5] * weapon_catalogue_damage(WEAPON_DB_NUMBER);

  } else {
    weapon_damage = weapon_catalogue_damage(WEAPON_DB_NUMBER);
  }

  return autopilot_weapon_score(&(AutopilotWeaponScoreSituation){
      .range = RANGE,
      .minimum_range = WEAPON_RANGES.minimum,
      .short_range = WEAPON_RANGES.short_range,
      .medium_range = WEAPON_RANGES.medium_range,
      .long_range = WEAPON_RANGES.long_range,
      .damage = weapon_damage,
      .heat = weapon_catalogue_heat(WEAPON_DB_NUMBER)});
}

/*
 * AI profiling event
 *
 * Every so often updates the profile for the AI's weapons
 */
void auto_update_profile_event(Autopilot *autopilot) {
  Mech *mech = (Mech *)autopilot->mymech;

  AutopilotWeapon *temp_weapon_node;
  DoublyLinkedListNode *temp_dllist_node;

  int section;
  int weapon_count_section;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];

  int range;

  int weapon_count;
  int weapon_number;

  /* Basic checks */
  /* some accounting checks. try to prevent some race stuff */

  if (!is_good_obj(autopilot->xcode.context->database, autopilot->mymechnum)) {
    /* most commonly, the mech is a bad memory space.
     * lets not try to access it
     */
    autopilot_gunning_stop(autopilot);
    return;
  }

  if (!mech) {
    return;
  }
  if (!autopilot) {
    return;
  }
  if (!btech_context_is_mech(mech_context(mech), mech_dbref(mech)) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Ok our mech is dead we're done */
  if (mech_is_destroyed(mech)) {
    return;
  }

  /* Log Message */
  autopilot_autogun_log(autopilot, "Profiling Unit #%ld", mech_dbref(mech));

  /* Destroy the arrays first, don't worry about the weap
   * structures because we can clear them with the ddlist
   * weaplist */

  /* Zero the array of RedBlackTree stuff */
  autopilot_weapon_profiles_clear(autopilot);

  /* Check to see if the weaplist exists */
  if (autopilot->weaplist != nullptr) {

    /* Destroy the list */
    auto_destroy_weaplist(autopilot);
  }

  /* List doesn't exist so lets build it */
  autopilot->weaplist = doubly_linked_list_create_list();

  /* Reset the AI's max range value for its mech */
  autopilot->mech_max_range = 0;

  /* Set our counter */
  weapon_count = -1;

  /* Now loop through the weapons building a list */
  for (section = 0; section < NUM_SECTIONS; section++) {

    /* Find all the weapons for a given section */
    weapon_count_section =
        find_weapons_advanced(mech, section, weaparray, weapdata, critical, 1);

    /* No weapons here */
    if (weapon_count_section <= 0)
      continue;

    /* loop through the possible weapons */
    for (weapon_number = 0; weapon_number < weapon_count_section;
         weapon_number++) {
      const int WEAPON_INDEX =
          (int)autopilot_weapon_number_at(weaparray, weapon_number);
      const int CRITICAL_INDEX =
          autopilot_weapon_critical_at(critical, weapon_number);

      /* Count it even if its not a valid weapon like AMS */
      /* This is so when we go to fire the weapon we know
       * which one to send in the command */
      weapon_count++;

      if (weapon_catalogue_is_anti_missile(WEAPON_INDEX))
        continue;

      /* Does it work? */
      if (weapon_is_nonfunctional(
              mech, section, CRITICAL_INDEX,
              get_weapon_crits(mech,
                               weapon_from_equipment_index(WEAPON_INDEX))) > 0)
        continue;

      /* Ok made it this far, lets add it to our list */
      temp_weapon_node = auto_create_weapon_node(&(AutopilotWeaponRequest){
          .weapon_number = weapon_count,
          .catalogue_index = WEAPON_INDEX,
          .slot = {.section = section, .critical = CRITICAL_INDEX}});

      temp_dllist_node = doubly_linked_list_create_node(temp_weapon_node);
      doubly_linked_list_insert_end(autopilot->weaplist, temp_dllist_node);

      /* Check the max range */
      const int LONG_RANGE = weapon_catalogue_ranges(WEAPON_INDEX).long_range;
      if (autopilot->mech_max_range < LONG_RANGE) {
        autopilot->mech_max_range = LONG_RANGE;
      }
    }
  }

  /* Now build the profile array, basicly loop through
   * all the current avail weapons, get its max range,
   * then loop through ranges and for each range add it
   * to profile */

  /* Our counter */
  weapon_number = 1;

  while (weapon_number <= doubly_linked_list_size(autopilot->weaplist)) {

    /* Get the weapon */
    temp_weapon_node = (AutopilotWeapon *)doubly_linked_list_get_node(
        autopilot->weaplist, weapon_number);

    const int LONG_RANGE =
        weapon_catalogue_ranges(temp_weapon_node->weapon_db_number).long_range;
    for (range = 0; range < LONG_RANGE; range++) {

      /* Out side the the range of AI's profile system */
      if (range >= AUTO_PROFILE_MAX_SIZE) {
        break;
      }

      /* Score the weapon */
      int *range_score =
          autopilot_weapon_range_score_key(temp_weapon_node, range);
      *range_score = auto_calc_weapon_score(&(AutopilotWeaponScoreRequest){
          .context = autopilot->xcode.context,
          .weapon_index = temp_weapon_node->weapon_db_number,
          .range = range});

      /* If RedBlackTree for this range doesn't exist, create it */
      RedBlackTree profile = autopilot_weapon_profile_get(autopilot, range);
      if (profile == nullptr) {
        profile = red_black_tree_init(&auto_generic_compare, nullptr);
        autopilot_weapon_profile_set(autopilot, range, profile);
      }

      /* Check to see if the score exists in the tree
       * if so alter it slightly so we don't have
       * overlaping keys */
      while (1) {

        if (red_black_tree_exists(profile, range_score)) {
          (*range_score)++;
        } else {
          break;
        }
      }

      /* Add it to tree */
      red_black_tree_insert(profile, range_score, temp_weapon_node);
    }

    /* Increment */
    weapon_number++;
  }

  /* Log Message */
  autopilot_autogun_log(autopilot, "Finished Profiling");
}

/*
 * Function to calculate a score based on a target and
 * its range to the AI
 */
