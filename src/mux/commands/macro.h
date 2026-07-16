/* macro.h - Player macro storage and macro-command declarations. */

#pragma once

#include "mux/database/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/world/match.h"

enum : int { MACRO_L = 1, MACRO_R = 2, MACRO_W = 4 };
constexpr int MAX_SLOTS = 5; /* Number of macro slots a person can have. */

typedef struct macroentry MACENT;
struct macroentry {
  const char *cmdname;
  void (*handler)(DbRef, char *);
};

struct macros {
  int player;
  char status;
  char *desc;
  int nummacros;
  int maxmacros;
  char *alias;   /* Chopped into 5 byte sections.  Macro can have  */
  char **string; /* at most a 4 letter alias                       */
};

extern int nummacros;
extern int maxmacros;
extern struct macros **macros;

static inline bool is_valid_macro_index(int n) {
  return n >= 0 && n < nummacros;
}

void init_mactab(void);
struct macros *get_macro_set(DbRef player, int which);
int can_write_macros(DbRef player, struct macros *m);
int can_read_macros(DbRef player, struct macros *m);

void clear_macro_set(int set);

int do_macro(DbRef player, char *in, char **out);
void do_add_macro(DbRef player, char *s);

void do_chown_macro(DbRef player, char *cmd);
void do_clear_macro(DbRef player, char *s);
void do_chmod_macro(DbRef player, char *s);
void do_create_macro(DbRef player, char *s);
void do_def_macro(DbRef player, char *cmd);
void do_del_macro(DbRef player, char *s);
void do_desc_macro(DbRef player, char *s);
void do_edit_macro(DbRef player, char *s);
void do_ex_macro(DbRef player, char *s);
void do_list_macro(DbRef player, char *s);
void do_status_macro(DbRef player, char *s);
void do_undef_macro(DbRef player, char *cmd);
void do_gex_macro(DbRef player, char *s);
char *do_process_macro(DbRef player, char *in, char *s);
