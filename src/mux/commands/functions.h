/* functions.h - Built-in function handlers and function-processing support. */

#pragma once

#include <time.h>

#include "mux/database/attrs.h"
#include "mux/database/db.h"

typedef struct fun {
  const char *name; /* function name */
  void (*fun)(char *, char **, DbRef, DbRef, char **, int, char **,
              int); /* handler */
  int nargs;        /* Number of args needed or expected */
  int flags;        /* Function flags */
  int perms;        /* Access to function */
} FUN;

typedef struct ufun {
  const char *name;  /* function name */
  DbRef obj;         /* Object ID */
  int atr;           /* Attribute ID */
  int flags;         /* Function flags */
  int perms;         /* Access to function */
  struct ufun *next; /* Next ufun in chain */
} UFUN;

constexpr int FN_VARARGS = 1; /* Function allows a variable # of args */
constexpr int FN_NO_EVAL = 2; /* Don't evaluate args to function */
constexpr int FN_PRIV = 4;    /* Perform user-def function as holding obj */
constexpr int FN_PRES = 8;    /* Preseve r-regs before user-def functions */

typedef void FunProto(char *, char **, DbRef, DbRef, char **, int, char **,
                      int);

FunProto fun_alphamax;
FunProto fun_alphamin;
FunProto fun_andflags;
FunProto fun_ansi;
FunProto fun_art;
FunProto fun_beep;
FunProto fun_children;
FunProto fun_clist;
FunProto fun_cobj;
FunProto fun_columns;
FunProto fun_config;
FunProto fun_create;
FunProto fun_cwho;
FunProto fun_dec;
FunProto fun_decrypt;
FunProto fun_default;
FunProto fun_die;
FunProto fun_edefault;
FunProto fun_elements;
FunProto fun_empty;
FunProto fun_encrypt;
FunProto fun_findable;
FunProto fun_foreach;
FunProto fun_grab;
FunProto fun_graball;
FunProto fun_grep;
FunProto fun_grepi;
FunProto fun_hasattr;
FunProto fun_hasattrp;
FunProto fun_hastype;
FunProto fun_ifelse;
FunProto fun_inc;
FunProto fun_inzone;
FunProto fun_isword;
FunProto fun_items;
FunProto fun_last;
FunProto fun_link;
FunProto fun_lit;
FunProto fun_lparent;
FunProto fun_lstack;
FunProto fun_matchall;
FunProto fun_mix;
FunProto fun_munge;
FunProto fun_objeval;
FunProto fun_objmem;
FunProto fun_orflags;
FunProto fun_peek;
FunProto fun_pemit;
FunProto fun_playmem;
FunProto fun_pop;
FunProto fun_ports;
FunProto fun_push;
FunProto fun_regmatch;
FunProto fun_scramble;
FunProto fun_set;
FunProto fun_setlock;
FunProto fun_shl;
FunProto fun_shr;
FunProto fun_shuffle;
FunProto fun_sortby;
FunProto fun_squish;
FunProto fun_strcat;
FunProto fun_stripansi;
FunProto fun_strtrunc;
FunProto fun_tel;
FunProto fun_translate;
FunProto fun_udefault;
FunProto fun_vadd;
FunProto fun_valid;
FunProto fun_vdim;
FunProto fun_visible;
FunProto fun_vmag;
FunProto fun_vmul;
FunProto fun_vsub;
FunProto fun_vunit;
FunProto fun_zexits;
FunProto fun_zfun;
FunProto fun_zobjects;
FunProto fun_zone;
FunProto fun_zplayers;
FunProto fun_zrooms;
FunProto fun_zwho;

extern void init_functab(void);
char *trim_space_sep(char *str, char sep);
char *next_token(char *str, char sep);
char *split_token(char **sp, char sep);
int list2arr(char *arr[], int maxlen, char *list, char sep);
void arr2list(char *arr[], int alen, char *list, char **bufc, char sep);
int nearby_or_control(DbRef player, DbRef thing);
int fn_range_check(const char *fname, int nfargs, int minargs, int maxargs,
                   char *result, char **bufc);
int delim_check(char *fargs[], int nfargs, int sep_arg, char *sep, char *buff,
                char **bufc, int eval, DbRef player, DbRef cause, char *cargs[],
                int ncargs);
int countwords(char *str, char sep);
int do_convtime(char *str, struct tm *ttm);
char *get_uptime_to_string(int uptime);
int check_read_perms(DbRef player, DbRef thing, Attribute *attr, DbRef aowner,
                     long aflags, char *buff, char **bufc);
int xlate(char *arg);
extern void list_functable(DbRef);
extern DbRef match_thing(DbRef, char *);
void do_function(DbRef player, DbRef cause, int key, char *fname, char *target);
int cf_func_access(int *vp, char *str, long extra, DbRef player, char *cmd);
