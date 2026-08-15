#include "ai_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
#include "mech_los_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mech_utils_internal.h"

#include "checked_conversion.h"
#include "map_units_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include <math.h>

float map_spatial_range(const MapSpatialSegment *segment) {
  const float DX = segment->start.x - segment->end.x;
  const float DY = segment->start.y - segment->end.y;
  const float DZ = segment->start.z - segment->end.z;

  return sqrtf((DX * DX) + (DY * DY) + (DZ * DZ)) / (float)SCALEMAP;
}

/* Computes hex range between Cartesian (x0, y0) and (x1, y1).  */
float map_real_range(const MapRealSegment *segment) {
  const float DX = segment->start.x - segment->end.x;
  const float DY = segment->start.y - segment->end.y;

  return sqrtf((DX * DX) + (DY * DY)) / (float)SCALEMAP;
}

/* CONVERSION ROUTINES courtesy Mike :) (Whoever that may be -focus) */

/* Picture blatantly ripped from the MUSH code by Dizzledorf and co. If only
   I had found it _before_ reverse-engineering the code :)
     - Focus, July 2002.
 */

/*
 * Convert floating-point cartesian coordinates into hex coordinates.
 *
 * To do this, split the hex map into a repeatable region, which itself has
 * 4 distinct regions, each part of a different hex. (See picture.) The hex
 * is normalized so that it is 1 unit high, and so is the repeatable region.
 * It works out that the repeatable region is exactly sqrt(3) wide, and can
 * be split up in six portions of each 1/6th sqrt(3), called 'alpha'.
 * Section I is 2 alpha wide at the top and bottom, and 3 alpha in the
 * middle. Sections II and III are reversed, being 4 alpha at the top and
 * bottom of the region, and 2 alpha in the middle. Section IV is 1 alpha in
 * the middle and 0 at the top and bottom. The whole region encompasses
 * exactly two x-columns and one y-row. All calculations are now done in
 * 'real' scale, to avoid rounding errors (isn't floating point arithmatic
 * fun ?)
 *
 * Alpha also returns in the slope of the hexsides (2*alpha, flipped or rotated
 * as necessary).  ANGLE_ALPHA is alpha (unscaled) for use in angle
 * calculations.
 *
 *       ________________________
 *      |        \              /|
 *      |         \    III     / |
 *      |          \          /  |
 *      |           \________/ IV|
 *      |    I      /        \   |
 *      |          /   II     \  |
 *      |         /            \ |
 *      |________/______________\|
 *
 */

/* Doubles for added accuracy; most calculations are doubles internally
   anyway, so we suffer little to no performance hit. */

static constexpr float ROOT3 = 558.5864F;         /* sqrt(3) * SCALEMAP */
static constexpr float ALPHA = 93.09773F;         /* ROOT3 / 6 */
static constexpr float ANGLE_ALPHA = 0.28867513F; /* sqrt(3) / 6 */
static constexpr float FULL_Y = (float)SCALEMAP;
static constexpr float HALF_Y = 0.5F * FULL_Y;

void real_coord_to_map_coord(short *hex_x, short *hex_y, float cart_x,
                             float cart_y) {
  float x;
  float y;
  int x_count;
  int y_count;

  if (cart_x < ALPHA) {
    /* Special case: we are in section IV of x-column 0 or off the map */
    *hex_x = cart_x < 0.0F ? -1 : 0;
    *hex_y = clamp_float_to_short(floorf(cart_y / (float)SCALEMAP));
    return;
  }

  /* 'shift' the map to the left so the repeatable box starts at 0 */
  cart_x -= ALPHA;

  /* Figure out the x-coordinate of the 'repeatable box' we're in. */
  x_count = clamp_float_to_int(cart_x / ROOT3);
  /* And the offset inside the box, from the left edge. */
  x = cart_x - ((float)x_count * ROOT3);

  /* The repbox holds two x-columns, we want the real X coordinate. */
  x_count *= 2;

  /* Do the same for the y-coordinate; this is easy */
  y_count = clamp_float_to_int(floorf(cart_y / FULL_Y));
  y = cart_y - ((float)y_count * FULL_Y);

  if (x < 2 * ALPHA) {

    /* Clean in area I. Nothing to do */

  } else if (x >= 3 * ALPHA && x < 5 * ALPHA) {
    /* Clean in either area II or III. Up x one, and y if in the lower
       half of the box. */
    x_count++;
    if (y >= HALF_Y)
      /* Area II */
      y_count++;

  } else if (x >= 2 * ALPHA && x < 3 * ALPHA) {
    /* Any of areas I, II and III. */
    if (y >= HALF_Y) {
      /* Area I or II */
      if (2 * ANGLE_ALPHA * (FULL_Y - y) <= x - (2 * ALPHA)) {
        /* Area II, up both */
        x_count++;
        y_count++;
      }
    } else {
      /* Area I or III */
      if (2 * ANGLE_ALPHA * y <= x - (2 * ALPHA))
        /* Area III, up only x */
        x_count++;
    }
  } else if (y >= HALF_Y) {
    /* Area II or IV. Up x at least one, maybe two, and y maybe one. */
    x_count++;
    if (2.0F * ANGLE_ALPHA * (y - HALF_Y) > (x - (5.0F * ALPHA)))
      /* Area II */
      y_count++;
    else
      /* Area IV */
      x_count++;
  } else {
    /* Area III or IV, up x at least one, maybe two */
    x_count++;
    if (2 * ANGLE_ALPHA * y > ROOT3 - x)
      /* Area IV */
      x_count++;
  }

  *hex_x = clamp_int_to_short(x_count);
  *hex_y = clamp_int_to_short(y_count);
}

/*
 * Convert hex coordinates into centered floating-point cartesian coordinates.
 *
 * Properties of hex centers:
 * 1) Spaced 3 ALPHA apart horizontally, starting from 2 ALPHA.
 * 2) Spaced FULL_Y apart vertically.
 * 3a) Even column centers (counting from 0) are vertically offset HALF_Y.
 * 3b) Odd column centers (counting from 0) are not vertically offset.
 */
void map_coord_to_real_coord(int hex_x, int hex_y, float *cart_x,
                             float *cart_y) {
  /* TODO: Can use some integer math if we're careful about overflow.  */
  /* Use % 2 for theoretical portability to non-2's-complement archs.  */
  *cart_x = (2.0F + (3.0F * (float)hex_x)) * ALPHA;
  *cart_y = ((hex_x % 2) ? 0.0F : HALF_Y) + ((float)hex_y * FULL_Y);
}

/*
   Sketch a 'mech on a Navigate map. Done here since it fiddles directly
   with cartesian coords.

   Navigate is 9 rows high, and a hex is exactly 1*SCALEMAP high, so each
   row is FULL_Y/9 cartesian y-coords high.

   Navigate is 21 hexes wide, at its widest point. This corresponds to the
   hex width, which is 4 * ALPHA, so each column is 4*ALPHA/21 cartesian
   x-coords wide.

   The actual navigate map starts two rows from the top and four columns
   from the left.

 */

static constexpr float NAV_ROW_HEIGHT = FULL_Y / 9.0F;
static constexpr float NAV_COLUMN_WIDTH = 4.0F * ALPHA / 21.0F;
static constexpr int NAV_Y_OFFSET = 2;
static constexpr int NAV_X_OFFSET = 4;
static constexpr int NAV_MAX_HEIGHT = 2 + 9 + 2;
static constexpr int NAV_MAX_WIDTH = 4 + 21 + 2;

void navigate_sketch_mechs(const NavigateSketchRequest *request) {
  Mech *mech = request->mech;
  BattleMap *map = request->map;
  const int X = request->center.x;
  const int Y = request->center.y;
  float corner_fx;
  float corner_fy;
  float fx;
  float fy;
  int row;
  int column;
  Mech *other;

  map_coord_to_real_coord(X, Y, &corner_fx, &corner_fy);
  corner_fx -= 2.0F * ALPHA;
  corner_fy -= HALF_Y;

  for (int i = 0; i < battle_map_unit_count(map); i++) {
    DbRef other_dbref = battle_map_unit_dbref(map, i);
    if (other_dbref < 0)
      continue;
    other = btech_context_find_object(mech->xcode.context, other_dbref);
    if (!other)
      continue;
    if (other == mech)
      continue;
    if (((other)->pd.x) != X || ((other)->pd.y) != Y)
      continue;
    if (!mech_los_check(mech, other, X, Y, 0.5))
      continue;

    fx = ((other)->pd.fx) - corner_fx;
    column = clamp_float_to_int((fx / NAV_COLUMN_WIDTH) + NAV_X_OFFSET);

    fy = ((other)->pd.fy) - corner_fy;
    row = clamp_float_to_int((fy / NAV_ROW_HEIGHT) + NAV_Y_OFFSET);

    if (column < 0 || column > NAV_MAX_WIDTH || row < 0 || row > NAV_MAX_HEIGHT)
      continue;

    request->plot(&(NavigatePlotCall){
        .row = row,
        .column = column,
        .marker = mech->pd.team == other->pd.team &&
                          mech_los_check_unblocked(mech, other, 0, 0, 0)
                      ? 'x'
                      : 'X',
        .context = request->context,
    });
  }

  /* Draw 'mech last so we always see it. */

  fx = ((mech)->pd.fx) - corner_fx;
  column = clamp_float_to_int((fx / NAV_COLUMN_WIDTH) + NAV_X_OFFSET);

  fy = ((mech)->pd.fy) - corner_fy;
  row = clamp_float_to_int((fy / NAV_ROW_HEIGHT) + NAV_Y_OFFSET);

  if (column < 0 || column > NAV_MAX_WIDTH || row < 0 || row > NAV_MAX_HEIGHT)
    return;

  request->plot(&(NavigatePlotCall){
      .row = row,
      .column = column,
      .marker = '*',
      .context = request->context,
  });
}

MechTargetPositionResult mech_target_position(const Mech *mech) {
  MechTargetPositionResult result = {};
  Mech *temp_mech;

  if (mech_target_dbref(mech) != -1) {
    temp_mech =
        btech_context_get_mech(mech->xcode.context, mech_target_dbref(mech));
    if (temp_mech) {
      result.found = true;
      result.position = (MapSpatialPosition){
          .x = temp_mech->pd.fx,
          .y = temp_mech->pd.fy,
          .z = temp_mech->pd.fz,
      };
      return result;
    }
  } else if (mech_target_hex_x(mech) != -1 && mech_target_hex_y(mech) != -1) {
    map_coord_to_real_coord(mech_target_hex_x(mech), mech_target_hex_y(mech),
                            &result.position.x, &result.position.y);
    int target_hex_z = mech_target_hex_z(mech);
    result.position.z = (float)ZSCALE * (float)target_hex_z;

    result.found = true;
    return result;
  }
  return result;
}
