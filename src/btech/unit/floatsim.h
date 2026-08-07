
/*
 * $Id: floatsim.h,v 1.1.1.1 2005/01/11 21:18:07 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1998 Markus Stenberg
 *       All rights reserved
 *
 * Created: Mon Jul 20 00:26:46 1998 fingon
 * Last modified: Mon Jul 20 00:46:33 1998 fingon
 *
 */

#pragma once

/* Simulate floats by using ints in interesting way */

constexpr int INT_DECIMAL_BITS = 8;

/* out of 32 */
constexpr int SHO_DECIMAL_BITS = 5;

/* out of 16 ; note : this makes signed ints only 0-1023, unsigneds 0-2047 */

static inline int float_simulation_to_int(int value) {
  return value >> INT_DECIMAL_BITS;
}

static inline int float_simulation_to_short(int value) {
  return value >> SHO_DECIMAL_BITS;
}

static inline int int_to_float_simulation(int value) {
  return value << INT_DECIMAL_BITS;
}

static inline int short_to_float_simulation(int value) {
  return value << SHO_DECIMAL_BITS;
}
