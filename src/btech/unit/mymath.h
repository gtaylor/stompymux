
/*
 * $Id: mymath.h,v 1.1.1.1 2005/01/11 21:18:29 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Wed Oct  9 10:49:02 1996 fingon
 * Last modified: Wed Oct  9 10:49:13 1996 fingon
 *
 */

#pragma once

#include <math.h>

#ifdef MAX
#undef MAX
#endif
#ifdef MIN
#undef MIN
#endif
constexpr float TWOPIOVER360 = 0.0174533F;
constexpr double PI = 3.141592654;
