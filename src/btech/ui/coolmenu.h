
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

constexpr int LETTERFIRST = CM_TOGGLE | CM_NUMBER | CM_STRING;
constexpr int RIGHTEDGES = CM_TOGGLE | CM_NUMBER;

typedef struct CoolMenu {
  int id;       /* Used for some purposes by external agency */
  char *text;   /* Text (varies) */
  int value;    /* toggle = 0/1, number=0-999 */
  int maxvalue; /* if maxvalue's < 999 */
  char letter;  /* Letter allocated to this entry */
  int flags;    /* This entry's flags */
  struct CoolMenu *next;
} CoolMenu;

typedef struct CoolMenuEntryRequest {
  CoolMenu **menu;
  const char *text;
  int flags;
  int id;
  int value;
  int maximum_value;
} CoolMenuEntryRequest;

void cool_menu_entry_add(const CoolMenuEntryRequest *request);

static inline void cool_menu_entry_normal(CoolMenu **menu, const char *text,
                                          int flags, int id, int max_value) {
  cool_menu_entry_add(&(CoolMenuEntryRequest){.menu = menu,
                                              .text = text,
                                              .flags = flags,
                                              .id = id,
                                              .value = 0,
                                              .maximum_value = max_value});
}

static inline void cool_menu_entry_simple(CoolMenu **menu, const char *text,
                                          int flags) {
  cool_menu_entry_normal(menu, text, flags, 0, 999);
}

static inline void cool_menu_entry_very_simple(CoolMenu **menu,
                                               const char *text) {
  cool_menu_entry_normal(menu, text, CM_ONE, 0, 999);
}

void kill_cool_menu(CoolMenu *c);
void show_cool_menu(EvaluationContext *evaluation, DbRef player, CoolMenu *c);
char **make_cool_menu_text(CoolMenu *c, size_t *line_count);
int cool_menu_fpw_bit(int number, int maxlen);

typedef struct CoolMenuSelectionRequest {
  int columns;
  const char *heading;
  const char *const *strings;
  size_t string_count;
  int entry_type;
  int maximum_value;
} CoolMenuSelectionRequest;

/* Automated 'nice' looking menus: */
CoolMenu *cool_menu_selection_create(const CoolMenuSelectionRequest *request);

/* last = how many entries we have */
CoolMenu *sel_col_fun_string_menu_k(int columns, char *heading,
                                    char *(*fun)(int), int last);
CoolMenu *sel_col_fun_string_menu_context_k(int columns, const char *heading,
                                            char *(*fun)(void *, int,
                                                         char *buffer),
                                            void *context, int last);

/* Same, except we dunno how many entries we got */
CoolMenu *sel_col_fun_string_menu(int columns, char *heading,
                                  char *(*fun)(int));
