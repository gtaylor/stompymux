/* checked_conversion.h - Clamped conversions for legacy narrow storage. */

#pragma once

#include <limits.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>

static inline char clamp_int_to_char(int value) {
  if (value < CHAR_MIN)
    return CHAR_MIN;
  if (value > CHAR_MAX)
    return CHAR_MAX;
  return (char)value;
}

static inline short clamp_int_to_short(int value) {
  if (value < SHRT_MIN)
    return SHRT_MIN;
  if (value > SHRT_MAX)
    return SHRT_MAX;
  return (short)value;
}

static inline unsigned char clamp_int_to_unsigned_char(int value) {
  if (value < 0)
    return 0;
  if (value > UCHAR_MAX)
    return UCHAR_MAX;
  return (unsigned char)value;
}

static inline unsigned short clamp_int_to_unsigned_short(int value) {
  if (value < 0)
    return 0;
  if (value > USHRT_MAX)
    return USHRT_MAX;
  return (unsigned short)value;
}

static inline unsigned int clamp_int_to_unsigned_int(int value) {
  return value < 0 ? 0U : (unsigned int)value;
}

static inline int clamp_unsigned_int_to_int(unsigned int value) {
  return value > INT_MAX ? INT_MAX : (int)value;
}

static inline int clamp_intptr_to_int(intptr_t value) {
  if (value < INT_MIN)
    return INT_MIN;
  if (value > INT_MAX)
    return INT_MAX;
  return (int)value;
}

static inline int clamp_long_to_int(long value) {
  if (value < INT_MIN)
    return INT_MIN;
  if (value > INT_MAX)
    return INT_MAX;
  return (int)value;
}

static inline char clamp_long_to_char(long value) {
  if (value < CHAR_MIN)
    return CHAR_MIN;
  if (value > CHAR_MAX)
    return CHAR_MAX;
  return (char)value;
}

static inline int clamp_size_to_int(size_t value) {
  if (value > INT_MAX)
    return INT_MAX;
  return (int)value;
}

static inline short clamp_float_to_short(float value) {
  if (isnan(value))
    return 0;
  if (value < (float)SHRT_MIN)
    return SHRT_MIN;
  if (value > (float)SHRT_MAX)
    return SHRT_MAX;
  return (short)value;
}

static inline int clamp_float_to_int(float value) {
  if (isnan(value))
    return 0;
  if (value < (float)INT_MIN)
    return INT_MIN;
  if (value > (float)INT_MAX)
    return INT_MAX;
  return (int)value;
}
