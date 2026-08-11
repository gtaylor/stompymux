/* Scores potential targets for autonomous combat decisions. */

#include <math.h>

#include "autopilot.h"
#include "autopilot_autogun_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
#include "map_los_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_position_api.h"
#include "mech_progress_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"

int auto_calc_target_score(Autopilot *autopilot, Mech *mech, Mech *target,
                           BattleMap *map) {

  int target_score;
  float range;
  float target_speed;
  int target_bv;

  int total_armor_current;
  int total_armor_original;
  int total_internal_current;
  int total_internal_original;

  int section;

  float damage_score;
  float bv_score;
  float speed_score;
  float range_score;
  float status_score;

  /* Default Values */
  target_score = 0;

  total_armor_current = 0;
  total_armor_original = 0;
  total_internal_current = 0;
  total_internal_original = 0;

  status_score = 0.0F;

  /* Here is the meat of the function, basicly I gave each
   * part a maximum score, then fit a linear plot from the
   * max to a min value and score.  Then I just summed
   * all the pieces together, very linear but should
   * give us a good starting point */

  /* Is the target dead? */
  if (mech_is_destroyed(target))
    return target_score;

  /* If target is combat safe don't even try to shoot it */
  if (mech_condition_summary(target).combat_safe)
    return target_score;

  /* Compare Teams - for now we won't try to shoot a guy on our team */
  if (mech_team(target) == mech_team(mech))
    return target_score;

  /* Are we in los of the target - not sure really what to do about this
   * one, since we want the AI to be smart and all, for now, lets have
   * it be all seeing */

  /* Range to target */
  range = map_real_range(&(MapRealSegment){
      .start = {.x = mech_position_real_x(mech),
                .y = mech_position_real_y(mech)},
      .end = {.x = mech_position_real_x(target),
              .y = mech_position_real_y(target)},
  });

  /* Our we outside the range of the AI's System */
  if (range >= (float)AUTO_GUN_MAX_RANGE) {
    return target_score;
  }

  /* Range score calc */
  /* Min range is 0, max range is 30, so score goes from 300 to 0 */
  range_score = -10.0F * range + 300.0F;

  /* Get the Speed of the target */
  target_speed = mech_current_speed(target);

  /* Speed score calc */
  /* Min speed is 0, max is 150 (can go higher tho), and score goes from
   * 300 to 0 (can go negative if the target is faster then 150) */
  /*! \todo {Check to see what happens when the target is backing} */
  speed_score = -2.0F * target_speed + 300.0F;

  /* Get the BV of the target */
  target_bv = mech_battle_value(target);

  /* BV score calc */
  /* Min bv is 0, max is around 2000 (can go higher), and score goes from
   * 0 to 100 (can go higher but we don't care much about bv) */
  bv_score = 0.05F * (float)target_bv;

  /* Get the damage of the target by cycling through all the sections
   * and adding up the current and original values */
  for (section = 0; section < NUM_SECTIONS; section++) {

    /* Total the current armor and original armor */
    total_armor_current += mech_section_armor(target, section) +
                           mech_section_rear_armor(target, section);
    total_armor_original += mech_section_original_armor(target, section) +
                            mech_section_original_rear_armor(target, section);

    /* Total the current internal and original internal */
    total_internal_current += mech_section_internal(target, section);
    total_internal_original += mech_section_original_internal(target, section);
  }

  /* Ok like above, we set a min and max, for armor was 100% to 0%
   * and scored from 0 to 300.  For internal was 100% to 0% and
   * scored from 0 to 200. But we have to take care not to divide
   * by zero. */

  /* Check the totals before we divide so no Divide by zeros */
  if (total_internal_original == 0 && total_armor_original == 0) {

    /* Both values are zero, not going to try and shoot it */
    return target_score;

  } else if (total_internal_original == 0) {

    /* Just use armor part of the calc */
    damage_score =
        -3.0F * ((float)total_armor_current / (float)total_armor_original) +
        300.0F;

  } else if (total_armor_original == 0) {

    /* Just use internal part of the calc */
    damage_score = -2.0F * ((float)total_internal_current /
                            (float)total_internal_original) +
                   200.0F;

  } else {

    /* Use the whole thing */
    damage_score =
        -3.0F * ((float)total_armor_current / (float)total_armor_original) +
        300.0F -
        2.0F *
            ((float)total_internal_current / (float)total_internal_original) +
        200.0F;
  }

  /* Get the 'state' ie: shutdown, prone whatever */
  if (!mech_is_started(target))
    status_score += 100.0F;

  if (mech_pilot_is_unconscious(target))
    status_score += 100.0F;

  /* Since the max bv is somewhat around 2000, lets put mechs in LOS on an even
   * scale */
  if (battle_map_unit_is_seen(map, mech, target))
    status_score += 2000.0F;

  /* Add the individual scores and return the value */
  const float COMBINED_SCORE =
      range_score + speed_score + bv_score + damage_score + status_score;
  const float ROUNDED_SCORE = floorf(COMBINED_SCORE);
  target_score = (int)ROUNDED_SCORE;

  return target_score;
}

/*
 * The main targeting/firing event for the AI
 *
 * Loops through all the cons around it, scoring them and deciding
 * what to shoot and what weapons to shoot at it
 */
