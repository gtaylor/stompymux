
/* functions.h - declarations for functions & function processing */

/* $Id: functions.h,v 1.3 2005/06/23 02:59:58 murrayma Exp $ */


#pragma once

#include <time.h>

#include "attrs.h"
#include "db.h"

typedef struct fun {
    const char *name;		/* function name */
    void (*fun) (char *, char **, dbref, dbref, char **, int, char **, int);		/* handler */
    int nargs;			/* Number of args needed or expected */
    int flags;			/* Function flags */
    int perms;			/* Access to function */
} FUN;

typedef struct ufun {
    const char *name;		/* function name */
    dbref obj;			/* Object ID */
    int atr;			/* Attribute ID */
    int flags;			/* Function flags */
    int perms;			/* Access to function */
    struct ufun *next;		/* Next ufun in chain */
} UFUN;

#define	FN_VARARGS	1	/* Function allows a variable # of args */
#define	FN_NO_EVAL	2	/* Don't evaluate args to function */
#define	FN_PRIV		4	/* Perform user-def function as holding obj */
#define FN_PRES		8	/* Preseve r-regs before user-def functions */

#define FUN_PROTO(name) void name(char *, char **, dbref, dbref, char **, int, char **, int)
FUN_PROTO(fun_alphamax);
FUN_PROTO(fun_alphamin);
FUN_PROTO(fun_andflags);
FUN_PROTO(fun_ansi);
FUN_PROTO(fun_art);
FUN_PROTO(fun_beep);
FUN_PROTO(fun_children);
FUN_PROTO(fun_clist);
FUN_PROTO(fun_cobj);
FUN_PROTO(fun_columns);
FUN_PROTO(fun_config);
FUN_PROTO(fun_create);
FUN_PROTO(fun_cwho);
FUN_PROTO(fun_dec);
FUN_PROTO(fun_decrypt);
FUN_PROTO(fun_default);
FUN_PROTO(fun_die);
FUN_PROTO(fun_edefault);
FUN_PROTO(fun_elements);
FUN_PROTO(fun_empty);
FUN_PROTO(fun_encrypt);
FUN_PROTO(fun_findable);
FUN_PROTO(fun_foreach);
FUN_PROTO(fun_grab);
FUN_PROTO(fun_graball);
FUN_PROTO(fun_grep);
FUN_PROTO(fun_grepi);
FUN_PROTO(fun_hasattr);
FUN_PROTO(fun_hasattrp);
FUN_PROTO(fun_hastype);
FUN_PROTO(fun_ifelse);
FUN_PROTO(fun_inc);
FUN_PROTO(fun_inzone);
FUN_PROTO(fun_isword);
FUN_PROTO(fun_items);
FUN_PROTO(fun_last);
FUN_PROTO(fun_link);
FUN_PROTO(fun_lit);
FUN_PROTO(fun_lparent);
FUN_PROTO(fun_lstack);
FUN_PROTO(fun_matchall);
FUN_PROTO(fun_mix);
FUN_PROTO(fun_munge);
FUN_PROTO(fun_objeval);
FUN_PROTO(fun_objmem);
FUN_PROTO(fun_orflags);
FUN_PROTO(fun_peek);
FUN_PROTO(fun_pemit);
FUN_PROTO(fun_playmem);
FUN_PROTO(fun_pop);
FUN_PROTO(fun_ports);
FUN_PROTO(fun_push);
FUN_PROTO(fun_regmatch);
FUN_PROTO(fun_scramble);
FUN_PROTO(fun_set);
FUN_PROTO(fun_setlock);
FUN_PROTO(fun_shl);
FUN_PROTO(fun_shr);
FUN_PROTO(fun_shuffle);
FUN_PROTO(fun_sortby);
FUN_PROTO(fun_squish);
FUN_PROTO(fun_strcat);
FUN_PROTO(fun_stripansi);
FUN_PROTO(fun_strtrunc);
FUN_PROTO(fun_tel);
FUN_PROTO(fun_translate);
FUN_PROTO(fun_udefault);
FUN_PROTO(fun_vadd);
FUN_PROTO(fun_valid);
FUN_PROTO(fun_vdim);
FUN_PROTO(fun_visible);
FUN_PROTO(fun_vmag);
FUN_PROTO(fun_vmul);
FUN_PROTO(fun_vsub);
FUN_PROTO(fun_vunit);
FUN_PROTO(fun_zexits);
FUN_PROTO(fun_zfun);
FUN_PROTO(fun_zobjects);
FUN_PROTO(fun_zone);
FUN_PROTO(fun_zplayers);
FUN_PROTO(fun_zrooms);
FUN_PROTO(fun_zwho);
#undef FUN_PROTO

extern void init_functab(void);
char *trim_space_sep(char *str, char sep);
char *next_token(char *str, char sep);
char *split_token(char **sp, char sep);
int list2arr(char *arr[], int maxlen, char *list, char sep);
void arr2list(char *arr[], int alen, char *list, char **bufc, char sep);
int nearby_or_control(dbref player, dbref thing);
int fn_range_check(const char *fname, int nfargs, int minargs, int maxargs,
				   char *result, char **bufc);
int delim_check(char *fargs[], int nfargs, int sep_arg, char *sep,
				char *buff, char **bufc, int eval, dbref player,
				dbref cause, char *cargs[], int ncargs);
int countwords(char *str, char sep);
time_t mytime(dbref player);
int do_convtime(char *str, struct tm *ttm);
char *get_uptime_to_string(int uptime);
char *get_uptime_to_short_string(int uptime);
int check_read_perms(dbref player, dbref thing, ATTR *attr, int aowner,
					 int aflags, char *buff, char **bufc);
int xlate(char *arg);
extern void list_functable(dbref);
extern dbref match_thing(dbref, char *);
void do_function(dbref player, dbref cause, int key, char *fname,
				 char *target);
int cf_func_access(int *vp, char *str, long extra, dbref player, char *cmd);
