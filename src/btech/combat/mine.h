
/*
 * $Id: mine.h,v 1.1.1.1 2005/01/11 21:18:29 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Tue Oct 22 18:32:22 1996 fingon
 * Last modified: Tue Oct 21 18:35:36 1997 fingon
 *
 */

#pragma once

#include <stdbool.h>

typedef enum MineType {
  MINE_STANDARD = 1,
  MINE_INFERNO = 2,
  MINE_COMMAND = 3,
  MINE_VIBRA = 4,
  /* Same as vibra, except shows no message and is not destroyed. */
  MINE_TRIGGER = 5,
  MINE_LOW = MINE_STANDARD,
  MINE_HIGH = MINE_TRIGGER,
} MineType;

static inline bool mine_type_is_vibrating(int type) {
  return type == MINE_VIBRA || type == MINE_TRIGGER;
}
