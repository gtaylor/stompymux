/* macro.h - Player macro storage and macro-command declarations. */

#pragma once

#include <stddef.h>

#include "mux/commands/command_context.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/commac.h"
#include "mux/server/platform.h"
#include "mux/server/server_registries.h"

struct MacroRegistry; // IWYU pragma: keep

enum : int { MACRO_L = 1, MACRO_R = 2, MACRO_W = 4 };
constexpr int MAX_SLOTS = 5; /* Number of macro slots a person can have. */

typedef struct MacroRegistry MacroRegistry;
typedef struct ChannelRegistry ChannelRegistry;
typedef struct Macroentry MACENT;
struct Macroentry {
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
MacroSet *macro_registry_item(const MacroRegistry *registry, size_t index);
MacroSet **macro_registry_slot(MacroRegistry *registry, size_t index);
char *macro_string_item(const MacroSet *set, size_t index);
char **macro_string_slot(MacroSet *set, size_t index);
char *macro_alias_at(const MacroSet *set, size_t index);

void init_mactab(CommandRegistry *commands);
typedef struct MacroSetRequest {
  MacroRegistry *registry;
  DbRef player;
  int slot;
} MacroSetRequest;

MacroSet *get_macro_set(const MacroSetRequest *request);
int can_write_macros(DbRef player, MacroSet *set);
int can_read_macros(GameDatabase *database, DbRef player, MacroSet *set);

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
                     char *description);
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
typedef struct MacroExpansionRequest {
  MacroRegistry *registry;
  DbRef player;
  char *input;
  char *arguments;
} MacroExpansionRequest;

char *do_process_macro(const MacroExpansionRequest *request);
