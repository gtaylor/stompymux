/* macro.h - Player macro storage and macro-command declarations. */

#pragma once

#include "mux/database/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/world/match.h"

enum : int { MACRO_L = 1, MACRO_R = 2, MACRO_W = 4 };
constexpr int MAX_SLOTS = 5; /* Number of macro slots a person can have. */

typedef struct MacroRegistry MacroRegistry;
typedef struct ChannelRegistry ChannelRegistry;
typedef struct macroentry MACENT;
struct macroentry {
  const char *cmdname;
  void (*handler)(MatchContext *, MacroRegistry *, DbRef, char *);
};

typedef struct MacroSet MacroSet;
struct MacroSet {
  int player;
  char status;
  char *desc;
  int macro_count;
  int macro_capacity;
  char *alias;   /* Chopped into 5 byte sections.  Macro can have  */
  char **string; /* at most a 4 letter alias                       */
};

struct MacroRegistry {
  ChannelRegistry *channels;
  MacroSet **sets;
  int count;
  int capacity;
};
typedef struct CommandRegistry CommandRegistry;

void macro_registry_initialize(MacroRegistry *registry,
                               ChannelRegistry *channels);
void macro_registry_destroy(MacroRegistry *registry);

void init_mactab(CommandRegistry *commands);
MacroSet *get_macro_set(MacroRegistry *registry, DbRef player, int which);
int can_write_macros(DbRef player, MacroSet *m);
int can_read_macros(GameDatabase *database, DbRef player, MacroSet *m);

void clear_macro_set(MacroRegistry *registry, int set);

int do_macro(MatchContext *match, CommandRegistry *commands,
             MacroRegistry *registry, DbRef player, char *in, char **out);
void do_add_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                  char *s);

void do_chown_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                    char *cmd);
void do_clear_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                    char *s);
void do_chmod_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                    char *s);
void do_create_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                     char *s);
void do_def_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                  char *cmd);
void do_del_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                  char *s);
void do_desc_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                   char *s);
void do_edit_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                   char *s);
void do_ex_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                 char *s);
void do_list_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                   char *s);
void do_status_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                     char *s);
void do_undef_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                    char *cmd);
void do_gex_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                  char *s);
char *do_process_macro(MacroRegistry *registry, DbRef player, char *in,
                       char *s);
