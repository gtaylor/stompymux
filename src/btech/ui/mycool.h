
/*
 * $Id: mycool.h,v 1.1 2005/06/13 20:50:52 murrayma Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Wed Oct 30 21:00:38 1996 fingon
 * Last modified: Sat Mar  8 19:01:50 1997 fingon
 *
 */

#pragma once

#include "coolmenu.h"

static inline void cool_menu_add(CoolMenu **menu, char *text) {
  cool_menu_entry_simple(menu, text, CM_TWO);
}

static inline void cool_menu_add_line(CoolMenu **menu) {
  cool_menu_entry_simple(menu, nullptr, CM_ONE | CM_LINE);
}

static inline void cool_menu_add_text(CoolMenu **menu, char *text) {
  cool_menu_entry_very_simple(menu, text);
}

static inline void cool_menu_add_with_flags(CoolMenu **menu, char *text,
                                            int flags) {
  cool_menu_entry_simple(menu, text, flags);
}

static inline void cool_menu_add_centered(CoolMenu **menu, char *text) {
  cool_menu_add_with_flags(menu, text, CM_ONE | CM_CENTER);
}
