/* Builds weapon profiles for autonomous combat decisions. */

#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "autopilot_autogun_api.h"
#include "autopilot_weapon_profile_api.h"
#include "btech/context.h"
#include "equipment_types.h"
#include "mech_identity_api.h"
#include "mech_runtime_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/flags.h"
#include "mux/server/diagnostics.h"
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
  return checked_storage_at(autopilot->profile, AUTO_PROFILE_MAX_SIZE,
                            sizeof(RedBlackTree), (size_t)range);
}

void autopilot_weapon_profiles_initialize(Autopilot *autopilot) {
  memset(autopilot->profile, 0, sizeof(autopilot->profile));
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
  const RedBlackTree *profile =
      checked_storage_at_const(autopilot->profile, AUTO_PROFILE_MAX_SIZE,
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

static AutopilotWeapon *auto_create_weapon_node(int weapon_number,
                                                int weapon_db_number,
                                                int section, int critical) {

  AutopilotWeapon *temp;

  temp = malloc(sizeof(AutopilotWeapon));

  if (temp == nullptr) {
    return nullptr;
  }

  memset(temp, 0, sizeof(AutopilotWeapon));

  temp->weapon_number = weapon_number;
  temp->weapon_db_number = weapon_db_number;
  temp->section = section;
  temp->critical = critical;

  return temp;
}

/*
 * Destroy weapon node
 */
static void auto_destroy_weapon_node(AutopilotWeapon *victim) {

  free(victim);
  return;
}

/*
 * Create a target node for the target list
 */
AutopilotTarget *auto_create_target_node(int target_score, DbRef target_dbref) {

  AutopilotTarget *temp;

  temp = malloc(sizeof(AutopilotTarget));

  if (temp == nullptr) {
    return nullptr;
  }

  memset(temp, 0, sizeof(AutopilotTarget));

  temp->target_score = target_score;
  temp->target_dbref = target_dbref;

  return temp;
}

/*
 * Destroy a target node
 */
static void auto_destroy_target_node(AutopilotTarget *victim) {

  free(victim);
  return;
}

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
int auto_targets_callback(void *key, void *data, int depth, void *arg) {

  AutopilotTarget *temp;

  temp = (AutopilotTarget *)data;
  auto_destroy_target_node(temp);

  return 1;
}

/*
 * RedBlackTree generic compare function
 */
int auto_generic_compare(void *a, void *b, void *token) {

  int *one, *two;

  one = (int *)a;
  two = (int *)b;

  return (*one - *two);
}

/*
 * How we score a given weapon based on range, heat and damage
 */
static int auto_calc_weapon_score(BtechContext *context, int weapon_db_number,
                                  int range) {

  int weapon_score;
  int range_score;
  int damage_score;
  int heat_score;
  float minrange_score;

  int weapon_damage;
  int weapon_heat;
  const WeaponRangeProfile weapon_ranges =
      weapon_catalogue_ranges(weapon_db_number);

  /* Simple Calc */

  /* For the modifiers I assumed best was approx 550
   *
   * So for SR, chance of hitting is roughly 92% which is 506 rounded to 500
   * For MR, its 72%, so 390 and LR its 41% its 225 */

  /* Assume default values */
  weapon_score = 0;
  weapon_damage = 0;
  range_score = 500; /* Since by default we assume its SR */
  minrange_score = 0;

  /* Don't bother trying to set a value if its outside its range */
  if (range >= weapon_ranges.long_range) {
    return weapon_score;
  }

  /* Are we at LR ? */
  if (range >= weapon_ranges.medium_range) {
    range_score = 215;
  }

  /* Are we at MR ? */
  if (range >= weapon_ranges.short_range &&
      range < weapon_ranges.medium_range) {
    range_score = 390;
  }

  /* Check min range */
  /* Use a polynomial equation here because at 2 under min its equiv to MR, at
   * 4 under its equiv to LR, so we want it to balance out the range score */
  /* score = -12.5(min - range)^2 - 25 * (min - range) */
  if (range < weapon_ranges.minimum) {
    const int minimum_range_delta = weapon_ranges.minimum - range;
    minrange_score =
        -12.5F * (float)(minimum_range_delta * minimum_range_delta) -
        25.0F * (float)minimum_range_delta;
  }

  /* Get the damage for the weapon */
  if (weapon_catalogue_is_missile(weapon_db_number)) {
    const MissileHitEntry *entry = missile_hit_registry_find_weapon(
        &context->missile_hits, weapon_db_number);

    /* Its a missile weapon so lookup in the Missile table get the max
     * number of missiles it can hit with, and multiply by the damage
     * per missile */
    /* To make it more fair going to use the avg # of missile hits
     * which is when they would roll a 7, which becomes slot #
     * 5 */
    if (entry != nullptr)
      weapon_damage =
          entry->num_missiles[5] * weapon_catalogue_damage(weapon_db_number);

  } else {
    weapon_damage = weapon_catalogue_damage(weapon_db_number);
  }

  /* Get the damage score */
  /* Straight linear plot */
  damage_score = 50 * weapon_damage;

  /* Get the heat */
  weapon_heat = weapon_catalogue_heat(weapon_db_number);

  /* Get the heat score */
  /* Straight inverse linear plot - more heat bad... */
  heat_score = -25 * weapon_heat + 250;

  /* Final calc */
  weapon_score = range_score + damage_score + heat_score + (int)minrange_score;

  return weapon_score;
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
    dprintk("ap mymechnum is bad");
    autopilot_gunning_stop(autopilot);
    return;
  }

  if (!mech) {
    dprintk("mech is bad!");
    return;
  }
  if (!autopilot) {
    dprintk("ai is bad!");
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
        FindWeapons_Advanced(mech, section, weaparray, weapdata, critical, 1);

    /* No weapons here */
    if (weapon_count_section <= 0)
      continue;

    /* loop through the possible weapons */
    for (weapon_number = 0; weapon_number < weapon_count_section;
         weapon_number++) {
      const int weapon_index =
          (int)autopilot_weapon_number_at(weaparray, weapon_number);
      const int critical_index =
          autopilot_weapon_critical_at(critical, weapon_number);

      /* Count it even if its not a valid weapon like AMS */
      /* This is so when we go to fire the weapon we know
       * which one to send in the command */
      weapon_count++;

      if (weapon_catalogue_is_anti_missile(weapon_index))
        continue;

      /* Does it work? */
      if (WeaponIsNonfunctional(
              mech, section, critical_index,
              GetWeaponCrits(mech, weapon_from_equipment_index(weapon_index))) >
          0)
        continue;

      /* Ok made it this far, lets add it to our list */
      temp_weapon_node = auto_create_weapon_node(weapon_count, weapon_index,
                                                 section, critical_index);

      temp_dllist_node = doubly_linked_list_create_node(temp_weapon_node);
      doubly_linked_list_insert_end(autopilot->weaplist, temp_dllist_node);

      /* Check the max range */
      const int long_range = weapon_catalogue_ranges(weapon_index).long_range;
      if (autopilot->mech_max_range < long_range) {
        autopilot->mech_max_range = long_range;
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

    const int long_range =
        weapon_catalogue_ranges(temp_weapon_node->weapon_db_number).long_range;
    for (range = 0; range < long_range; range++) {

      /* Out side the the range of AI's profile system */
      if (range >= AUTO_PROFILE_MAX_SIZE) {
        break;
      }

      /* Score the weapon */
      int *range_score =
          autopilot_weapon_range_score_key(temp_weapon_node, range);
      *range_score = auto_calc_weapon_score(
          autopilot->xcode.context, temp_weapon_node->weapon_db_number, range);

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
