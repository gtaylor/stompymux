
/* Declares the reusable terminal menu interface. */

#pragma once

#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

typedef struct StringifiedValue {
  char text[5];
} StringifiedValue;

/* #define MAX_MENU_LENGTH 24 */
constexpr int MAX_MENU_LENGTH = 400;
constexpr int MAX_MENU_WIDTH = 240;
constexpr int MENU_CHAR_WIDTH = 78;

/*
non-ticked toggle
   [a] ....    < >
ticked toggle
   [A] ....    <X>
value
   [a] ....    ___
string that can be changed
   [a] ...
string
   ...
   */

constexpr int CM_ONE = 0x001;       /* Just one / line */
constexpr int CM_TWO = 0x002;       /* Two / line */
constexpr int CM_THREE = 0x004;     /* Three / line */
constexpr int CM_FOUR = 0x008;      /* Four / line */
constexpr int CM_CENTER = 0x010;    /* Stuff's centered, not left-edge */
constexpr int CM_TOGGLE = 0x020;    /* Field that can be toggled */
constexpr int CM_NUMBER = 0x040;    /* Field with number in it (add/lower) */
constexpr int CM_LINE = 0x080;      /* No text, just blank line */
constexpr int CM_STRING = 0x100;    /* String with letter ahead of it */
constexpr int CM_NO_HILITE = 0x200; /* No extra highlight */
constexpr int CM_NOTOG = 0x400;     /* Not really toggleable */
constexpr int CM_NORIGHT = 0x800;   /* No right-end field */
constexpr int CM_NOCUT = 0x1000;    /* Turn off cutoff */

#define LETTERFIRST (CM_TOGGLE | CM_NUMBER | CM_STRING)
#define RIGHTEDGES (CM_TOGGLE | CM_NUMBER)

typedef struct CoolMenu {
  int id;       /* Used for some purposes by external agency */
  char *text;   /* Text (varies) */
  int value;    /* toggle = 0/1, number=0-999 */
  int maxvalue; /* if maxvalue's < 999 */
  char letter;  /* Letter allocated to this entry */
  int flags;    /* This entry's flags */
  struct CoolMenu *next;
} CoolMenu;

void CreateMenuEntry_Killer(CoolMenu **c, const char *text, int flag, int id,
                            int value, int maxvalue);

static inline void cool_menu_entry_normal(CoolMenu **menu, const char *text,
                                          int flags, int id, int max_value) {
  CreateMenuEntry_Killer(menu, text, flags, id, 0, max_value);
}

static inline void cool_menu_entry_simple(CoolMenu **menu, const char *text,
                                          int flags) {
  cool_menu_entry_normal(menu, text, flags, 0, 999);
}

static inline void cool_menu_entry_very_simple(CoolMenu **menu,
                                               const char *text) {
  cool_menu_entry_normal(menu, text, CM_ONE, 0, 999);
}

void KillCoolMenu(CoolMenu *c);
void ShowCoolMenu(EvaluationContext *evaluation, DbRef player, CoolMenu *c);
char **MakeCoolMenuText(CoolMenu *c, size_t *line_count);
int CoolMenu_FPWBit(int number, int maxlen);

/* Automated 'nice' looking menus: */
CoolMenu *SelCol_Menu(int columns, char *heading, char *const *strings,
                      size_t string_count, int type, int max);
CoolMenu *SelCol_ConstMenu(int columns, const char *heading,
                           const char *const strings[], size_t string_count,
                           int type, int max);

/* last = how many entries we have */
CoolMenu *SelCol_FunStringMenuK(int columns, char *heading, char *(*fun)(int),
                                int last);
CoolMenu *SelCol_FunStringMenuContextK(int columns, const char *heading,
                                       char *(*fun)(void *, int, char *buffer),
                                       void *context, int last);

/* Same, except we dunno how many entries we got */
CoolMenu *SelCol_FunStringMenu(int columns, char *heading, char *(*fun)(int));

static inline CoolMenu *auto_column_menu(char *heading, char **strings,
                                         size_t string_count, int type) {
  return SelCol_Menu(-1, heading, strings, string_count, type, 0);
}

static inline CoolMenu *auto_column_string_menu(char *heading, char **strings,
                                                size_t string_count) {
  return auto_column_menu(heading, strings, string_count, 0);
}

static inline CoolMenu *
auto_column_const_string_menu(const char *heading, const char *const strings[],
                              size_t string_count) {
  return SelCol_ConstMenu(-1, heading, strings, string_count, 0, 0);
}

static inline CoolMenu *selected_column_string_menu(int columns, char *heading,
                                                    char **strings,
                                                    size_t string_count) {
  return SelCol_Menu(columns, heading, strings, string_count, 0, 0);
}
