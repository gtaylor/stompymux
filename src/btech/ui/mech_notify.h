
/*
 * $Id: mech.notify.h,v 1.1.1.1 2005/01/11 21:18:20 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Sat Aug 31 17:44:51 1996 fingon
 * Last modified: Sat Jun  6 19:53:57 1998 fingon
 *
 */

#pragma once

constexpr int MECHPILOT = 0;
constexpr int MECHSTARTED = 1;
constexpr int MECHALL = 2;

#define cch(c) ccheck(player, mech, (c))
#define ccheck(a, b, c)                                                        \
  if (!common_checks((a), (b), (c)))                                           \
  return
