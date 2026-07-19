/*
   A C-program for MT19937, with initialization improved 2002/1/26.
   Coded by Takuji Nishimura and Makoto Matsumoto.

   The generator state is explicitly owned by BtechRandom.

   Copyright (C) 1997 - 2002, Makoto Matsumoto and Takuji Nishimura,
   All rights reserved.
   Copyright (C) 2005, Mutsuo Saito,
   All rights reserved.

   Redistribution and use in source and binary forms, with or without
   modification, are permitted provided that the following conditions
   are met:

     1. Redistributions of source code must retain the above copyright
        notice, this list of conditions and the following disclaimer.

     2. Redistributions in binary form must reproduce the above copyright
        notice, this list of conditions and the following disclaimer in the
        documentation and/or other materials provided with the distribution.

     3. The names of its contributors may not be used to endorse or promote
        products derived from this software without specific prior written
        permission.

   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
   "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
   LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
   A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT OWNER
   OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
   EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
   PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
   PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
   LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
   NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
   SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.


   Any feedback is very welcome.
   http://www.math.sci.hiroshima-u.ac.jp/~m-mat/MT/emt.html
   email: m-mat @ math.sci.hiroshima-u.ac.jp (remove space)
*/

#include "btech/random.h"

#include <assert.h>

/* Period parameters */
#define M 397
#define MATRIX_A 0x9908b0dfUL   /* constant vector a */
#define UPPER_MASK 0x80000000UL /* most significant w-r bits */
#define LOWER_MASK 0x7fffffffUL /* least significant r bits */

void btech_random_seed(BtechRandom *random, unsigned long seed) {
  assert(random != nullptr);
  random->state[0] = seed & 0xffffffffUL;
  for (random->index = 1; random->index < BTECH_RANDOM_STATE_SIZE;
       random->index++) {
    random->state[random->index] =
        (1812433253UL * (random->state[random->index - 1] ^
                         (random->state[random->index - 1] >> 30)) +
         random->index);
    /* See Knuth TAOCP Vol2. 3rd Ed. P.106 for multiplier. */
    /* In the previous versions, MSBs of the seed affect   */
    /* only MSBs of the array mt[].                        */
    /* 2002/01/09 modified by Makoto Matsumoto             */
    random->state[random->index] &= 0xffffffffUL;
    /* for >32 bit machines */
  }
  random->initialized = true;
}

/* generates a random number on [0,0xffffffff]-interval */
unsigned long btech_random_u32(BtechRandom *random) {
  unsigned long y;
  static const unsigned long mag01[2] = {0x0UL, MATRIX_A};
  /* mag01[x] = x * MATRIX_A  for x=0,1 */

  assert(random != nullptr);
  assert(random->initialized);
  assert(random->index <= BTECH_RANDOM_STATE_SIZE);

  if (random->index == BTECH_RANDOM_STATE_SIZE) {
    int kk;

    for (kk = 0; kk < BTECH_RANDOM_STATE_SIZE - M; kk++) {
      y = (random->state[kk] & UPPER_MASK) |
          (random->state[kk + 1] & LOWER_MASK);
      random->state[kk] = random->state[kk + M] ^ (y >> 1) ^ mag01[y & 0x1UL];
    }
    for (; kk < BTECH_RANDOM_STATE_SIZE - 1; kk++) {
      y = (random->state[kk] & UPPER_MASK) |
          (random->state[kk + 1] & LOWER_MASK);
      random->state[kk] = random->state[kk + (M - BTECH_RANDOM_STATE_SIZE)] ^
                          (y >> 1) ^ mag01[y & 0x1UL];
    }
    y = (random->state[BTECH_RANDOM_STATE_SIZE - 1] & UPPER_MASK) |
        (random->state[0] & LOWER_MASK);
    random->state[BTECH_RANDOM_STATE_SIZE - 1] =
        random->state[M - 1] ^ (y >> 1) ^ mag01[y & 0x1UL];

    random->index = 0;
  }

  y = random->state[random->index++];

  /* Tempering */
  y ^= (y >> 11);
  y ^= (y << 7) & 0x9d2c5680UL;
  y ^= (y << 15) & 0xefc60000UL;
  y ^= (y >> 18);

  return y;
}

/* generates a random number on [0,0x7fffffff]-interval */
long btech_random_i31(BtechRandom *random) {
  return (long)(btech_random_u32(random) >> 1);
}
