
/* Traces line of sight through map hexes. */

/*************************************************************
****  LOS code  **********************************************
**************************************************************

What happens in the code now:
(pretty close, anyway -- I haven't spent THAT much time studying it.

Right now, the code just finds a line between the hexes and steps
along it in units of 1 hex, checking each hex it enters.  This is,
well, wrong.  It makes no allowance whatsoever that the LOS line might
"clip" the edge of a hex, and will occasionally skip hexes:
 __    __    __
/# \__/  \__/# \
\__/  \__/  \__/  is possible, when it SHOULD be:
 __    __    __
/# \__/  \__/# \  A 1 hex wide wall in the first case would be
\__/  \__/  \__/  skipped entirely.
   \__/  \__/

Now my, (correct, but tougher) algorithm:

OK, first FASA rules: "The LOS is checked by laying a straightedge
... from the center of the attackers's hex to the center of the
target's hex.  Any hex that the straightedge crosses is in the LOS.
If the straightedge passes directly between two hexes, the defender
chooses which hex ist passes through." -- Compendium, p21

It is fairly easy to convince yourself that the case of the line lying
directly between two hexes is a well-defined special case: it happens
only when the hexes lie exactly on a 30, 90, 150, 210, 270, or 330
line.  This is easy to check for, so we'll put that check in as the
first stage of the routine.  We'll assume from then on that the line
is not a special case  (which is good -- the numbers explode to
infinity for 90 degree lines).

Now we iterate as the current muse code does.  We start at the hex at
one end of the line, find the next one, and repeat until we get to the
end.  Here is the algorithm for finding the "next" hex:

First, we need only check the 6 adjacent hexes (obviously -- but the
current code ignores this and sometimes returns "next" hexes that are
not adjacent).

In addition, an eligible hex muse be "on the line."  That is, the
imaginary line drawn between the endpoints must pass through the hex.
Figuring this out is the "meat" of this algorithm.  The best way to
determine this (that I have discovered, anyway) is to find the
(absolute, cartesian, floating point) distance between the center
point of the hex in question and the line (i.e. the length of a line
drawn from the center of hex, perpendicular to the LOS line). If this
distance is less than the "effective radius" of the hex, then the line
lies inside the hex.  By "effective radius" I mean the width of the
hex along a line perpendicular to the LOS line.  This will vary
(depending on the angle of the line) between 0.5 (if the hex is lying
flat along the line -- i.e. a line pointing at 30, 90, 150, 210, 270,
or 330 degrees) and (1/sqrt(3)) (if the line passes only through the
point of the hex -- i.e. a line pointing at 0, 60, 120, 180, 240, or
300)

  _Effective_Radius_
  _____        _____
 /     \ |    /     \   /     (Hope this diagram makes sense!
/       \|   /   *_  \ /       the *'s are the line representing
\       /|   \     * //        the effective radius)
 \_____/ |    \_____//
    *****|          /

(1/sqrt(3))     0.5

For other angles, the effective radius varies as a cosine.  Since the
whole thing is periodic with intervals of 60 deg, we can "modulo" it
by subtracting 60 deg until we get the angle to within a -30 < angle <
30 degrees region and then taking the cosine.

Hoof!  That's the worst of it.  Now, from among the adjacent, eligible
hexes, the "next" one is the one that is closest "along the line" to
the one we are in. (But is still closer to the endpoint than the
current hex.  We have to check here to make sure we don't hop back
into the hex we came from!)

Now performance issues:

The main calculations here are the distance-to-line determination, and
the effective radius calculation.  Each of these must be performed 5
times (6 adjacent hexes, but we don't have to check the one we came
from).  Distance-to-line is best found by using a matrix to rotate the
coordinates to a frame where the line is horizontal and taking the
difference in y coords between the hex and any point on the line (one
of the endpoints will do).  The angle to rotate is -(slope of the LOS
line), which is -atan(y1-y2/x1-x2).  The atan function need only be
called once, since the slope of the line is unchanging.  That leaves
us with the matrix:   |cos(u) -sin(u)|
                      |sin(u)  cos(u)|   where u=-atan(y1-y2/x1-x2)

This contains four more trigonometric functions, but again each need
only be called once (and there are really only two anyway, sin(u) and
cos(u)).

The effective radius involves a cos() function.  Again, though, since
the slope of the line is unchanging, the effective radius of the hex
is a constant throughout the iterations, and need only be calculated
once.

So now we have one time costs:

*Calculate matrix coefficients:  1 atan(), 1 cos(), 1 sin()
                                 2 fp subtractions, 1 fp divide.

*Find effective radius:          1 cos(),
                                <6 fp subtractions (to get -30<u<30)

And per-hex-travelled costs:

*Test 5 hexes if on line:       10 fp mults, 5 fp additions
                                 5 fp compares

*Test for closest forward hex:  <5 fp compares

This is actually, I would think, not much more than the current LOS
code, which makes many calls to RealCoordToMapCoord() (a costly
function) and is generally messy, with lots of repeated mults by
constants such as ZSCALE (which would optimize out, if we did, but we
don't :)

**********************************************************************

OK, enough of this.  Let's get on to the code.
*/

#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_lostracer_api.h"
#include "mux/support/checked_storage.h"

static constexpr float DEG60 = 1.0471976F;
static constexpr float DEG30 = 0.5235988F;
static constexpr float ROOT3 = 1.7320508F;
static constexpr float TRACESCALEMAP = 1.0F;

typedef enum HexDirection {
  HEX_NORTH,
  HEX_NORTHEAST,
  HEX_SOUTHEAST,
  HEX_SOUTH,
  HEX_SOUTHWEST,
  HEX_NORTHWEST,
} HexDirection;

const LosTracePoint *los_trace_point_at(const LosTrace *trace, int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(trace->points, LOS_TRACE_CAPACITY,
                                  sizeof(*trace->points), (size_t)index);
}

static void los_trace_store(LosTrace *trace, int *count, int x, int y) {
  assert(*count < LOS_TRACE_CAPACITY);
  LosTracePoint *point =
      checked_storage_at(trace->points, LOS_TRACE_CAPACITY,
                         sizeof(*trace->points), (size_t)*count);
  point->x = x;
  point->y = y;
  ++*count;
}

static int los_trace_elevation(BattleMap *map, int x, int y) {
  const int elevation = (unsigned char)map_elevation_get(map, x, y);
  const char terrain = map_real_terrain_get(map, x, y);
  return terrain == BATTLE_TERRAIN_WATER || terrain == BATTLE_TERRAIN_ICE
             ? -elevation
             : elevation;
}

static void los_trace_store_best(BattleMap *map, LosTrace *trace, int *count,
                                 int x1, int y1, int x2, int y2) {
  if (x1 < 0 || x1 >= battle_map_width(map) || y1 < 0 ||
      y1 >= battle_map_height(map)) {
    los_trace_store(trace, count, x2, y2);
  } else if (x2 < 0 || x2 >= battle_map_width(map) || y2 < 0 ||
             y2 >= battle_map_height(map)) {
    los_trace_store(trace, count, x1, y1);
  } else if (los_trace_elevation(map, x1, y1) >
             los_trace_elevation(map, x2, y2)) {
    los_trace_store(trace, count, x1, y1);
  } else {
    los_trace_store(trace, count, x2, y2);
  }
}

static void los_map_coord_to_real(int x, int y, float *real_x, float *real_y) {
  *real_x = (float)x * ROOT3 / 2.0F * TRACESCALEMAP;
  *real_y = ((float)y - 0.5F * (float)(x % 2)) * TRACESCALEMAP;
}

static void GetAdjHex(int currx, int curry, HexDirection nexthex, int *x,
                      int *y) {
  switch (nexthex) {
  case HEX_NORTH:
    *x = currx;
    *y = curry - 1;
    break;
  case HEX_NORTHEAST:
    *x = currx + 1;
    *y = curry - (currx % 2);
    break;
  case HEX_SOUTHEAST:
    *x = currx + 1;
    *y = curry - (currx % 2) + 1;
    break;
  case HEX_SOUTH:
    *x = currx;
    *y = curry + 1;
    break;
  case HEX_SOUTHWEST:
    *x = currx - 1;
    *y = curry - (currx % 2) + 1;
    break;
  case HEX_NORTHWEST:
    *x = currx - 1;
    *y = curry - (currx % 2);
    break;
  default: /* Mostly there to satisfy gcc */
    (void)fprintf(stderr, "XXX ARGH: TraceLos doesn't know where to go!\n");
    *x = currx + 1; /* Just grab some values that aren't x/y */
    *y = curry + 1; /* so we can break out of the loop */
  }
}

int trace_los(BattleMap *map, int ax, int ay, int bx, int by, LosTrace *trace) {

  int i;                    /* Generic counter */
  float acx, acy, bcx, bcy; /* endpoints CARTESIAN coords */
  float currcx, currcy;     /* current hex CARTESIAN coords */
  int currx, curry;         /* current hex being worked from */
  int nextx, nexty;         /* x & y coords of next hex */
  int bestx = 0, besty = 0; /* best found so far */
  int xmul, ymul;           /* Used in 30/150/210/330 special case */
  HexDirection nexthex;     /* potential next hex being examined */
  float nextcx, nextcy;     /* Next hex's CARTESIAN coords */
  float slope;              /* slope of line */
  float uangle;             /* angle of line (in STD CARTESIAN FORM!) */
  float sinu;               /* sin of -uangle */
  float cosu;               /* cos of same */
  float liney;              /* TRANSFORMED y coord of the line */
  float tempangle;          /* temporary uangle used for effrad calc */
  float effrad;             /* effective radius of hex */
  float currdist;           /* distance along line of current hex */
  float nextdist;           /* distance along the line of potential hex */
  float bestdist;           /* "best" (not furthest) distance tried */
  float enddist;            /* distance along at end of line */
  int found_count = 0;

  /* Before doing anything, let's check for special circumstances, this */
  /* means vertical lines (which work using the current code, but depend */
  /* on atan returning proper vaules for atan(-Inf) -- which is probably */
  /* slow and may break on non-ANSI systems; and also lines at 30, 90 */
  /* etc.. degrees which contain 'ties' between hexes.  FASA rules here */
  /* say that the 'best' hex for the defender (the one that breaks LOS, */
  /* or gives a greater BTH penalty) should be used. */

  /* THE base case */
  if ((ax == bx) && (ay == by)) {
    los_trace_store(trace, &found_count, bx, by);
    return found_count;
  }
  /* Is it vertical? */
  if (ax == bx) {
    if (ay > by)
      for (i = ay - 1; i > by; i--)
        los_trace_store(trace, &found_count, ax, i);
    else
      for (i = ay + 1; i < by; i++)
        los_trace_store(trace, &found_count, ax, i);
    los_trace_store(trace, &found_count, bx, by);
    return found_count;
  }

  /* Does it lie along a 90 degree 'tie' direction? */
  /* IF(even-number-of-cols apart AND same-y-coord) */
  if (!((bx - ax) % 2) && ay == by) {
    currx = ax;
    i = bx > ax ? 1 : -1;
    while (currx != bx) {
      /* Do best of (currx+1,by-currx%2)   */
      /*         or (currx+1,by-currx%2+1) */
      los_trace_store_best(map, trace, &found_count, currx + 1 * i,
                           by - currx % 2, currx + 1 * i, by - currx % 2 + 1);

      if (currx != bx)
        los_trace_store(trace, &found_count, currx + 2 * i, by);

      currx += 2 * i;
    }

    return found_count;
  }

  /* Does it lie along a 30, 150, 210, 330 degree 'tie' direction? */
  /* This expression is messy, but it just means that a hex is along */
  /* 30 degrees if the y distance is (the integer part of) 3/2 */
  /* times the x distance, plus 1 if the x difference is odd, AND */
  /* the original hex was in an even column and heads in the +y  */
  /* direction, or odd and goes -y.  It works, try it :) */
  if (abs(by - ay) ==
      (3 * abs(bx - ax) / 2) +
          abs((bx - ax) % 2) * abs((by < ay) ? (ax % 2) : (1 - ax % 2))) {

    /* First get the x and y 'multipliers' -- either 1 or -1 */
    /* they determine the direction of the movement */
    if (bx > ax)
      if (by > ay) {
        xmul = 1;
        ymul = 1;
      } else {
        xmul = 1;
        ymul = -1;
      }
    else if (by > ay) {
      xmul = -1;
      ymul = 1;
    } else {
      xmul = -1;
      ymul = -1;
    }

    currx = ax;
    curry = ay;
    while ((currx != bx) && (curry != by)) {

      los_trace_store_best(
          map, trace, &found_count, currx, curry + ymul, currx + xmul,
          ymul == 1 ? curry + 1 - currx % 2 : curry - currx % 2);

      curry += (ymul == 1) ? (2 - currx % 2) : (-1 - currx % 2);
      currx += xmul;

      if (currx == bx && curry == by)
        continue;

      los_trace_store(trace, &found_count, currx, curry);
    }
    los_trace_store(trace, &found_count, currx, curry);
    return found_count;
  }

  /****************************************************************************/

  /****  OK, now we know it's not a special case ******************************/

  /****************************************************************************/

  /* First get the necessary constants set up */

  los_map_coord_to_real(ax, ay, &acx, &acy);
  los_map_coord_to_real(bx, by, &bcx, &bcy);

  slope = (float)(acy - bcy) / (float)(acx - bcx);

  uangle = -atanf(slope);

  sinu = sinf(uangle);
  cosu = cosf(uangle);

  liney = acx * sinu + acy * cosu; /* we could just as */
  /* correctly use bx, by */

  enddist = bcx * cosu - bcy * sinu;

  tempangle = fabsf(uangle);
  while (tempangle > DEG60)
    tempangle -= DEG60;
  effrad = cosf(tempangle - DEG30) * TRACESCALEMAP / ROOT3;

  /*****************************************************************/

  /**  OK, setup over, here's the loop:                            */

  /*****************************************************************/

  currx = ax;
  curry = ay;
  los_map_coord_to_real(currx, curry, &currcx, &currcy);
  currdist = currcx * cosu - currcy * sinu;
  bestdist = enddist; /* set this to the endpoint, the worst */
  /* possible point to go to  */

  while (!(currx == bx && curry == by)) {

    for (nexthex = HEX_NORTH; nexthex <= HEX_NORTHWEST; nexthex++) {

      GetAdjHex(currx, curry, nexthex, &nextx, &nexty);
      los_map_coord_to_real(nextx, nexty, &nextcx, &nextcy);

      /* Is it on the line? */
      if (fabsf((nextcx * sinu + nextcy * cosu) - liney) > effrad)
        continue;

      /* Where is it?  Find the transformed x coord */
      nextdist = nextcx * cosu - nextcy * sinu;

      /* is it forward of the current hex? */
      if (fabsf(enddist - nextdist) > fabsf(enddist - currdist))
        continue;

      /* Is it better than what we have? */
      if (fabsf(enddist - nextdist) >= fabsf(enddist - bestdist)) {
        bestdist = nextdist;
        bestx = nextx;
        besty = nexty;
      }
    }

    if (bestx == bx && besty == by) { /* If we've found the last hex, record */
      currx = bestx;                  /* and jump to the end of the loop */
      curry = besty;
      continue;
    }

    /* ********************************************************* */
    /* HERE is where you put the test code for intervening hexes */
    /* ********************************************************* */
    los_trace_store(trace, &found_count, bestx, besty);
    /* ********************************************************* */

    currx = bestx; /* Reset the curr hex for the next iteration */
    curry = besty;
    currdist = bestdist;
    bestdist = enddist; /* reset to worst possible value */
  }

  /* **************************************************** */
  /* HERE is where you put the test code for the LAST hex */
  /* **************************************************** */
  los_trace_store(trace, &found_count, currx, curry);
  /* ********************************************************* */
  return found_count;
}
