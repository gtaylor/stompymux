
/* macro.h */

/* $Id: macro.h,v 1.1 2005/06/13 20:50:47 murrayma Exp $ */

#pragma once

#include <stdio.h>

#include "db.h"
#include "interface.h"
#include "match.h"
#include "config.h"
#include "externs.h"

#define GMac(n) (n >= 0 && n < nummacros)

#define MACRO_L 1
#define MACRO_R 2
#define MACRO_W 4
#define MAX_SLOTS 5		/* Number of macro slots a person can have. */

typedef struct macroentry MACENT;
struct macroentry {
    char *cmdname;
    void (*handler) (dbref, char *);
};

struct macros {
    int player;
    char status;
    char *desc;
    int nummacros;
    int maxmacros;
    char *alias;		/* Chopped into 5 byte sections.  Macro can have  */
    char **string;		/* at most a 4 letter alias                       */
};

extern int nummacros;
extern int maxmacros;
extern struct macros **macros;

void init_mactab(void);
struct macros *get_macro_set(dbref player, int which);
int can_write_macros(dbref player, struct macros *m);
int can_read_macros(dbref player, struct macros *m);

void do_sort_macro_set(struct macros *m);
void save_macros(FILE *fp);
void load_macros(FILE *fp);
void clear_macro_set(int set);

int do_macro(dbref player, char *in, char **out);
void do_add_macro(dbref player, char *s);

void do_chown_macro(dbref player, char *cmd);
void do_clear_macro(dbref player, char *s);
void do_chmod_macro(dbref player, char *s);
void do_create_macro(dbref player, char *s);
void do_def_macro(dbref player, char *cmd);
void do_del_macro(dbref player, char *s);
void do_desc_macro(dbref player, char *s);
void do_edit_macro(dbref player, char *s);
void do_ex_macro(dbref player, char *s);
void do_list_macro(dbref player, char *s);
void do_status_macro(dbref player, char *s);
void do_undef_macro(dbref player, char *cmd);
void do_gex_macro(dbref player, char *s);
char *do_process_macro(dbref player, char *in, char *s);
