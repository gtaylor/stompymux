
/* Declares BattleTech-specific terminal menu helpers. */

#pragma once

#include "coolmenu.h"

static inline void cool_menu_add(CoolMenu **menu, const char *text) {
  cool_menu_entry_simple(menu, text, CM_TWO);
}

static inline void cool_menu_add_line(CoolMenu **menu) {
  cool_menu_entry_simple(menu, nullptr, CM_ONE | CM_LINE);
}

static inline void cool_menu_add_text(CoolMenu **menu, const char *text) {
  cool_menu_entry_very_simple(menu, text);
}

static inline void cool_menu_add_with_flags(CoolMenu **menu, const char *text,
                                            int flags) {
  cool_menu_entry_simple(menu, text, flags);
}

static inline void cool_menu_add_centered(CoolMenu **menu, const char *text) {
  cool_menu_add_with_flags(menu, text, CM_ONE | CM_CENTER);
}
