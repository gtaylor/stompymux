
/*
 * $Id: econ.c,v 1.1.1.1 2005/01/11 21:18:06 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Sat Oct  5 14:06:02 1996 fingon
 * Last modified: Sat Apr 19 13:54:56 1997 fingon
 *
 */

#include "btech/context.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_partnames_api.h"
#include "mux/objects/economy_parts.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"

void econ_change_items(BtechContext *context, DbRef d, int id, int brand,
                       int num) {
  GameDatabase *database = context->database;
  int base;

  if (!is_good_obj(database, d))
    return;
  if (brand)
    if (get_parts_short_name(context, id, brand) ==
        get_parts_short_name(context, id, 0))
      brand = 0;
  base = economy_parts_quantity(database, d, id, brand);
  base += num;
  if (base <= 0) {
    economy_parts_set_quantity(database, d, id, brand, 0);
    return;
  }
  if (!(equipment_is_actuator(id)))
    economy_parts_set_quantity(database, d, id, brand, base);
  if (equipment_is_actuator(id))
    econ_change_items(context, d, cargo_equipment_index(S_ACTUATOR), brand,
                      base);
  /* Successfully changed */
}

int econ_find_items(BtechContext *context, DbRef d, int id, int brand) {
  GameDatabase *database = context->database;
  if (!is_good_obj(database, d))
    return 0;
  if (brand)
    if (get_parts_short_name(context, id, brand) ==
        get_parts_short_name(context, id, 0))
      brand = 0;
  return economy_parts_quantity(database, d, id, brand);
}

void econ_set_items(BtechContext *context, DbRef d, int id, int brand,
                    int num) {
  int i;

  if (!is_good_obj(context->database, d))
    return;
  i = econ_find_items(context, d, id, brand);
  if (i != num)
    econ_change_items(context, d, id, brand, num - i);
}
