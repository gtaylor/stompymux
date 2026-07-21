/*
 * functions.c - MUX function handlers
 */

#include "mux/server/platform.h"

#include <limits.h>
#include <math.h>

#include "mux/commands/command.h"
#include "mux/commands/functions.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/configuration.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include "mux/world/search.h"
#include "mux/world/walkdb.h"

extern NameTable indiv_attraccess_nametab[];

/*
 * Function definitions from funceval.c
 */

extern FunProto fun_btlag;
extern FunProto fun_btdesignex;
extern FunProto fun_btgetcharvalue;
extern FunProto fun_btmapelev;
extern FunProto fun_btmapterr;
extern FunProto fun_btsetcharvalue;
extern FunProto fun_btgetxcodevalue;
extern FunProto fun_btmakepilotroll;
extern FunProto fun_btsetxcodevalue;
extern FunProto fun_btstores;
extern FunProto fun_btstores_short;
extern FunProto fun_btunderrepair;
extern FunProto fun_btdamages;
extern FunProto fun_btsectstatus;
extern FunProto fun_btcritstatus;
extern FunProto fun_btarmorstatus;
extern FunProto fun_btsetarmorstatus;
extern FunProto fun_btweapons; /* AAA */
extern FunProto fun_btweaponstatus;
extern FunProto fun_btthreshold;
extern FunProto fun_btdamagemech;
extern FunProto fun_bttechstatus;
extern FunProto fun_btpartmatch;
extern FunProto fun_btpartname;
extern FunProto fun_btpartscategorylist;
extern FunProto fun_btpartslist;
extern FunProto fun_btloadmap;
extern FunProto fun_btloadmech;
extern FunProto fun_btmechfreqs;
extern FunProto fun_btmapunits;
extern FunProto fun_btgetxcodevalue_ref;
extern FunProto fun_btarmorstatus_ref;
extern FunProto fun_btcritstatus_ref;
extern FunProto fun_btweaponstatus_ref;
extern FunProto fun_btid2db;
extern FunProto fun_bthexlos;
extern FunProto fun_btlosm2m;
extern FunProto fun_bthexemit;
extern FunProto fun_zmechs;
extern FunProto fun_btgetweight;
extern FunProto fun_btpartweight;
extern FunProto fun_btaddstores;
extern FunProto fun_btupdatelinks;
extern FunProto fun_btremovestores;
extern FunProto fun_bttechtime;
extern FunProto fun_btcritslot;
extern FunProto fun_btcritslot_ref;
extern FunProto fun_btgetrange;
extern FunProto fun_btsetmaxspeed;
extern FunProto fun_btgetrealmaxspeed;
extern FunProto fun_btgetbv;
extern FunProto fun_btgetbv_ref;
extern FunProto fun_btgetdbv_ref;
extern FunProto fun_btgetobv_ref;
extern FunProto fun_btgetbv2_ref;
extern FunProto fun_btgetbv2;
extern FunProto fun_bttechlist;
extern FunProto fun_bttechlist_ref;
extern FunProto fun_btpayload_ref;
extern FunProto fun_btshowstatus_ref;
extern FunProto fun_btshowwspecs_ref;
extern FunProto fun_btshowcritstatus_ref;
extern FunProto fun_btengrate;
extern FunProto fun_btengrate_ref;
extern FunProto fun_btweapstat;
extern FunProto fun_btnumrepjobs;
extern FunProto fun_btsetxy;
extern FunProto fun_btsettons;
extern FunProto fun_btmapemit;
extern FunProto fun_btparttype;
extern FunProto fun_btticweaps;
#ifdef BT_ADVANCED_ECON
extern FunProto fun_btgetpartcost;
extern FunProto fun_btsetpartcost;
extern FunProto fun_btfasabasecost_ref;
#endif
extern FunProto fun_btunitfixable;
extern FunProto fun_btunitpartslist;
extern FunProto fun_btunitpartslist_ref;
extern FunProto fun_btlistblz;
extern FunProto fun_bthexinblz;
extern FunProto fun_btcharlist;
extern FunProto fun_cobj;
extern FunProto fun_config;

extern FunProto fun_cwho;
extern FunProto fun_clist;
extern FunProto fun_cemit;
extern FunProto fun_beep;
extern FunProto fun_ansi;
extern FunProto fun_zone;
extern FunProto fun_link;
extern FunProto fun_tel;
extern FunProto fun_pemit;
extern FunProto fun_create;
extern FunProto fun_set;
extern FunProto fun_last;
extern FunProto fun_matchall;
extern FunProto fun_ports;
extern FunProto fun_visible;
extern FunProto fun_elements;
extern FunProto fun_grab;
extern FunProto fun_graball;
extern FunProto fun_scramble;
extern FunProto fun_shuffle;
extern FunProto fun_findable;
extern FunProto fun_isword;
extern FunProto fun_hasattr;
extern FunProto fun_hasattrp;
extern FunProto fun_zwho;
extern FunProto fun_zrooms;
extern FunProto fun_zexits;
extern FunProto fun_zobjects;
extern FunProto fun_zplayers;
extern FunProto fun_inzone;
extern FunProto fun_encrypt;
extern FunProto fun_decrypt;
extern FunProto fun_objeval;
extern FunProto fun_squish;
extern FunProto fun_stripansi;
extern FunProto fun_columns;
extern FunProto fun_playmem;
extern FunProto fun_objmem;
extern FunProto fun_orflags;
extern FunProto fun_andflags;
extern FunProto fun_strtrunc;
extern FunProto fun_ifelse;
extern FunProto fun_inc;
extern FunProto fun_dec;
extern FunProto fun_die;
extern FunProto fun_lit;
extern FunProto fun_shl;
extern FunProto fun_shr;
extern FunProto fun_vadd;
extern FunProto fun_vsub;
extern FunProto fun_vmul;
extern FunProto fun_vmag;
extern FunProto fun_vunit;
extern FunProto fun_vdim;
extern FunProto fun_strcat;
extern FunProto fun_art;
extern FunProto fun_alphamax;
extern FunProto fun_alphamin;
extern FunProto fun_valid;
extern FunProto fun_hastype;
extern FunProto fun_empty;
extern FunProto fun_push;
extern FunProto fun_peek;
extern FunProto fun_pop;
extern FunProto fun_items;
extern FunProto fun_lstack;
extern FunProto fun_regmatch;
extern FunProto fun_translate;

/*
 * This is for functions that take an optional delimiter character
 */

#define varargs_preamble(xname, xnargs)                                        \
  if (!fn_range_check(xname, nfargs, xnargs - 1, xnargs, buff, bufc))          \
    return;                                                                    \
  if (!delim_check(fargs, nfargs, xnargs, &sep, buff, bufc, 0, player, cause,  \
                   cargs, ncargs, context))                                    \
    return;

#define evarargs_preamble(xname, xnargs)                                       \
  if (!fn_range_check(xname, nfargs, xnargs - 1, xnargs, buff, bufc))          \
    return;                                                                    \
  if (!delim_check(fargs, nfargs, xnargs, &sep, buff, bufc, 1, player, cause,  \
                   cargs, ncargs, context))                                    \
    return;

#define mvarargs_preamble(xname, xminargs, xnargs)                             \
  if (!fn_range_check(xname, nfargs, xminargs, xnargs, buff, bufc))            \
    return;                                                                    \
  if (!delim_check(fargs, nfargs, xnargs, &sep, buff, bufc, 0, player, cause,  \
                   cargs, ncargs, context))                                    \
    return;

/**
 * Trim off leading and trailing spaces if the separator char is a space
 */
char *trim_space_sep(char *str, char sep) {
  char *p;

  if (*str == '\0')
    return str;
  if (sep != ' ')
    return str;
  while (*str && (*str == ' '))
    str++;
  for (p = str; *p; p++)
    ;
  for (p--; *p == ' ' && p > str; p--)
    ;
  p++;
  *p = '\0';
  return str;
}

/*
 * next_token: Point at start of next token in string
 */

char *next_token(char *str, char sep) {
  while (*str && (*str != sep))
    str++;
  if (!*str)
    return nullptr;
  str++;
  if (sep == ' ') {
    while (*str == sep)
      str++;
  }
  return str;
}

/*
 * split_token: Get next token from string as null-term string.  String is
 * * destructively modified.
 */

char *split_token(char **sp, char sep) {
  char *str, *save;

  save = str = *sp;
  if (!str) {
    *sp = nullptr;
    return nullptr;
  }
  while (*str && (*str != sep))
    str++;
  if (*str) {
    *str++ = '\0';
    if (sep == ' ') {
      while (*str == sep)
        str++;
    }
  } else {
    str = nullptr;
  }
  *sp = str;
  return save;
}

DbRef match_thing(MatchContext *match, DbRef player, char *name) {
  init_match(match, player, name, NOTYPE);
  match_everything(match, 0);
  return noisy_match_result(match);
}

/*
 * ---------------------------------------------------------------------------
 * * List management utilities.
 */

constexpr int ALPHANUM_LIST = 1;
constexpr int NUMERIC_LIST = 2;
constexpr int DBREF_LIST = 3;
constexpr int FLOAT_LIST = 4;

static int autodetect_list(char *ptrs[], int nitems) {
  int sort_type, i;
  char *p;

  sort_type = NUMERIC_LIST;
  for (i = 0; i < nitems; i++) {
    switch (sort_type) {
    case NUMERIC_LIST:
      if (!is_number(ptrs[i])) {

        /*
         * If non-numeric, switch to alphanum sort. *
         *
         * *  * *  * * Exception: if this is the
         * first * element * * and * it is a good
         * dbref, * switch to a * * dbref sort. *
         * We're a * little looser than *  * the
         * normal * 'good  * dbref' rules, any * *
         * number following # * the #-sign is
         * accepted.
         */

        if (i != 0) {
          return ALPHANUM_LIST;
        }
        p = ptrs[i];
        if (*p++ != NUMBER_TOKEN || !is_integer(p)) {
          return ALPHANUM_LIST;
        }
        sort_type = DBREF_LIST;
      } else if (index(ptrs[i], '.')) {
        sort_type = FLOAT_LIST;
      }
      break;
    case FLOAT_LIST:
      if (!is_number(ptrs[i])) {
        sort_type = ALPHANUM_LIST;
        return ALPHANUM_LIST;
      }
      break;
    case DBREF_LIST:
      p = ptrs[i];
      if (*p++ != NUMBER_TOKEN)
        return ALPHANUM_LIST;
      if (!is_integer(p))
        return ALPHANUM_LIST;
      break;
    default:
      return ALPHANUM_LIST;
    }
  }
  return sort_type;
}

static int get_list_type(char *fargs[], int nfargs, int type_pos, char *ptrs[],
                         int nitems) {
  if (nfargs >= type_pos) {
    switch (ToLower(*fargs[type_pos - 1])) {
    case 'd':
      return DBREF_LIST;
    case 'n':
      return NUMERIC_LIST;
    case 'f':
      return FLOAT_LIST;
    case '\0':
      return autodetect_list(ptrs, nitems);
    default:
      return ALPHANUM_LIST;
    }
  }
  return autodetect_list(ptrs, nitems);
}

int list2arr(char *arr[], int maxlen, char *list, char sep) {
  char *p;
  int i;

  list = trim_space_sep(list, sep);
  p = split_token(&list, sep);
  for (i = 0; p && i < maxlen; i++, p = split_token(&list, sep)) {
    arr[i] = p;
  }
  return i;
}

void arr2list(char *arr[], int alen, char *list, char **bufc, char sep) {
  int i;

  for (i = 0; i < alen; i++) {
    safe_str(arr[i], list, bufc);
    safe_chr(sep, list, bufc);
  }
  if (*bufc != list)
    (*bufc)--;
}

static int dbnum(char *dbr) {
  if ((strlen(dbr) < 2) && (*dbr != '#'))
    return 0;
  else
    return atoi(dbr + 1);
}

/**
 * Check if player is near or controls thing
 */
int nearby_or_control(EvaluationContext *context, DbRef player, DbRef thing) {
  if (!is_good_obj(context->world->database, player) ||
      !is_good_obj(context->world->database, thing))
    return 0;
  if (is_controls(context, player, thing))
    return 1;
  if (!nearby(context->world->database, player, thing))
    return 0;
  return 1;
}

/**
 * Copy the floating point value into a buffer and make it presentable
 */
static void fval(char *buff, char **bufc, double result) {
  char *p, *buf1;

  buf1 = *bufc;
  safe_tprintf_str(buff, bufc, "%.6f", result); /*
                                                 * get double val * *
                                                 * into buffer
                                                 */
  **bufc = '\0';
  p = (char *)rindex(buf1, '0');
  if (p == nullptr) { /*
                       * remove useless trailing 0's
                       */
    return;
  } else if (*(p + 1) == '\0') {
    while (*p == '0') {
      *p-- = '\0';
    }
    *bufc = p + 1;
  }
  p = (char *)rindex(buf1, '.'); /*
                                  * take care of dangling '.'
                                  */
  if ((p != nullptr) && (*(p + 1) == '\0')) {
    *p = '\0';
    *bufc = p;
  }
}

/**
 * Check # of args to a function with an optional argument
 * for validity.
 */
int fn_range_check(const char *fname, int nfargs, int minargs, int maxargs,
                   char *result, char **bufc) {
  if ((nfargs >= minargs) && (nfargs <= maxargs))
    return 1;

  if (maxargs == (minargs + 1))
    safe_tprintf_str(result, bufc,
                     "#-1 FUNCTION (%s) EXPECTS %d OR %d ARGUMENTS", fname,
                     minargs, maxargs);
  else
    safe_tprintf_str(result, bufc,
                     "#-1 FUNCTION (%s) EXPECTS BETWEEN %d AND %d ARGUMENTS",
                     fname, minargs, maxargs);
  return 0;
}

/**
 * * delim_check: obtain delimiter
 */
int delim_check(char *fargs[], int nfargs, int sep_arg, char *sep, char *buff,
                char **bufc, int eval, DbRef player, DbRef cause, char *cargs[],
                int ncargs, EvaluationContext *context) {
  char *tstr, *bp, *str;
  size_t tlen;

  if (nfargs >= sep_arg) {
    tlen = strlen(fargs[sep_arg - 1]);
    if (tlen <= 1)
      eval = 0;
    if (eval) {
      tstr = bp = alloc_lbuf("delim_check");
      str = fargs[sep_arg - 1];
      exec(context, tstr, &bp, 0, player, cause, EV_EVAL | EV_FCHECK, &str,
           cargs, ncargs);
      *bp = '\0';
      tlen = strlen(tstr);
      *sep = *tstr;
      free_lbuf(tstr);
    }
    if (tlen == 0) {
      *sep = ' ';
    } else if (tlen != 1) {
      safe_str("#-1 SEPARATOR MUST BE ONE CHARACTER", buff, bufc);
      return 0;
    } else if (!eval) {
      *sep = *fargs[sep_arg - 1];
    }
  } else {
    *sep = ' ';
  }
  return 1;
}

/**
 * Returns number of words in a string.
 * Added 1/28/91 Philip D. Wasson
 */
int countwords(char *str, char sep) {
  int n;

  str = trim_space_sep(str, sep);
  if (!*str)
    return 0;
  for (n = 0; str; str = next_token(str, sep), n++)
    ;
  return n;
}

static void fun_words(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  char sep;

  if (nfargs == 0) {
    safe_str("0", buff, bufc);
    return;
  }
  varargs_preamble("WORDS", 2);
  safe_tprintf_str(buff, bufc, "%d", countwords(fargs[0], sep));
}

/**
 * Returns the flags on an object.
 * Because @switch is case-insensitive, not quite as useful as it could be.
 */
static void fun_flags(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  DbRef it;
  char *buff2;

  it = match_thing(&context->command->match, player, fargs[0]);
  if ((it != NOTHING) &&
      (is_examinable(context, player, it) || (it == cause))) {
    buff2 = unparse_flags(context->world->database, player, it);
    safe_str(buff2, buff, bufc);
    free_sbuf(buff2);
  } else
    safe_str("#-1", buff, bufc);
  return;
}

/**
 * Return a random number from 0 to arg1-1
 */
static void fun_rand(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  int num;

  num = atoi(fargs[0]);
  if (num < 1)
    safe_str("0", buff, bufc);
  else
    safe_tprintf_str(buff, bufc, "%ld", (random() % num));
}

/**
 * Returns the absolute value of its argument.
 */
static void fun_abs(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  double num;

  num = atof(fargs[0]);
  if (num == 0.0) {
    safe_str("0", buff, bufc);
  } else if (num < 0.0) {
    fval(buff, bufc, -num);
  } else {
    fval(buff, bufc, num);
  }
}

/**
 * Returns -1, 0, or 1 based on the the sign of its argument.
 */
static void fun_sign(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  double num;

  num = atof(fargs[0]);
  if (num < 0)
    safe_str("-1", buff, bufc);
  else if (num > 0)
    safe_str("1", buff, bufc);
  else
    safe_str("0", buff, bufc);
}

/* Returns secs converted to digits, just like WHO does for connect time */

static void fun_digittime(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  register struct tm *delta;
  static char buf[64] = {0};
  time_t dt;

  dt = atol(fargs[0]);

  if (dt < 0)
    dt = 0;

  delta = gmtime(&dt);
  if (delta->tm_yday > 0) {
    snprintf(buf, sizeof(buf), "%dd %02d:%02d", delta->tm_yday, delta->tm_hour,
             delta->tm_min);
  } else {
    snprintf(buf, sizeof(buf), "%02d:%02d", delta->tm_hour, delta->tm_min);
  }

  safe_tprintf_str(buff, bufc, "%s", buf);
}

/**
 * Returns nicely-formatted time.
 */
static void fun_time(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  char *temp;

  temp = (char *)ctime(&context->runtime->clock->now);
  temp[strlen(temp) - 1] = '\0';
  safe_str(temp, buff, bufc);
}

/**
 * Seconds since 0:00 1/1/70
 */
static void fun_secs(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%ld", context->runtime->clock->now);
}

/**
 * Converts seconds to time string, based off 0:00 1/1/70
 */
static void fun_convsecs(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  char *temp;
  time_t tt;

  tt = atol(fargs[0]);
  temp = (char *)ctime(&tt);
  temp[strlen(temp) - 1] = '\0';
  safe_str(temp, buff, bufc);
}

/**
 * converts time string to seconds, based off 0:00 1/1/70
 * additional auxiliary function and table used to parse time string,
 * since no ANSI standard function are available to do this.
 */
static const char *monthtab[] = {"Jan", "Feb", "Mar", "Apr", "May", "Jun",
                                 "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"};

static const char daystab[] = {31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};

/**
 * Converts time string to a struct tm. Returns 1 on success, 0 on fail.
 * Time string format is always 24 characters long, in format
 * Ddd Mmm DD HH:MM:SS YYYY
 */

static void get_substr(char *buf, char **p) {
  *p = (char *)index(buf, ' ');
  if (*p) {
    *(*p)++ = '\0';
    while (**p == ' ')
      (*p)++;
  }
}

static inline bool is_leapyear_1900(int yr) {
  return yr % 400 == 100 || (yr % 100 != 0 && yr % 4 == 0);
}

int do_convtime(const ServerConfiguration *configuration, char *str,
                struct tm *ttm) {
  char *buf, *p, *q;
  int i;

  if (!str || !ttm)
    return 0;
  while (*str == ' ')
    str++;
  buf = p = alloc_sbuf("do_convtime"); /*
                                        * make a temp copy of arg
                                        */
  safe_sb_str(str, buf, &p);
  *p = '\0';

  get_substr(buf, &p); /*
                        * day-of-week or month
                        */
  if (!p || strlen(buf) != 3) {
    free_sbuf(buf);
    return 0;
  }
  for (i = 0; (i < 12) && string_compare(configuration, monthtab[i], p); i++)
    ;
  if (i == 12) {
    get_substr(p, &q); /*
                        * month
                        */
    if (!q || strlen(p) != 3) {
      free_sbuf(buf);
      return 0;
    }
    for (i = 0; (i < 12) && string_compare(configuration, monthtab[i], p); i++)
      ;
    if (i == 12) {
      free_sbuf(buf);
      return 0;
    }
    p = q;
  }
  ttm->tm_mon = i;

  get_substr(p, &q); /*
                      * day of month
                      */
  if (!q || (ttm->tm_mday = atoi(p)) < 1 || ttm->tm_mday > daystab[i]) {
    free_sbuf(buf);
    return 0;
  }
  p = (char *)index(q, ':'); /*
                              * hours
                              */
  if (!p) {
    free_sbuf(buf);
    return 0;
  }
  *p++ = '\0';
  if ((ttm->tm_hour = atoi(q)) > 23 || ttm->tm_hour < 0) {
    free_sbuf(buf);
    return 0;
  }
  if (ttm->tm_hour == 0) {
    while (isspace(*q))
      q++;
    if (*q != '0') {
      free_sbuf(buf);
      return 0;
    }
  }
  q = (char *)index(p, ':'); /*
                              * minutes
                              */
  if (!q) {
    free_sbuf(buf);
    return 0;
  }
  *q++ = '\0';
  if ((ttm->tm_min = atoi(p)) > 59 || ttm->tm_min < 0) {
    free_sbuf(buf);
    return 0;
  }
  if (ttm->tm_min == 0) {
    while (isspace(*p))
      p++;
    if (*p != '0') {
      free_sbuf(buf);
      return 0;
    }
  }
  get_substr(q, &p); /*
                      * seconds
                      */
  if (!p || (ttm->tm_sec = atoi(q)) > 59 || ttm->tm_sec < 0) {
    free_sbuf(buf);
    return 0;
  }
  if (ttm->tm_sec == 0) {
    while (isspace(*q))
      q++;
    if (*q != '0') {
      free_sbuf(buf);
      return 0;
    }
  }
  get_substr(p, &q); /*
                      * year
                      */
  if ((ttm->tm_year = atoi(p)) == 0) {
    while (isspace(*p))
      p++;
    if (*p != '0') {
      free_sbuf(buf);
      return 0;
    }
  }
  free_sbuf(buf);
  if (ttm->tm_year > 100)
    ttm->tm_year -= 1900;
  if (ttm->tm_year < 0) {
    return 0;
  }
  return (ttm->tm_mday != 29 || i != 1 || is_leapyear_1900(ttm->tm_year));
}

static void fun_convtime(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  struct tm *ttm;

  ttm = localtime(&context->runtime->clock->now);
  if (do_convtime(context->world->configuration, fargs[0], ttm))
    safe_tprintf_str(buff, bufc, "%ld", mktime(ttm));
  else
    safe_str("-1", buff, bufc);
}

/**
 * Converts number to minutes/secs/days
 */

constexpr int UPTIME_UNITS = 6;

struct {
  int multip;
  const char *name;
  const char *sname;
} uptime_unit_table[UPTIME_UNITS] = {{60 * 60 * 24 * 30 * 12, "year", "y"},
                                     {60 * 60 * 24 * 30, "month", "m"},
                                     {60 * 60 * 24, "day", "d"},
                                     {60 * 60, "hour", "h"},
                                     {60, "minute", "m"},
                                     {1, "second", "s"}};

char *get_uptime_to_string(int uptime) {
  char *buf = alloc_sbuf("get_uptime_to_string");
  int units[UPTIME_UNITS];
  int taim = uptime;
  int ut = 0, uc = 0, foofaa;

  if (uptime <= 0) {
    strlcpy(buf, "#-1 INVALID VALUE", SBUF_SIZE);
    return buf;
  }
  for (ut = 0; ut < UPTIME_UNITS; ut++)
    units[ut] = 0;
  ut = 0;
  buf[0] = 0;
  while (taim > 0) {
    if ((foofaa = (taim / uptime_unit_table[ut].multip)) > 0) {
      uc++;
      units[ut] = foofaa;
      taim -= uptime_unit_table[ut].multip * foofaa;
    }
    ut++;
  }
  /*
   * Now, we got it..
   */
  for (ut = 0; ut < UPTIME_UNITS; ut++) {
    if (units[ut]) {
      uc--;
      if (units[ut] > 1)
        snprintf(buf + strlen(buf), SBUF_SIZE - strlen(buf), "%d %ss",
                 units[ut], uptime_unit_table[ut].name);
      else
        snprintf(buf + strlen(buf), SBUF_SIZE - strlen(buf), "%d %s", units[ut],
                 uptime_unit_table[ut].name);
      if (uc > 1)
        strlcat(buf, ", ", SBUF_SIZE);
      else if (uc > 0)
        strlcat(buf, " and ", SBUF_SIZE);
    }
  }
  return buf;
}

static void fun_convuptime(char *buff, char **bufc, DbRef player, DbRef cause,
                           char *fargs[], int nfargs, char *cargs[], int ncargs,
                           EvaluationContext *context) {
  char *uptimestring = get_uptime_to_string(atoi(fargs[0]));

  safe_str(uptimestring, buff, bufc);
  free_sbuf(uptimestring);
}

/**
 * What time did this system last reboot?
 */
static void fun_starttime(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  char *temp;

  temp = (char *)ctime(context->runtime->start_time);
  temp[strlen(temp) - 1] = '\0';
  safe_str(temp, buff, bufc);
}

/**
 * What time (in seconds) did this system last reboot?
 */
static void fun_startsecs(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%ld", *context->runtime->start_time);
}

/**
 * What is the record number of players connected?
 */
static void fun_connrecord(char *buff, char **bufc, DbRef player, DbRef cause,
                           char *fargs[], int nfargs, char *cargs[], int ncargs,
                           EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%d", *context->runtime->record_players);
}

static void fun_subeval(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  char *str;

  if (nfargs != 1) {
    safe_str("#-1 FUNCTION (EVALNOCOMP) EXPECTS 1 OR 2 ARGUMENTS", buff, bufc);
    return;
  }

  str = fargs[0];
  exec(context, buff, bufc, 0, player, cause,
       EV_NO_LOCATION | EV_NOFCHECK | EV_FIGNORE | EV_NO_COMPRESS, &str,
       (char **)nullptr, 0);
}

static void fun_eval(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  if (nfargs != 1) {
    safe_str("#-1 FUNCTION (EVAL) EXPECTS 1 ARGUMENT", buff, bufc);
    return;
  }
  char *expression = fargs[0];
  exec(context, buff, bufc, 0, player, cause, EV_EVAL, &expression,
       (char **)nullptr, 0);
}

/**
 * Call a user-defined function.
 */

/**
 * Make list from evaluating arg3 with each member of arg2.
 * arg1 specifies a delimiter character to use in the parsing of arg2.
 * NOTE: This function expects that its arguments have not been evaluated.
 */
static void fun_parse(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  char *curr, *objstring, *buff2, *buff3, *cp, sep;
  char *dp, *str;
  int first, number = 0;
  char buffer[MBUF_SIZE];

  evarargs_preamble("PARSE", 3);
  cp = curr = dp = alloc_lbuf("fun_parse");
  str = fargs[0];
  exec(context, curr, &dp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
       &str, cargs, ncargs);
  *dp = '\0';
  cp = trim_space_sep(cp, sep);
  if (!*cp) {
    free_lbuf(curr);
    return;
  }
  first = 1;
  while (cp) {
    if (!first)
      safe_chr(' ', buff, bufc);
    first = 0;
    number++;
    objstring = split_token(&cp, sep);
    buff2 = replace_string(BOUND_VAR, objstring, fargs[1]);
    snprintf(buffer, MBUF_SIZE - 1, "%d", number);
    buff3 = replace_string(LISTPLACE_VAR, buffer, buff2);
    str = buff3;
    exec(context, buff, bufc, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
         &str, cargs, ncargs);
    free_lbuf(buff2);
    free_lbuf(buff3);
  }
  free_lbuf(curr);
}

/**
 * mid(foobar,2,3) returns oba
 */
static void fun_mid(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int l, len;
  char *oldp;
  char new[LBUF_SIZE];

  oldp = *bufc;
  l = atoi(fargs[1]);
  len = atoi(fargs[2]);
  if ((l < 0) || (len < 0) || ((len + l) > LBUF_SIZE) || ((len + 1) < 0)) {
    safe_str("#-1 OUT OF RANGE", buff, bufc);
    return;
  }
  strncpy(new, fargs[0], LBUF_SIZE - 1);
  if ((size_t)l < strlen(strip_ansi_r(new, fargs[0], strlen(fargs[0]))))
    safe_str(strip_ansi_r(new, fargs[0], strlen(fargs[0])) + l, buff, bufc);
  oldp[len] = 0;
  if ((oldp + len) < *bufc) {
    *bufc = oldp + len;
  }
}

/**
 * Returns first word in a string
 */
static void fun_first(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  char *s, *first, sep;

  /*
   * If we are passed an empty arglist return a null string
   */

  if (nfargs == 0) {
    return;
  }
  varargs_preamble("FIRST", 2);
  s = trim_space_sep(fargs[0], sep); /*
                                      * leading spaces ...
                                      */
  first = split_token(&s, sep);
  if (first) {
    safe_str(first, buff, bufc);
  }
}

/**
 * Returns all but the first word in a string
 */
static void fun_rest(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  char *s, sep;

  /*
   * If we are passed an empty arglist return a null string
   */

  if (nfargs == 0) {
    return;
  }
  varargs_preamble("REST", 2);
  s = trim_space_sep(fargs[0], sep); /*
                                      * leading spaces ...
                                      */
  split_token(&s, sep);
  if (s) {
    safe_str(s, buff, bufc);
  }
}

/**
 * Function form of %-substitution
 */
static void fun_v(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  char *sbuf = alloc_sbuf("fun_v");
  char *sbufc = sbuf;
  safe_sb_chr('%', sbuf, &sbufc);
  safe_sb_str(fargs[0], sbuf, &sbufc);
  *sbufc = '\0';
  char *str = sbuf;
  exec(context, buff, bufc, 0, player, cause, EV_FIGNORE, &str, cargs, ncargs);
  free_sbuf(sbuf);
}

/**
 * Returns first item in contents list of object/room
 */
static void fun_con(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  DbRef it;

  it = match_thing(&context->command->match, player, fargs[0]);

  if ((it != NOTHING) && (has_contents(context->world->database, it)) &&
      (is_examinable(context, player, it) ||
       (where_is(context->world->database, player) == it) || (it == cause))) {
    safe_tprintf_str(buff, bufc, "#%ld",
                     game_object_contents(context->world->database, it));
    return;
  }
  safe_str("#-1", buff, bufc);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_exit: Returns first exit in exits list of room.
 */
static void fun_exit(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  DbRef it, exit;
  int key;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (is_good_obj(context->world->database, it) &&
      has_exits(context->world->database, it) &&
      is_good_obj(context->world->database,
                  game_object_exits(context->world->database, it))) {
    key = 0;
    if (is_examinable(context, player, it))
      key |= VE_LOC_XAM;
    if (is_dark(context->world->database, it))
      key |= VE_LOC_DARK;
    DOLIST(context->world->database, exit,
           game_object_exits(context->world->database, it)) {
      if (exit_visible(context, exit, player, key)) {
        safe_tprintf_str(buff, bufc, "#%ld", exit);
        return;
      }
    }
  }
  safe_str("#-1", buff, bufc);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_next: return next thing in contents or exits chain
 */

static void fun_next(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  DbRef it, loc, exit, ex_here;
  int key;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (is_good_obj(context->world->database, it) &&
      has_siblings(context->world->database, it)) {
    loc = where_is(context->world->database, it);
    ex_here = is_good_obj(context->world->database, loc)
                  ? is_examinable(context, player, loc)
                  : 0;
    if (ex_here || (loc == player) ||
        (loc == where_is(context->world->database, player))) {
      if (!is_exit(context->world->database, it)) {
        safe_tprintf_str(buff, bufc, "#%ld",
                         game_object_next(context->world->database, it));
        return;
      } else {
        key = 0;
        if (ex_here)
          key |= VE_LOC_XAM;
        if (is_dark(context->world->database, loc))
          key |= VE_LOC_DARK;
        DOLIST(context->world->database, exit, it) {
          if ((exit != it) && exit_visible(context, exit, player, key)) {
            safe_tprintf_str(buff, bufc, "#%ld", exit);
            return;
          }
        }
      }
    }
  }
  safe_str("#-1", buff, bufc);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_loc: Returns the location of something
 */

static void fun_loc(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  DbRef it;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (locatable(context, context->world->configuration, player, it, cause))
    safe_tprintf_str(buff, bufc, "#%ld",
                     game_object_location(context->world->database, it));
  else
    safe_str("#-1", buff, bufc);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_where: Returns the "true" location of something
 */

static void fun_where(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  DbRef it;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (locatable(context, context->world->configuration, player, it, cause))
    safe_tprintf_str(buff, bufc, "#%ld",
                     where_is(context->world->database, it));
  else
    safe_str("#-1", buff, bufc);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_rloc: Returns the recursed location of something (specifying #levels)
 */

static void fun_rloc(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  int i, levels;
  DbRef it;

  levels = atoi(fargs[1]);
  if (levels > context->world->configuration->ntfy_nest_lim)
    levels = context->world->configuration->ntfy_nest_lim;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (locatable(context, context->world->configuration, player, it, cause)) {
    for (i = 0; i < levels; i++) {
      if (!is_good_obj(context->world->database, it) ||
          !has_location(context->world->database, it))
        break;
      it = game_object_location(context->world->database, it);
    }
    safe_tprintf_str(buff, bufc, "#%ld", it);
    return;
  }
  safe_str("#-1", buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_room: Find the room an object is ultimately in.
 */

static void fun_room(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  DbRef it;
  int count;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (locatable(context, context->world->configuration, player, it, cause)) {
    for (count = context->world->configuration->ntfy_nest_lim; count > 0;
         count--) {
      it = game_object_location(context->world->database, it);
      if (!is_good_obj(context->world->database, it))
        break;
      if (is_room(context->world->database, it)) {
        safe_tprintf_str(buff, bufc, "#%ld", it);
        return;
      }
    }
    safe_str("#-1", buff, bufc);
  } else if (is_room(context->world->database, it)) {
    safe_tprintf_str(buff, bufc, "#%ld", it);
  } else {
    safe_str("#-1", buff, bufc);
  }
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_owner: Return the owner of an object.
 */

static void fun_owner(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);
  if (it != NOTHING)
    it = game_object_owner(context->world->database, it);
  safe_tprintf_str(buff, bufc, "#%ld", it);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_controls: Does x control y?
 */

static void fun_controls(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  DbRef x, y;

  x = match_thing(&context->command->match, player, fargs[0]);
  if (x == NOTHING) {
    safe_tprintf_str(buff, bufc, "%s", "#-1 ARG1 NOT FOUND");
    return;
  }
  y = match_thing(&context->command->match, player, fargs[1]);
  if (y == NOTHING) {
    safe_tprintf_str(buff, bufc, "%s", "#-1 ARG2 NOT FOUND");
    return;
  }
  safe_tprintf_str(buff, bufc, "%d", is_controls(context, x, y));
}

/*
 * ---------------------------------------------------------------------------
 * * fun_fullname: Return the fullname of an object (good for exits)
 */

static void fun_fullname(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  DbRef it;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (it == NOTHING) {
    return;
  }
  if (!nearby_or_control(context, player, it) &&
      !is_player(context->world->database, it)) {
    safe_str("#-1 TOO FAR AWAY TO SEE", buff, bufc);
    return;
  }
  safe_str(game_object_name(context->world->database, it), buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_name: Return the name of an object
 */

static void fun_name(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  DbRef it;
  char *s, *temp;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (it == NOTHING) {
    return;
  }
  if (!nearby_or_control(context, player, it) &&
      !is_player(context->world->database, it) &&
      !is_long_fingers(context->world->database, player)) {
    safe_str("#-1 TOO FAR AWAY TO SEE", buff, bufc);
    return;
  }
  temp = *bufc;
  safe_str(game_object_name(context->world->database, it), buff, bufc);
  if (is_exit(context->world->database, it)) {
    for (s = temp; (s != *bufc) && (*s != ';'); s++)
      ;
    if (*s == ';')
      *bufc = s;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fun_match, fun_strmatch: Match arg2 against each word of arg1 returning
 * * index of first match, or against the whole string.
 */

static void fun_match(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  int wcount;
  char *r, *s, sep;

  varargs_preamble("MATCH", 3);

  /*
   * Check each word individually, returning the word number of the * *
   *
   * *  * * first one that matches.  If none match, return 0.
   */

  wcount = 1;
  s = trim_space_sep(fargs[0], sep);
  do {
    r = split_token(&s, sep);
    if (quick_wild(fargs[1], r)) {
      safe_tprintf_str(buff, bufc, "%d", wcount);
      return;
    }
    wcount++;
  } while (s);
  safe_str("0", buff, bufc);
}

static void fun_strmatch(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  /*
   * Check if we match the whole string.  If so, return 1
   */

  if (quick_wild(fargs[1], fargs[0]))
    safe_str("1", buff, bufc);
  else
    safe_str("0", buff, bufc);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_extract: extract words from string:
 * * extract(foo bar baz,1,2) returns 'foo bar'
 * * extract(foo bar baz,2,1) returns 'bar'
 * * extract(foo bar baz,2,2) returns 'bar baz'
 * *
 * * Now takes optional separator extract(foo-bar-baz,1,2,-) returns 'foo-bar'
 */

static void fun_extract(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  int start, len;
  char *r, *s, sep;

  varargs_preamble("EXTRACT", 4);

  s = fargs[0];
  start = atoi(fargs[1]);
  len = atoi(fargs[2]);

  if ((start < 1) || (len < 1)) {
    return;
  }
  /*
   * Skip to the start of the string to save
   */

  start--;
  s = trim_space_sep(s, sep);
  while (start && s) {
    s = next_token(s, sep);
    start--;
  }

  /*
   * If we ran of the end of the string, return nothing
   */

  if (!s || !*s) {
    return;
  }
  /*
   * Count off the words in the string to save
   */

  r = s;
  len--;
  while (len && s) {
    s = next_token(s, sep);
    len--;
  }

  /*
   * Chop off the rest of the string, if needed
   */

  if (s && *s)
    split_token(&s, sep);
  safe_str(r, buff, bufc);
}

int xlate(char *arg) {
  int temp;
  char *temp2;

  if (arg[0] == '#') {
    arg++;
    /* #- anything is false */
    if (arg[1] == '-') {
      return 0;
    }
    if (is_integer(arg)) {
      temp = atoi(arg);
      if (temp == -1)
        temp = 0;
      return temp;
    }
    return 0;
  }
  temp2 = trim_space_sep(arg, ' ');
  if (!*temp2)
    return 0;
  if (is_integer(temp2))
    return atoi(temp2);
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_index:  like extract(), but it works with an arbitrary separator.
 * * index(a b | c d e | f gh | ij k, |, 2, 1) => c d e
 * * index(a b | c d e | f gh | ij k, |, 2, 2) => c d e | f g h
 */

static void fun_index(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  int start, end;
  char c, *s, *p;

  s = fargs[0];
  c = *fargs[1];
  start = atoi(fargs[2]);
  end = atoi(fargs[3]);

  if ((start < 1) || (end < 1) || (*s == '\0'))
    return;
  if (c == '\0')
    c = ' ';

  /*
   * move s to point to the start of the item we want
   */

  start--;
  while (start && s && *s) {
    if ((s = (char *)index(s, c)) != nullptr)
      s++;
    start--;
  }

  /*
   * skip over just spaces
   */

  while (s && (*s == ' '))
    s++;
  if (!s || !*s)
    return;

  /*
   * figure out where to end the string
   */

  p = s;
  while (end && p && *p) {
    if ((p = (char *)index(p, c)) != nullptr) {
      if (--end == 0) {
        do {
          p--;
        } while ((*p == ' ') && (p > s));
        *(++p) = '\0';
        safe_str(s, buff, bufc);
        return;
      } else {
        p++;
      }
    }
  }

  /*
   * if we've gotten this far, we've run off the end of the string
   */

  safe_str(s, buff, bufc);
}

static void fun_cat(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int i;

  safe_str(fargs[0], buff, bufc);
  for (i = 1; i < nfargs; i++) {
    safe_chr(' ', buff, bufc);
    safe_str(fargs[i], buff, bufc);
  }
}

static void fun_version(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  safe_str(context->runtime->version, buff, bufc);
}

/*
 * Doesn't actually do anything. Use this function for documenting things,
 * similar to the @@ command.
 */
static void fun_double_at(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  // Don't do anything at all
}

static void fun_strlen(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  char new[LBUF_SIZE];
  strncpy(new, fargs[0], LBUF_SIZE - 1);
  safe_tprintf_str(
      buff, bufc, "%d",
      (int)strlen((char *)strip_ansi_r(new, fargs[0], strlen(fargs[0]))));
}

static void fun_num(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "#%ld",
                   match_thing(&context->command->match, player, fargs[0]));
}

static void fun_pmatch(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  DbRef thing;

  if (*fargs[0] == '#') {
    safe_tprintf_str(buff, bufc, "#%ld",
                     match_thing(&context->command->match, player, fargs[0]));
    return;
  }
  if (!((thing = lookup_player(context->world, player, fargs[0], 1)) ==
        NOTHING)) {
    safe_tprintf_str(buff, bufc, "#%ld", thing);
    return;
  } else
    safe_str("#-1 NO MATCH", buff, bufc);
}

static void fun_gt(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%d", (atof(fargs[0]) > atof(fargs[1])));
}

static void fun_gte(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%d", (atof(fargs[0]) >= atof(fargs[1])));
}

static void fun_lt(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%d", (atof(fargs[0]) < atof(fargs[1])));
}

static void fun_lte(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%d", (atof(fargs[0]) <= atof(fargs[1])));
}

static void fun_eq(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  double a = atof(fargs[0]);
  double b = atof(fargs[1]);
  safe_tprintf_str(buff, bufc, "%d", !(a < b) && !(a > b));
}

static void fun_neq(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  double a = atof(fargs[0]);
  double b = atof(fargs[1]);
  safe_tprintf_str(buff, bufc, "%d", (a < b) || (a > b));
}

static void fun_and(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int i, val;

  if (nfargs < 2) {
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
    return;
  }

  val = atoi(fargs[0]);
  for (i = 1; val && i < nfargs; i++)
    val = val && atoi(fargs[i]);

  safe_tprintf_str(buff, bufc, "%d", val);
}

static void fun_or(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  int i, val;

  if (nfargs < 2) {
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
    return;
  }

  val = atoi(fargs[0]);
  for (i = 1; !val && i < nfargs; i++)
    val = val || atoi(fargs[i]);

  safe_tprintf_str(buff, bufc, "%d", val);
}

static void fun_xor(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int i, val;

  if (nfargs < 2) {
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
    return;
  }

  val = atoi(fargs[0]);
  for (i = 1; i < nfargs; i++) {
    int tval = atoi(fargs[i]);
    val = (val && !tval) || (!val && tval);
  }
  safe_tprintf_str(buff, bufc, "%d", val);
}

static void fun_not(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%d", !xlate(fargs[0]));
}

static void fun_t(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%d", !(!xlate(fargs[0])));
}

static void fun_sqrt(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  double val;

  val = atof(fargs[0]);
  if (val < 0) {
    safe_str("#-1 SQUARE ROOT OF NEGATIVE", buff, bufc);
  } else if (val > 0) {
    fval(buff, bufc, sqrt(val));
  } else {
    safe_str("0", buff, bufc);
  }
}

static void fun_add(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  double sum = 0;
  int i;

  if (!nfargs) {
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
    return;
  }
  for (i = 0; i < nfargs; i++)
    sum += atof(fargs[i]);

  fval(buff, bufc, sum);
}

static void fun_sub(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  fval(buff, bufc, atof(fargs[0]) - atof(fargs[1]));
}

static void fun_mul(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int i;
  double prod;

  if (!nfargs) {
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
    return;
  }

  prod = atof(fargs[0]);
  for (i = 1; i < nfargs; i++)
    prod *= atof(fargs[i]);

  fval(buff, bufc, prod);
}

static void fun_floor(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%.0f", floor(atof(fargs[0])));
}

static void fun_ceil(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%.0f", ceil(atof(fargs[0])));
}

static void fun_round(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  const char *fstr;
  char *oldp;

  oldp = *bufc;

  switch (atoi(fargs[1])) {
  case 1:
    fstr = "%.1f";
    break;
  case 2:
    fstr = "%.2f";
    break;
  case 3:
    fstr = "%.3f";
    break;
  case 4:
    fstr = "%.4f";
    break;
  case 5:
    fstr = "%.5f";
    break;
  case 6:
    fstr = "%.6f";
    break;
  default:
    fstr = "%.0f";
    break;
  }
  /* fstr is always one of the "%.Nf" literals assigned above. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wformat-nonliteral"
  safe_tprintf_str(buff, bufc, fstr, atof(fargs[0]));
#pragma clang diagnostic pop

  /* Handle bogus result of "-0" from snprintf.  Yay, cclib. */

  if (!strcmp(oldp, "-0")) {
    *oldp = '0';
    *bufc = oldp + 1;
  }
}

static void fun_trunc(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%.0f", atof(fargs[0]));
}

static void fun_div(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int bot;

  bot = atoi(fargs[1]);
  if (bot == 0) {
    safe_str("#-1 DIVIDE BY ZERO", buff, bufc);
  } else {
    safe_tprintf_str(buff, bufc, "%d", (atoi(fargs[0]) / bot));
  }
}

static void fun_fdiv(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  double bot;

  bot = atof(fargs[1]);
  if (!(bot < 0) && !(bot > 0)) {
    safe_str("#-1 DIVIDE BY ZERO", buff, bufc);
  } else {
    fval(buff, bufc, (atof(fargs[0]) / bot));
  }
}

static void fun_mod(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int bot;

  bot = atoi(fargs[1]);
  if (bot == 0)
    bot = 1;
  safe_tprintf_str(buff, bufc, "%d", atoi(fargs[0]) % bot);
}

static void fun_pi(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  safe_str("3.141592654", buff, bufc);
}

static void fun_e(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  safe_str("2.718281828", buff, bufc);
}

static void fun_sin(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  fval(buff, bufc, sin(atof(fargs[0])));
}

static void fun_cos(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  fval(buff, bufc, cos(atof(fargs[0])));
}

static void fun_tan(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  fval(buff, bufc, tan(atof(fargs[0])));
}

static void fun_exp(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  fval(buff, bufc, exp(atof(fargs[0])));
}

static void fun_power(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  double val1, val2;

  val1 = atof(fargs[0]);
  val2 = atof(fargs[1]);
  if (val1 < 0) {
    safe_str("#-1 POWER OF NEGATIVE", buff, bufc);
  } else {
    fval(buff, bufc, pow(val1, val2));
  }
}

static void fun_ln(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  double val;

  val = atof(fargs[0]);
  if (val > 0)
    fval(buff, bufc, log(val));
  else
    safe_str("#-1 LN OF NEGATIVE OR ZERO", buff, bufc);
}

static void fun_log(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  double val;

  val = atof(fargs[0]);
  if (val > 0) {
    fval(buff, bufc, log10(val));
  } else {
    safe_str("#-1 LOG OF NEGATIVE OR ZERO", buff, bufc);
  }
}

static void fun_asin(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  double val;

  val = atof(fargs[0]);
  if ((val < -1) || (val > 1)) {
    safe_str("#-1 ASIN ARGUMENT OUT OF RANGE", buff, bufc);
  } else {
    fval(buff, bufc, asin(val));
  }
}

static void fun_acos(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  double val;

  val = atof(fargs[0]);
  if ((val < -1) || (val > 1)) {
    safe_str("#-1 ACOS ARGUMENT OUT OF RANGE", buff, bufc);
  } else {
    fval(buff, bufc, acos(val));
  }
}

static void fun_atan(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  fval(buff, bufc, atan(atof(fargs[0])));
}

static void fun_cart2hex(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  double x, y;
  int x_count, y_count;
  int hex_x, hex_y;
  double cart_x, cart_y;
  double root3 = sqrt(3) * 322.5;
  double alpha = root3 / 6;
  double angle_alpha = sqrt(3) / 6;

  cart_x = atof(fargs[0]);
  cart_y = atof(fargs[1]);

  if (cart_x < alpha) {
    /* Special case: we are in section IV of x-column 0 or off the map */
    double hex_y_d = floor(cart_y / 322.5);
    hex_x = cart_x < 0 ? -1 : 0;
    hex_y = (int)hex_y_d;
    safe_tprintf_str(buff, bufc, "%d %d", hex_x, hex_y);
    return;
  }

  /* 'shift' the map to the left so the repeatable box starts at 0 */
  cart_x -= alpha;

  /* Figure out the x-coordinate of the 'repeatable box' we're in. */
  x_count = (int)(cart_x / root3);
  /* And the offset inside the box, from the left edge. */
  x = cart_x - x_count * root3;

  /* The repbox holds two x-columns, we want the real X coordinate. */
  x_count *= 2;

  /* Do the same for the y-coordinate; this is easy */
  double y_count_d = floor(cart_y / 322.5);
  y_count = (int)y_count_d;
  y = cart_y - y_count * 322.5;

  if (x < 2 * alpha) {

    /* Clean in area I. Nothing to do */

  } else if (x >= 3 * alpha && x < 5 * alpha) {
    /* Clean in either area II or III. Up x one, and y if in the lower
       half of the box. */
    x_count++;
    if (y >= 161.25)
      /* Area II */
      y_count++;

  } else if (x >= 2 * alpha && x < 3 * alpha) {
    /* Any of areas I, II and III. */
    if (y >= 161.25) {
      /* Area I or II */
      if (2 * angle_alpha * (322.5 - y) <= x - 2 * alpha) {
        /* Area II, up both */
        x_count++;
        y_count++;
      }
    } else {
      /* Area I or III */
      if (2 * angle_alpha * y <= x - 2 * alpha)
        /* Area III, up only x */
        x_count++;
    }
  } else if (y >= 161.25) {
    /* Area II or IV. Up x at least one, maybe two, and y maybe one. */
    x_count++;
    if (2 * angle_alpha * (y - 161.25) > (x - 5.0 * alpha))
      /* Area II */
      y_count++;
    else
      /* Area IV */
      x_count++;
  } else {
    /* Area III or IV, up x at least one, maybe two */
    x_count++;
    if (2 * angle_alpha * y > root3 - x)
      /* Area IV */
      x_count++;
  }

  hex_x = x_count;
  hex_y = y_count;

  safe_tprintf_str(buff, bufc, "%d %d", hex_x, hex_y);
}

static void fun_hex2cart(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  double cart_x, cart_y;

  cart_x = (2.0 + 3.0 * atof(fargs[0])) * ((sqrt(3) * 322.5) / 6);
  cart_y = ((atoi(fargs[0]) % 2) ? 0 : 161.25) + (atof(fargs[1]) * 322.5);

  safe_tprintf_str(buff, bufc, "%f %f", cart_x, cart_y);
}
static void fun_circumcenter(char *buff, char **bufc, DbRef player, DbRef cause,
                             char *fargs[], int nfargs, char *cargs[],
                             int ncargs, EvaluationContext *context) {
  // Find the CircumCenter of 3 points. Borrowed from Delphi code.

  double x1, x2, x3, y1, y2, y3;
  double formA, formB, formC, formD, formE, formF, formG;
  double xcenter, ycenter;

  x1 = atof(fargs[0]);
  x2 = atof(fargs[2]);
  x3 = atof(fargs[4]);
  y1 = atof(fargs[1]);
  y2 = atof(fargs[3]);
  y3 = atof(fargs[5]);

  formA = x2 - x1;
  formB = y2 - y1;
  formC = x3 - x1;
  formD = y3 - y1;
  formE = formA * (x1 + x2) + formB * (y1 + y2);
  formF = formC * (x1 + x3) + formD * (y1 + y3);
  formG = 2.0 * (formA * (y3 - y2) - formB * (x3 - x2));

  if (formG == 0.0) {
    safe_str("#-1", buff, bufc);
  } else {
    xcenter = (formD * formE - formB * formF) / formG;
    ycenter = (formA * formF - formC * formE) / formG;
    safe_tprintf_str(buff, bufc, "%d %d", (int)xcenter, (int)ycenter);
  }
}

static void fun_dist2d(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  int d;
  double r;

  d = atoi(fargs[0]) - atoi(fargs[2]);
  r = (double)(d * d);
  d = atoi(fargs[1]) - atoi(fargs[3]);
  r += (double)(d * d);
  d = (int)(sqrt(r) + 0.5);
  safe_tprintf_str(buff, bufc, "%d", d);
}

static void fun_dist3d(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  int d;
  double r;

  d = atoi(fargs[0]) - atoi(fargs[3]);
  r = (double)(d * d);
  d = atoi(fargs[1]) - atoi(fargs[4]);
  r += (double)(d * d);
  d = atoi(fargs[2]) - atoi(fargs[5]);
  r += (double)(d * d);
  d = (int)(sqrt(r) + 0.5);
  safe_tprintf_str(buff, bufc, "%d", d);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_comp: string compare.
 */

static void fun_comp(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  int x;

  x = strcmp(fargs[0], fargs[1]);
  if (x > 0)
    safe_str("1", buff, bufc);
  else if (x < 0)
    safe_str("-1", buff, bufc);
  else
    safe_str("0", buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_lcon: Return a list of contents.
 */

static void fun_lcon(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  DbRef thing, it;
  char *tbuf;
  int first = 1;

  it = match_thing(&context->command->match, player, fargs[0]);
  if ((it != NOTHING) && (has_contents(context->world->database, it)) &&
      (is_examinable(context, player, it) ||
       (game_object_location(context->world->database, player) == it) ||
       (it == cause))) {
    tbuf = alloc_sbuf("fun_lcon");
    DOLIST(context->world->database, thing,
           game_object_contents(context->world->database, it)) {
      if (!first)
        snprintf(tbuf, SBUF_SIZE, " #%ld", thing);
      else {
        snprintf(tbuf, SBUF_SIZE, "#%ld", thing);
        first = 0;
      }
      safe_str(tbuf, buff, bufc);
    }
    free_sbuf(tbuf);
  } else
    safe_str("#-1", buff, bufc);
}

/* fun_lplayers: Return a list of players in an object  (Connected or not) */

static void fun_lplayers(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  DbRef thing, it;
  char *tbuf;
  int first = 1;

  it = match_thing(&context->command->match, player, fargs[0]);
  if ((it != NOTHING) && (has_contents(context->world->database, it)) &&
      (is_examinable(context, player, it) ||
       (game_object_location(context->world->database, player) == it) ||
       (it == cause))) {

    tbuf = alloc_sbuf("fun_lplayers");
    DOLIST(context->world->database, thing,
           game_object_contents(context->world->database, it)) {
      if (typeof_obj(context->world->database, thing) == TYPE_PLAYER &&
          !is_dark(context->world->database, thing)) {
        if (!first)
          snprintf(tbuf, SBUF_SIZE, " #%ld", thing);
        else {
          snprintf(tbuf, SBUF_SIZE, "#%ld", thing);
          first = 0;
        }
        safe_str(tbuf, buff, bufc);
      }
    }
    free_sbuf(tbuf);
  } else
    safe_str("#-1", buff, bufc);
}

/* fun_lvplayers: Return of list of connected players in an object */

static void fun_lvplayers(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  DbRef thing, it;
  char *tbuf;
  int first = 1;

  it = match_thing(&context->command->match, player, fargs[0]);
  if ((it != NOTHING) && (has_contents(context->world->database, it)) &&
      (is_examinable(context, player, it) ||
       (game_object_location(context->world->database, player) == it) ||
       (it == cause))) {

    tbuf = alloc_sbuf("fun_lvplayers");
    DOLIST(context->world->database, thing,
           game_object_contents(context->world->database, it)) {
      if (typeof_obj(context->world->database, thing) == TYPE_PLAYER &&
          !is_dark(context->world->database, thing) &&
          is_connected(context->world->database, thing)) {
        if (!first)
          snprintf(tbuf, SBUF_SIZE, " #%ld", thing);
        else {
          snprintf(tbuf, SBUF_SIZE, "#%ld", thing);
          first = 0;
        }
        safe_str(tbuf, buff, bufc);
      }
    }
    free_sbuf(tbuf);
  } else
    safe_str("#-1", buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_lexits: Return a list of exits.
 */

static void fun_lexits(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  DbRef thing, it;
  char *tbuf;
  int exam, key;
  int first = 1;

  it = match_thing(&context->command->match, player, fargs[0]);

  if (!is_good_obj(context->world->database, it) ||
      !has_exits(context->world->database, it)) {
    safe_str("#-1", buff, bufc);
    return;
  }
  exam = is_examinable(context, player, it);
  if (!exam && (where_is(context->world->database, player) != it) &&
      (it != cause)) {
    safe_str("#-1", buff, bufc);
    return;
  }
  tbuf = alloc_sbuf("fun_lexits");

  key = 0;
  if (exam)
    key |= VE_LOC_XAM;
  if (is_dark(context->world->database, it))
    key |= VE_LOC_DARK;
  DOLIST(context->world->database, thing,
         game_object_exits(context->world->database, it)) {
    if (exit_visible(context, thing, player, key)) {
      if (!first)
        snprintf(tbuf, SBUF_SIZE, " #%ld", thing);
      else {
        snprintf(tbuf, SBUF_SIZE, "#%ld", thing);
        first = 0;
      }
      safe_str(tbuf, buff, bufc);
    }
  }
  free_sbuf(tbuf);
  return;
}

/*
 * --------------------------------------------------------------------------
 * * fun_home: Return an object's home
 */

static void fun_home(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  DbRef it;

  it = match_thing(&context->command->match, player, fargs[0]);
  // "#-1" is both the invalid-object guard and the final default; the
  // middle branches are the real logic.
  if (!is_good_obj(context->world->database, it) ||
      !is_examinable(context, player, it))
    safe_str("#-1", buff, bufc); // NOLINT(bugprone-branch-clone)
  else if (has_home(context->world->database, it))
    safe_tprintf_str(buff, bufc, "#%ld",
                     game_object_link(context->world->database, it));
  else if (has_dropto(context->world->database, it))
    safe_tprintf_str(buff, bufc, "#%ld",
                     game_object_location(context->world->database, it));
  else if (is_exit(context->world->database, it))
    safe_tprintf_str(buff, bufc, "#%ld",
                     where_is(context->world->database, it));
  else
    safe_str("#-1", buff, bufc);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_pos: Find a word in a string
 */

static void fun_pos(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int i = 1;
  char *s, *t, *u;

  i = 1;
  s = fargs[1];
  while (*s) {
    u = s;
    t = fargs[0];
    while (*t && *t == *u)
      ++t, ++u;
    if (*t == '\0') {
      safe_tprintf_str(buff, bufc, "%d", i);
      return;
    }
    ++i, ++s;
  }
  safe_str("#-1", buff, bufc);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * ldelete: Remove a word from a string by place
 * *  ldelete(<list>,<position>[,<separator>])
 * *
 * * insert: insert a word into a string by place
 * *  insert(<list>,<position>,<new item> [,<separator>])
 * *
 * * replace: replace a word into a string by place
 * *  replace(<list>,<position>,<new item>[,<separator>])
 */

constexpr int IF_DELETE = 0;
constexpr int IF_REPLACE = 1;
constexpr int IF_INSERT = 2;

static void do_itemfuns(char *buff, char **bufc, char *str, int el, char *word,
                        char sep, int flag) {
  int ct, overrun;
  char *sptr, *iptr, *eptr;

  /*
   * If passed a null string return an empty string, except that we * *
   *
   * *  * * are allowed to append to a null string.
   */

  if ((!str || !*str) && ((flag != IF_INSERT) || (el != 1))) {
    return;
  }
  /*
   * we can't fiddle with anything before the first position
   */

  if (el < 1) {
    safe_str(str, buff, bufc);
    return;
  }
  /*
   * Split the list up into 'before', 'target', and 'after' chunks * *
   * * * pointed to by sptr, iptr, and eptr respectively.
   */

  if (el == 1) {
    /*
     * No 'before' portion, just split off element 1
     */

    sptr = nullptr;
    if (!str || !*str) {
      eptr = nullptr;
      iptr = nullptr;
    } else {
      eptr = trim_space_sep(str, sep);
      iptr = split_token(&eptr, sep);
    }
  } else {
    /*
     * Break off 'before' portion
     */

    sptr = eptr = trim_space_sep(str, sep);
    overrun = 1;
    for (ct = el; ct > 2 && eptr; eptr = next_token(eptr, sep), ct--)
      ;
    if (eptr) {
      overrun = 0;
      iptr = split_token(&eptr, sep);
    }
    /*
     * If we didn't make it to the target element, just return *
     * * * * the string.  Insert is allowed to continue if we are
     * *  * *  * exactly at the end of the string, but replace
     * and * delete *  *  * * are not.
     */

    if (!(eptr || ((flag == IF_INSERT) && !overrun))) {
      safe_str(str, buff, bufc);
      return;
    }
    /*
     * Split the 'target' word from the 'after' portion.
     */

    if (eptr)
      iptr = split_token(&eptr, sep);
    else
      iptr = nullptr;
  }

  switch (flag) {
  case IF_DELETE: /*
                   * deletion
                   */
    if (sptr) {
      safe_str(sptr, buff, bufc);
      if (eptr)
        safe_chr(sep, buff, bufc);
    }
    if (eptr) {
      safe_str(eptr, buff, bufc);
    }
    break;
  case IF_REPLACE: /*
                    * replacing
                    */
    if (sptr) {
      safe_str(sptr, buff, bufc);
      safe_chr(sep, buff, bufc);
    }
    safe_str(word, buff, bufc);
    if (eptr) {
      safe_chr(sep, buff, bufc);
      safe_str(eptr, buff, bufc);
    }
    break;
  case IF_INSERT: /*
                   * insertion
                   */
    if (sptr) {
      safe_str(sptr, buff, bufc);
      safe_chr(sep, buff, bufc);
    }
    safe_str(word, buff, bufc);
    if (iptr) {
      safe_chr(sep, buff, bufc);
      safe_str(iptr, buff, bufc);
    }
    if (eptr) {
      safe_chr(sep, buff, bufc);
      safe_str(eptr, buff, bufc);
    }
    break;
  default:
    break;
  }
}

static void fun_ldelete(
    char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
    int nfargs, char *cargs[], int ncargs,
    EvaluationContext *context) { /*
                                   * delete a word at position X of a list
                                   */
  char sep;

  varargs_preamble("LDELETE", 3);
  do_itemfuns(buff, bufc, fargs[0], atoi(fargs[1]), nullptr, sep, IF_DELETE);
}

static void fun_replace(
    char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
    int nfargs, char *cargs[], int ncargs,
    EvaluationContext *context) { /*
                                   * replace a word at position X of a list
                                   */
  char sep;

  varargs_preamble("REPLACE", 4);
  do_itemfuns(buff, bufc, fargs[0], atoi(fargs[1]), fargs[2], sep, IF_REPLACE);
}

static void fun_insert(
    char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
    int nfargs, char *cargs[], int ncargs,
    EvaluationContext *context) { /*
                                   * insert a word at position X of a list
                                   */
  char sep;

  varargs_preamble("INSERT", 4);
  do_itemfuns(buff, bufc, fargs[0], atoi(fargs[1]), fargs[2], sep, IF_INSERT);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_remove: Remove a word from a string
 */

static void fun_remove(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  char *s, *sp, *word;
  char sep;
  int first, found;

  varargs_preamble("REMOVE", 3);
  if (index(fargs[1], sep)) {
    safe_str("#-1 CAN ONLY DELETE ONE ELEMENT", buff, bufc);
    return;
  }
  s = fargs[0];
  word = fargs[1];

  /*
   * Walk through the string copying words until (if ever) we get to *
   * * * * one that matches the target word.
   */

  sp = s;
  found = 0;
  first = 1;
  while (s) {
    sp = split_token(&s, sep);
    if (found || strcmp(sp, word)) {
      if (!first)
        safe_chr(sep, buff, bufc);
      safe_str(sp, buff, bufc);
      first = 0;
    } else {
      found = 1;
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fun_member: Is a word in a string
 */

static void fun_member(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  int wcount;
  char *r, *s, sep;

  varargs_preamble("MEMBER", 3);
  wcount = 1;
  s = trim_space_sep(fargs[0], sep);
  do {
    r = split_token(&s, sep);
    if (!strcmp(fargs[1], r)) {
      safe_tprintf_str(buff, bufc, "%d", wcount);
      return;
    }
    wcount++;
  } while (s);
  safe_str("0", buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_secure, fun_escape: escape [, ], %, \, and the beginning of the string.
 */

static void fun_secure(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  char *s;

  s = fargs[0];
  while (*s) {
    switch (*s) {
    case '%':
    case '$':
    case '\\':
    case '[':
    case ']':
    case '(':
    case ')':
    case '{':
    case '}':
    case ',':
    case ';':
      safe_chr(' ', buff, bufc);
      break;
    default:
      safe_chr(*s, buff, bufc);
    }
    s++;
  }
}

static void fun_ansi_secure(char *buff, char **bufc, DbRef player, DbRef cause,
                            char *fargs[], int nfargs, char *cargs[],
                            int ncargs, EvaluationContext *context) {
  char *s;

  s = fargs[0];
  while (*s) {
    switch (*s) {
    case '\033':
      safe_chr(*s++, buff, bufc);
      if (*s == '[') {
        safe_chr(*s, buff, bufc);
        break;
      } /* FALLTHRU */
    case '%':
    case '[':
    case '$':
    case '\\':
    case ']':
    case '(':
    case ')':
    case '{':
    case '}':
    case ',':
    case ';':
      safe_chr(' ', buff, bufc);
      break;
    default:
      safe_chr(*s, buff, bufc);
    }
    s++;
  }
}

static void fun_escape(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  char *s, *d;

  d = *bufc;
  s = fargs[0];
  while (*s) {
    switch (*s) {
    case '%':
    case '\\':
    case '[':
    case ']':
    case '{':
    case '}':
    case ';':
      safe_chr('\\', buff, bufc);
      [[fallthrough]];
    default:
      if (*bufc == d)
        safe_chr('\\', buff, bufc);
      safe_chr(*s, buff, bufc);
    }
    s++;
  }
}

/*
 * Take a character position and return which word that char is in.
 * wordpos(<string>, <charpos>)
 */
static void fun_wordpos(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  int charpos, i;
  char *cp, *tp, *xp, sep;

  varargs_preamble("WORDPOS", 3);

  charpos = atoi(fargs[1]);
  cp = fargs[0];
  if ((charpos > 0) && ((size_t)charpos <= strlen(cp))) {
    tp = &(cp[charpos - 1]);
    cp = trim_space_sep(cp, sep);
    xp = split_token(&cp, sep);
    for (i = 1; xp; i++) {
      if (tp < (xp + strlen(xp)))
        break;
      xp = split_token(&cp, sep);
    }
    safe_tprintf_str(buff, bufc, "%d", i);
    return;
  }
  safe_str("#-1", buff, bufc);
  return;
}

static void fun_type(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  DbRef it;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->world->database, it)) {
    safe_str("#-1 NOT FOUND", buff, bufc);
    return;
  }
  switch (typeof_obj(context->world->database, it)) {
  case TYPE_ROOM:
    safe_str("ROOM", buff, bufc);
    break;
  case TYPE_EXIT:
    safe_str("EXIT", buff, bufc);
    break;
  case TYPE_PLAYER:
    safe_str("PLAYER", buff, bufc);
    break;
  case TYPE_THING:
    safe_str("THING", buff, bufc);
    break;
  default:
    safe_str("#-1 ILLEGAL TYPE", buff, bufc);
  }
  return;
}

static void fun_hasflag(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  DbRef it;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->world->database, it)) {
    safe_str("#-1 NOT FOUND", buff, bufc);
    return;
  }
  if (is_examinable(context, player, it) || (it == cause)) {
    if (has_flag(context->world, player, it, fargs[1]))
      safe_str("1", buff, bufc);
    else
      safe_str("0", buff, bufc);
  } else {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
  }
}

static void fun_haspower(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  DbRef it;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->world->database, it)) {
    safe_str("#-1 NOT FOUND", buff, bufc);
    return;
  }
  if (is_examinable(context, player, it) || (it == cause)) {
    if (has_power(context->world, player, it, fargs[1]))
      safe_str("1", buff, bufc);
    else
      safe_str("0", buff, bufc);
  } else {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
  }
}

static void fun_delete(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  char *s, *temp, *bp;
  int i, start, nchars, len;

  s = fargs[0];
  start = atoi(fargs[1]);
  nchars = atoi(fargs[2]);
  len = (int)strlen(s);
  if ((start >= len) || (nchars <= 0)) {
    safe_str(s, buff, bufc);
    return;
  }
  bp = temp = alloc_lbuf("fun_delete");
  for (i = 0; i < start; i++)
    *bp++ = (*s++);
  if ((i + nchars) < len && (i + nchars) > 0) {
    s += nchars;
    while ((*bp++ = *s++))
      ;
  } else
    *bp = '\0';

  safe_str(temp, buff, bufc);
  free_lbuf(temp);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_lwho: Return list of connected users.
 */

static void fun_lwho(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  make_ulist(context->world->database, context->runtime->descriptors, player,
             buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_nearby: Return whether or not obj1 is near obj2.
 */

static void fun_nearby(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  DbRef obj1, obj2;

  obj1 = match_thing(&context->command->match, player, fargs[0]);
  obj2 = match_thing(&context->command->match, player, fargs[1]);
  if ((nearby_or_control(context, player, obj1) ||
       nearby_or_control(context, player, obj2)) &&
      nearby(context->world->database, obj1, obj2))
    safe_str("1", buff, bufc);
  else
    safe_str("0", buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_obj and fun_poss: perform pronoun substitution for object.
 */

static void process_pronoun(EvaluationContext *context, DbRef player,
                            char *what, const char *token, char *buff,
                            char **bufc) {
  DbRef it;
  char *str;

  it = match_thing(&context->command->match, player, what);
  if (!is_good_obj(context->world->database, it) ||
      (!is_player(context->world->database, it) &&
       !nearby_or_control(context, player, it))) {
    safe_str("#-1 NO MATCH", buff, bufc);
  } else {
    /* exec()'s cursor parameter is char ** for its own reasons; token is
       never written through here, only scanned. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    str = (char *)token;
#pragma clang diagnostic pop
    exec(context, buff, bufc, 0, it, it, 0, &str, (char **)nullptr, 0);
  }
}

static void fun_obj(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  process_pronoun(context, player, fargs[0], "%o", buff, bufc);
}

static void fun_poss(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  process_pronoun(context, player, fargs[0], "%p", buff, bufc);
}

static void fun_aposs(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  process_pronoun(context, player, fargs[0], "%a", buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_mudname: Return the name of the mud.
 */

static void fun_mudname(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  safe_str(context->world->configuration->mud_name, buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_lcstr, fun_ucstr, fun_capstr: Lowercase, uppercase, or capitalize str.
 */

static void fun_lcstr(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  char *ap;

  ap = fargs[0];
  while (*ap) {
    **bufc = ToLower(*ap);
    ap++;
    (*bufc)++;
  }
}

static void fun_ucstr(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  char *ap;

  ap = fargs[0];
  while (*ap) {
    **bufc = ToUpper(*ap);
    ap++;
    (*bufc)++;
  }
}

static void fun_capstr(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  char *s;

  s = *bufc;

  safe_str(fargs[0], buff, bufc);
  *s = ToUpper(*s);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_lnum: Return a list of numbers.
 */

static void fun_lnum(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  char tbuff[16];
  int ctr, limit, llimit = 0, over;

  if (nfargs > 2 || nfargs < 1) {
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
    return;
  }
  over = 0;
  limit = atoi(fargs[0]);
  if (nfargs == 2) {
    llimit = atoi(fargs[0]);
    limit = atoi(fargs[1]) + 1;
  } else
    limit = atoi(fargs[0]);
  if (limit > 0 && llimit >= 0 && llimit < limit) {
    for (ctr = llimit; ctr < limit && !over; ctr++) {
      {
        snprintf(tbuff, sizeof(tbuff), "%s%d", ctr != llimit ? " " : "", ctr);
        over = safe_str(tbuff, buff, bufc);
      }
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fun_lattr: Return list of attributes I can see on the object.
 */

/*
 * ---------------------------------------------------------------------------
 * * do_reverse, fun_reverse, fun_revwords: Reverse things.
 */

static void do_reverse(char *from, char *to) {
  char *tp;

  tp = to + strlen(from);
  *tp-- = '\0';
  while (*from) {
    *tp-- = *from++;
  }
}

static void fun_reverse(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  do_reverse(fargs[0], *bufc);
  *bufc += strlen(fargs[0]);
}

static void fun_revwords(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  char *temp, *tp, *t1, sep;
  int first;

  /*
   * If we are passed an empty arglist return a null string
   */

  if (nfargs == 0) {
    return;
  }
  varargs_preamble("REVWORDS", 2);
  temp = alloc_lbuf("fun_revwords");

  /*
   * Reverse the whole string
   */

  do_reverse(fargs[0], temp);

  /*
   * Now individually reverse each word in the string.  This will
   * undo the reversing of the words (so the words themselves are
   * forwards again.
   */

  tp = temp;
  first = 1;
  while (tp) {
    if (!first)
      safe_chr(sep, buff, bufc);
    t1 = split_token(&tp, sep);
    do_reverse(t1, *bufc);
    *bufc += strlen(t1);
    first = 0;
  }
  free_lbuf(temp);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_after, fun_before: Return substring after or before a specified string.
 */

static void fun_after(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  char *bp, *cp, *mp;
  size_t mlen;

  if (nfargs == 0) {
    return;
  }
  if (!fn_range_check("AFTER", nfargs, 1, 2, buff, bufc))
    return;
  bp = fargs[0];
  mp = fargs[1];

  /*
   * Sanity-check arg1 and arg2
   */

  /* bp/mp alias fargs[]; they're mutable char * so these literal
     defaults need an explicit cast. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  if (bp == nullptr)
    bp = (char *)"";
  if (mp == nullptr)
    mp = (char *)" ";
  if (!mp || !*mp)
    mp = (char *)" ";
#pragma clang diagnostic pop
  mlen = strlen(mp);
  if ((mlen == 1) && (*mp == ' '))
    bp = trim_space_sep(bp, ' ');

  /*
   * Look for the target string
   */

  while (*bp) {

    /*
     * Search for the first character in the target string
     */

    cp = (char *)index(bp, *mp);
    if (cp == nullptr) {

      /*
       * Not found, return empty string
       */

      return;
    }
    /*
     * See if what follows is what we are looking for
     */

    if (!strncmp(cp, mp, mlen)) {

      /*
       * Yup, return what follows
       */

      bp = cp + mlen;
      safe_str(bp, buff, bufc);
      return;
    }
    /*
     * Continue search after found first character
     */

    bp = cp + 1;
  }

  /*
   * Ran off the end without finding it
   */

  return;
}

static void fun_before(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  char *bp, *cp, *mp, *ip;
  size_t mlen;

  if (nfargs == 0) {
    return;
  }
  if (!fn_range_check("BEFORE", nfargs, 1, 2, buff, bufc))
    return;

  bp = fargs[0];
  mp = fargs[1];

  /*
   * Sanity-check arg1 and arg2
   */

  /* bp/mp alias fargs[]; they're mutable char * so these literal
     defaults need an explicit cast. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  if (bp == nullptr)
    bp = (char *)"";
  if (mp == nullptr)
    mp = (char *)" ";
  if (!mp || !*mp)
    mp = (char *)" ";
#pragma clang diagnostic pop
  mlen = strlen(mp);
  if ((mlen == 1) && (*mp == ' '))
    bp = trim_space_sep(bp, ' ');
  ip = bp;

  /*
   * Look for the target string
   */

  while (*bp) {

    /*
     * Search for the first character in the target string
     */

    cp = (char *)index(bp, *mp);
    if (cp == nullptr) {

      /*
       * Not found, return entire string
       */

      safe_str(ip, buff, bufc);
      return;
    }
    /*
     * See if what follows is what we are looking for
     */

    if (!strncmp(cp, mp, mlen)) {

      /*
       * Yup, return what follows
       */

      *cp = '\0';
      safe_str(ip, buff, bufc);
      return;
    }
    /*
     * Continue search after found first character
     */

    bp = cp + 1;
  }

  /*
   * Ran off the end without finding it
   */

  safe_str(ip, buff, bufc);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_max, fun_min: Return maximum (minimum) value.
 */

static void fun_max(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int i, got_one;
  double max, j;

  max = 0;
  for (i = 0, got_one = 0; i < nfargs; i++) {
    if (fargs[i]) {
      j = atof(fargs[i]);
      if (!got_one || (j > max)) {
        got_one = 1;
        max = j;
      }
    }
  }

  if (!got_one)
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
  else
    fval(buff, bufc, max);
  return;
}

static void fun_min(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  int i, got_one;
  double min, j;

  min = 0;
  for (i = 0, got_one = 0; i < nfargs; i++) {
    if (fargs[i]) {
      j = atof(fargs[i]);
      if (!got_one || (j < min)) {
        got_one = 1;
        min = j;
      }
    }
  }

  if (!got_one) {
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
  } else {
    fval(buff, bufc, min);
  }
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_search: Search the db for things, returning a list of what matches
 */

static void fun_search(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  DbRef thing;
  char *bp, *nbuf;
  SearchCriteria searchparm;
  ObjectList results;

  /*
   * Set up for the search.  If any errors, abort.
   */

  if (!search_criteria_setup(context, player, fargs[0], &searchparm)) {
    safe_str("#-1 ERROR DURING SEARCH", buff, bufc);
    return;
  }
  /*
   * Do the search and report the results
   */

  object_list_initialize(&results);
  search_criteria_perform(context, player, cause, &searchparm, &results);
  bp = *bufc;
  nbuf = alloc_sbuf("fun_search");
  for (thing = object_list_first(&results); thing != NOTHING;
       thing = object_list_next(&results)) {
    if (bp == *bufc)
      snprintf(nbuf, SBUF_SIZE, "#%ld", thing);
    else
      snprintf(nbuf, SBUF_SIZE, " #%ld", thing);
    safe_str(nbuf, buff, bufc);
  }
  free_sbuf(nbuf);
  object_list_destroy(&results);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_stats: Get database size statistics.
 */

static void fun_stats(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  DbRef who;
  DatabaseStatistics statinfo;

  if ((!fargs[0]) || !*fargs[0] ||
      !string_compare(context->world->configuration, fargs[0], "all")) {
    who = NOTHING;
  } else {
    who = lookup_player(context->world, player, fargs[0], 1);
    if (who == NOTHING) {
      safe_str("#-1 NOT FOUND", buff, bufc);
      return;
    }
  }
  if (!database_statistics_get(context, player, who, &statinfo)) {
    safe_str("#-1 ERROR GETTING STATS", buff, bufc);
    return;
  }
  safe_tprintf_str(buff, bufc, "%d %d %d %d %d %d", statinfo.s_total,
                   statinfo.s_rooms, statinfo.s_exits, statinfo.s_things,
                   statinfo.s_players, statinfo.s_garbage);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_merge:  given two strings and a character, merge the two strings
 * *   by replacing characters in string1 that are the same as the given
 * *   character by the corresponding character in string2 (by position).
 * *   The strings must be of the same length.
 */

static void fun_merge(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  char *str, *rep;
  char c;

  /*
   * do length checks first
   */

  if (strlen(fargs[0]) != strlen(fargs[1])) {
    safe_str("#-1 STRING LENGTHS MUST BE EQUAL", buff, bufc);
    return;
  }
  if (strlen(fargs[2]) > 1) {
    safe_str("#-1 TOO MANY CHARACTERS", buff, bufc);
    return;
  }
  /*
   * find the character to look for. null character is considered * a *
   *
   * *  * * space
   */

  if (!*fargs[2])
    c = ' ';
  else
    c = *fargs[2];

  /*
   * walk strings, copy from the appropriate string
   */

  for (str = fargs[0], rep = fargs[1];
       *str && *rep && ((*bufc - buff) < LBUF_SIZE); str++, rep++, (*bufc)++) {
    if (*str == c)
      **bufc = *rep;
    else
      **bufc = *str;
  }

  /*
   * There is no need to check for overflowing the buffer since * both
   * * * * strings are LBUF_SIZE or less and the new string cannot be *
   * * any * * longer.
   */

  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_splice: similar to MERGE(), eplaces by word instead of by character.
 */

static void fun_splice(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  char *p1, *p2, *q1, *q2, sep;
  int words, i, first;

  varargs_preamble("SPLICE", 4);

  /*
   * length checks
   */

  if (countwords(fargs[2], sep) > 1) {
    safe_str("#-1 TOO MANY WORDS", buff, bufc);
    return;
  }
  words = countwords(fargs[0], sep);
  if (words != countwords(fargs[1], sep)) {
    safe_str("#-1 NUMBER OF WORDS MUST BE EQUAL", buff, bufc);
    return;
  }
  /*
   * loop through the two lists
   */

  p1 = fargs[0];
  q1 = fargs[1];
  first = 1;
  for (i = 0; i < words; i++) {
    p2 = split_token(&p1, sep);
    q2 = split_token(&q1, sep);
    if (!first)
      safe_chr(sep, buff, bufc);
    if (!strcmp(p2, fargs[2]))
      safe_str(q2, buff, bufc); /*
                                 * replace
                                 */
    else
      safe_str(p2, buff, bufc); /*
                                 * copy
                                 */
    first = 0;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fun_repeat: repeats a string
 */

static void fun_repeat(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  int times, i;

  times = atoi(fargs[1]);
  if ((times < 1) || (fargs[0] == nullptr) || (!*fargs[0])) {
    return;
  } else if (times == 1) {
    safe_str(fargs[0], buff, bufc);
  } else if (strlen(fargs[0]) * (size_t)times >= (LBUF_SIZE - 1)) {
    safe_str("#-1 STRING TOO LONG", buff, bufc);
  } else {
    for (i = 0; i < times; i++)
      safe_str(fargs[0], buff, bufc);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fun_iter: Make list from evaluating arg2 with each member of arg1.
 * * NOTE: This function expects that its arguments have not been evaluated.
 */

static void fun_iter(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  char *curr, *objstring, *buff2, *buff3, *cp, *dp, sep, *str;
  int first, number = 0;

  evarargs_preamble("ITER", 3);
  dp = cp = curr = alloc_lbuf("fun_iter");
  str = fargs[0];
  exec(context, curr, &dp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
       &str, cargs, ncargs);
  *dp = '\0';
  cp = trim_space_sep(cp, sep);
  if (!*cp) {
    free_lbuf(curr);
    return;
  }
  first = 1;
  while (cp) {
    if (!first)
      safe_chr(' ', buff, bufc);
    first = 0;
    number++;
    objstring = split_token(&cp, sep);
    buff2 = replace_string(BOUND_VAR, objstring, fargs[1]);
    buff3 = replace_string(LISTPLACE_VAR, tprintf("%d", number), buff2);
    str = buff3;
    exec(context, buff, bufc, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
         &str, cargs, ncargs);
    free_lbuf(buff2);
    free_lbuf(buff3);
  }
  free_lbuf(curr);
}

static void fun_sum(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  char *curr, *str, *cp, *dp, sep;
  double sum;

  evarargs_preamble("SUM", 2);
  dp = cp = curr = alloc_lbuf("fun_sum");
  str = fargs[0];
  exec(context, curr, &dp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
       &str, cargs, ncargs);
  *dp = '\0';
  cp = trim_space_sep(cp, sep);

  if (!*cp) {
    free_lbuf(curr);
    return;
  }

  sum = 0;
  while (cp) {
    sum += atof(split_token(&cp, sep));
  }
  fval(buff, bufc, sum);
  free_lbuf(curr);
}

static void fun_list(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  char *curr, *objstring, *buff2, *buff3, *result, *cp, *dp, *str, sep;
  int number = 0;

  evarargs_preamble("LIST", 3);
  cp = curr = dp = alloc_lbuf("fun_list");
  str = fargs[0];
  exec(context, curr, &dp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
       &str, cargs, ncargs);
  cp = trim_space_sep(cp, sep);
  if (!*cp) {
    free_lbuf(curr);
    return;
  }
  while (cp) {
    number++;
    objstring = split_token(&cp, sep);
    buff2 = replace_string(BOUND_VAR, objstring, fargs[1]);
    buff3 = replace_string(LISTPLACE_VAR, tprintf("%d", number), buff2);
    dp = result = alloc_lbuf("fun_list.2");
    str = buff3;
    exec(context, result, &dp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
         &str, cargs, ncargs);
    *dp = '\0';
    free_lbuf(buff2);
    free_lbuf(buff3);
    notify(context, cause, result);
    free_lbuf(result);
  }
  free_lbuf(curr);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_fold: iteratively eval an attrib with a list of arguments
 * *        and an optional base case.  With no base case, the first list
 * element
 * *    is passed as %0 and the second is %1.  The attrib is then evaluated
 * *    with these args, the result is then used as %0 and the next arg is
 * *    %1 and so it goes as there are elements left in the list.  The
 * *    optinal base case gives the user a nice starting point.
 * *
 * *    > &REP_NUM object=[%0][repeat(%1,%1)]
 * *    > say fold(OBJECT/REP_NUM,1 2 3 4 5,->)
 * *    You say "->122333444455555"
 * *
 * *      NOTE: To use added list separator, you must use base case!
 */

/*
 * ---------------------------------------------------------------------------
 * * fun_filter: iteratively perform a function with a list of arguments
 * *              and return the arg, if the function evaluates to TRUE using
 * the
 * *      arg.
 * *
 * *      > &IS_ODD object=mod(%0,2)
 * *      > say filter(object/is_odd,1 2 3 4 5)
 * *      You say "1 3 5"
 * *      > say filter(object/is_odd,1-2-3-4-5,-)
 * *      You say "1-3-5"
 * *
 * *  NOTE:  If you specify a separator it is used to delimit returned list
 */

/*
 * ---------------------------------------------------------------------------
 * * fun_map: iteratively evaluate an attribute with a list of arguments.
 * *
 * *  > &DIV_TWO object=fdiv(%0,2)
 * *  > say map(1 2 3 4 5,object/div_two)
 * *  You say "0.5 1 1.5 2 2.5"
 * *  > say map(object/div_two,1-2-3-4-5,-)
 * *  You say "0.5-1-1.5-2-2.5"
 * *
 */

/*
 * ---------------------------------------------------------------------------
 * * fun_edit: Edit text.
 */

static void fun_edit(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  char *tstr;
  char new[LBUF_SIZE];

  strncpy(new, fargs[0], LBUF_SIZE - 1);
  edit_string((char *)strip_ansi_r(new, fargs[0], strlen(fargs[0])), &tstr,
              fargs[1], fargs[2]);
  safe_str(tstr, buff, bufc);
  free_lbuf(tstr);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_locate: Search for things with the perspective of another obj.
 */

static void fun_locate(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  int pref_type, check_locks, verbose, multiple;
  DbRef thing, what;
  char *cp;

  pref_type = NOTYPE;
  check_locks = verbose = multiple = 0;

  /*
   * Find the thing to do the looking, make sure we control it.
   */

  if (is_wizard(context->world->database, player))
    thing = match_thing(&context->command->match, player, fargs[0]);
  else
    thing = match_controlled(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->world->database, thing)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  /*
   * Get pre- and post-conditions and modifiers
   */

  for (cp = fargs[2]; *cp; cp++) {
    switch (*cp) {
    case 'E':
      pref_type = TYPE_EXIT;
      break;
    case 'L':
      check_locks = 1;
      break;
    case 'P':
      pref_type = TYPE_PLAYER;
      break;
    case 'R':
      pref_type = TYPE_ROOM;
      break;
    case 'T':
      pref_type = TYPE_THING;
      break;
    case 'V':
      verbose = 1;
      break;
    case 'X':
      multiple = 1;
      break;
    default:
      break;
    }
  }

  /*
   * Set up for the search
   */

  if (check_locks)
    init_match_check_keys(&context->command->match, thing, fargs[1], pref_type);
  else
    init_match(&context->command->match, thing, fargs[1], pref_type);

  /*
   * Search for each requested thing
   */

  for (cp = fargs[2]; *cp; cp++) {
    switch (*cp) {
    case 'a':
      match_absolute(&context->command->match);
      break;
    case 'c':
      match_carried_exit(&context->command->match);
      break;
    case 'e':
      match_exit(&context->command->match);
      break;
    case 'h':
      match_here(&context->command->match);
      break;
    case 'i':
      match_possession(&context->command->match);
      break;
    case 'm':
      match_me(&context->command->match);
      break;
    case 'n':
      match_neighbor(&context->command->match);
      break;
    case 'p':
      match_player(&context->command->match);
      break;
    case '*':
      match_everything(&context->command->match, 0);
      break;
    default:
      break;
    }
  }

  /*
   * Get the result and return it to the caller
   */

  if (multiple)
    what = last_match_result(&context->command->match);
  else
    what = match_result(&context->command->match);

  if (verbose)
    (void)match_status(context, player, what);

  safe_tprintf_str(buff, bufc, "#%ld", what);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_switch: Return value based on pattern matching (ala @switch)
 * * NOTE: This function expects that its arguments have not been evaluated.
 */

static void fun_switch(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  int i;
  char *mbuff, *tbuff, *buf, *bp, *str;

  /*
   * If we don't have at least 2 args, return nothing
   */

  if (nfargs < 2) {
    return;
  }
  /*
   * Evaluate the target in fargs[0]
   */

  mbuff = bp = alloc_lbuf("fun_switch");
  str = fargs[0];
  exec(context, mbuff, &bp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
       &str, cargs, ncargs);
  *bp = '\0';

  /*
   * Loop through the patterns looking for a match
   */

  for (i = 1; (i < nfargs - 1) && fargs[i] && fargs[i + 1]; i += 2) {
    tbuff = bp = alloc_lbuf("fun_switch.2");
    str = fargs[i];
    exec(context, tbuff, &bp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
         &str, cargs, ncargs);
    *bp = '\0';
    if (quick_wild(tbuff, mbuff)) {
      free_lbuf(tbuff);
      buf = alloc_lbuf("fun_switch");
      StringCopy(buf, fargs[i + 1]);
      str = buf;
      exec(context, buff, bufc, 0, player, cause,
           EV_STRIP | EV_FCHECK | EV_EVAL, &str, cargs, ncargs);
      free_lbuf(buf);
      free_lbuf(mbuff);
      return;
    }
    free_lbuf(tbuff);
  }
  free_lbuf(mbuff);

  /*
   * Nope, return the default if there is one
   */

  if ((i < nfargs) && fargs[i]) {
    buf = alloc_lbuf("fun_switch");
    StringCopy(buf, fargs[i]);
    str = buf;
    exec(context, buff, bufc, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
         &str, cargs, ncargs);
    free_lbuf(buf);
  }
  return;
}

static void fun_case(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  int i;
  char *mbuff, *buf, *bp, *str;

  /*
   * If we don't have at least 2 args, return nothing
   */

  if (nfargs < 2) {
    return;
  }
  /*
   * Evaluate the target in fargs[0]
   */

  mbuff = bp = alloc_lbuf("fun_switch");
  str = fargs[0];
  exec(context, mbuff, &bp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
       &str, cargs, ncargs);
  *bp = '\0';

  /*
   * Loop through the patterns looking for a match
   */

  for (i = 1; (i < nfargs - 1) && fargs[i] && fargs[i + 1]; i += 2) {
    if (*fargs[i] == *mbuff) {
      buf = alloc_lbuf("fun_switch");
      StringCopy(buf, fargs[i + 1]);
      str = buf;
      exec(context, buff, bufc, 0, player, cause,
           EV_STRIP | EV_FCHECK | EV_EVAL, &str, cargs, ncargs);
      free_lbuf(buf);
      free_lbuf(mbuff);
      return;
    }
  }
  free_lbuf(mbuff);

  /*
   * Nope, return the default if there is one
   */

  if ((i < nfargs) && fargs[i]) {
    buf = alloc_lbuf("fun_switch");
    StringCopy(buf, fargs[i]);
    str = buf;
    exec(context, buff, bufc, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
         &str, cargs, ncargs);
    free_lbuf(buf);
  }
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_space: Make spaces.
 */

static void fun_space(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  int num;
  char *cp;

  if (!fargs[0] || !(*fargs[0])) {
    num = 1;
  } else {
    num = atoi(fargs[0]);
  }

  if (num < 1) {

    /*
     * If negative or zero spaces return a single space,  * * * *
     * -except- allow 'space(0)' to return "" for calculated * *
     * * * padding
     */

    if (!is_integer(fargs[0]) || (num != 0)) {
      num = 1;
    }
  } else if (num >= LBUF_SIZE) {
    num = LBUF_SIZE - 1;
  }
  for (cp = *bufc; num > 0; num--)
    *cp++ = ' ';
  *bufc = cp;
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * fun_idle, fun_conn: return seconds idle or connected.
 */

static void fun_idle(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  DbRef target;

  target = lookup_player(context->world, player, fargs[0], 1);
  if (is_good_obj(context->world->database, target) &&
      is_dark(context->world->database, target) &&
      !is_wizard(context->world->database, player))
    target = NOTHING;
  safe_tprintf_str(buff, bufc, "%d",
                   fetch_idle(context->runtime->descriptors,
                              context->runtime->clock, target));
}

static void fun_conn(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  DbRef target;

  target = lookup_player(context->world, player, fargs[0], 1);
  if (is_good_obj(context->world->database, target) &&
      is_dark(context->world->database, target) &&
      !is_wizard(context->world->database, player))
    target = NOTHING;
  safe_tprintf_str(buff, bufc, "%d",
                   fetch_connect(context->runtime->descriptors,
                                 context->runtime->clock, target));
}

/*
 * ---------------------------------------------------------------------------
 * * fun_sort: Sort lists.
 */

typedef struct f_record f_rec;
typedef struct i_record i_rec;

struct f_record {
  double data;
  char *str;
};

struct i_record {
  long data;
  char *str;
};

/*
 * qsort() mandates the const void * comparator signature; casting it away
 * to read the real element type is the standard, unavoidable idiom.
 */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
static int a_comp(const void *s1, const void *s2) {
  return strcmp(*(char **)s1, *(char **)s2);
}

static int f_comp(const void *s1, const void *s2) {
  if (((f_rec *)s1)->data > ((f_rec *)s2)->data)
    return 1;
  if (((f_rec *)s1)->data < ((f_rec *)s2)->data)
    return -1;
  return 0;
}

static int i_comp(const void *s1, const void *s2) {
  if (((i_rec *)s1)->data > ((i_rec *)s2)->data)
    return 1;
  if (((i_rec *)s1)->data < ((i_rec *)s2)->data)
    return -1;
  return 0;
}
#pragma clang diagnostic pop

static void do_asort(char *s[], int n, int sort_type) {
  int i;
  f_rec *fp;
  i_rec *ip;

  switch (sort_type) {
  case ALPHANUM_LIST:
    qsort((void *)s, (size_t)n, sizeof(char *), a_comp);

    break;
  case NUMERIC_LIST:
    ip = malloc((size_t)n * sizeof(i_rec));
    for (i = 0; i < n; i++) {
      ip[i].str = s[i];
      ip[i].data = atoi(s[i]);
    }
    qsort((void *)ip, (size_t)n, sizeof(i_rec), i_comp);
    for (i = 0; i < n; i++) {
      s[i] = ip[i].str;
    }
    free(ip);
    break;
  case DBREF_LIST:
    ip = malloc((size_t)n * sizeof(i_rec));
    for (i = 0; i < n; i++) {
      ip[i].str = s[i];
      ip[i].data = dbnum(s[i]);
    }
    qsort((void *)ip, (size_t)n, sizeof(i_rec), i_comp);
    for (i = 0; i < n; i++) {
      s[i] = ip[i].str;
    }
    free(ip);
    break;
  case FLOAT_LIST:
    fp = malloc((size_t)n * sizeof(f_rec));
    for (i = 0; i < n; i++) {
      fp[i].str = s[i];
      fp[i].data = atof(s[i]);
    }
    qsort((void *)fp, (size_t)n, sizeof(f_rec), f_comp);
    for (i = 0; i < n; i++) {
      s[i] = fp[i].str;
    }
    free(fp);
    break;
  default:
    break;
  }
}

static void fun_sort(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  int nitems, sort_type;
  char *list, sep;
  char *ptrs[LBUF_SIZE / 2];

  /*
   * If we are passed an empty arglist return a null string
   */

  if (nfargs == 0) {
    return;
  }
  mvarargs_preamble("SORT", 1, 3);

  /*
   * Convert the list to an array
   */

  list = alloc_lbuf("fun_sort");
  StringCopy(list, fargs[0]);
  nitems = list2arr(ptrs, LBUF_SIZE / 2, list, sep);
  sort_type = get_list_type(fargs, nfargs, 2, ptrs, nitems);
  do_asort(ptrs, nitems, sort_type);
  arr2list(ptrs, nitems, buff, bufc, sep);
  free_lbuf(list);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_setunion, fun_setdiff, fun_setinter: Set management.
 */

constexpr int SET_UNION = 1;
constexpr int SET_INTERSECT = 2;
constexpr int SET_DIFF = 3;

static void handle_sets(char *fargs[], char *buff, char **bufc, int oper,
                        char sep) {
  char *list1, *list2, *oldp;
  char *ptrs1[LBUF_SIZE], *ptrs2[LBUF_SIZE];
  int i1, i2, n1, n2, val, first;

  list1 = alloc_lbuf("fun_setunion.1");
  StringCopy(list1, fargs[0]);
  n1 = list2arr(ptrs1, LBUF_SIZE, list1, sep);
  do_asort(ptrs1, n1, ALPHANUM_LIST);

  list2 = alloc_lbuf("fun_setunion.2");
  StringCopy(list2, fargs[1]);
  n2 = list2arr(ptrs2, LBUF_SIZE, list2, sep);
  do_asort(ptrs2, n2, ALPHANUM_LIST);

  i1 = i2 = 0;
  first = 1;
  oldp = *bufc;
  **bufc = '\0';

  switch (oper) {
  case SET_UNION: /*
                   * Copy elements common to both lists
                   */

    /*
     * Handle case of two identical single-element lists
     */

    if ((n1 == 1) && (n2 == 1) && (!strcmp(ptrs1[0], ptrs2[0]))) {
      safe_str(ptrs1[0], buff, bufc);
      break;
    }
    /*
     * Process until one list is empty
     */

    while ((i1 < n1) && (i2 < n2)) {

      /*
       * Skip over duplicates
       */

      if ((i1 > 0) || (i2 > 0)) {
        while ((i1 < n1) && !strcmp(ptrs1[i1], oldp))
          i1++;
        while ((i2 < n2) && !strcmp(ptrs2[i2], oldp))
          i2++;
      }
      /*
       * Compare and copy
       */

      if ((i1 < n1) && (i2 < n2)) {
        if (!first)
          safe_chr(sep, buff, bufc);
        first = 0;
        oldp = *bufc;
        if (strcmp(ptrs1[i1], ptrs2[i2]) < 0) {
          safe_str(ptrs1[i1], buff, bufc);
          i1++;
        } else {
          safe_str(ptrs2[i2], buff, bufc);
          i2++;
        }
        **bufc = '\0';
      }
    }

    /*
     * Copy rest of remaining list, stripping duplicates
     */

    for (; i1 < n1; i1++) {
      if (strcmp(oldp, ptrs1[i1])) {
        if (!first)
          safe_chr(sep, buff, bufc);
        first = 0;
        oldp = *bufc;
        safe_str(ptrs1[i1], buff, bufc);
        **bufc = '\0';
      }
    }
    for (; i2 < n2; i2++) {
      if (strcmp(oldp, ptrs2[i2])) {
        if (!first)
          safe_chr(sep, buff, bufc);
        first = 0;
        oldp = *bufc;
        safe_str(ptrs2[i2], buff, bufc);
        **bufc = '\0';
      }
    }
    break;
  case SET_INTERSECT: /*
                       * Copy elements not in both lists
                       */

    while ((i1 < n1) && (i2 < n2)) {
      val = strcmp(ptrs1[i1], ptrs2[i2]);
      if (!val) {

        /*
         * Got a match, copy it
         */

        if (!first)
          safe_chr(sep, buff, bufc);
        first = 0;
        oldp = *bufc;
        safe_str(ptrs1[i1], buff, bufc);
        i1++;
        i2++;
        while ((i1 < n1) && !strcmp(ptrs1[i1], oldp))
          i1++;
        while ((i2 < n2) && !strcmp(ptrs2[i2], oldp))
          i2++;
      } else if (val < 0) {
        i1++;
      } else {
        i2++;
      }
    }
    break;
  case SET_DIFF: /*
                  * Copy elements unique to list1
                  */

    while ((i1 < n1) && (i2 < n2)) {
      val = strcmp(ptrs1[i1], ptrs2[i2]);
      if (!val) {

        /*
         * Got a match, increment pointers
         */

        oldp = ptrs1[i1];
        while ((i1 < n1) && !strcmp(ptrs1[i1], oldp))
          i1++;
        while ((i2 < n2) && !strcmp(ptrs2[i2], oldp))
          i2++;
      } else if (val < 0) {

        /*
         * Item in list1 not in list2, copy
         */

        if (!first)
          safe_chr(sep, buff, bufc);
        first = 0;
        safe_str(ptrs1[i1], buff, bufc);
        oldp = ptrs1[i1];
        i1++;
        while ((i1 < n1) && !strcmp(ptrs1[i1], oldp))
          i1++;
      } else {

        /*
         * Item in list2 but not in list1, discard
         */

        oldp = ptrs2[i2];
        i2++;
        while ((i2 < n2) && !strcmp(ptrs2[i2], oldp))
          i2++;
      }
    }

    /*
     * Copy remainder of list1
     */

    while (i1 < n1) {
      if (!first)
        safe_chr(sep, buff, bufc);
      first = 0;
      safe_str(ptrs1[i1], buff, bufc);
      oldp = ptrs1[i1];
      i1++;
      while ((i1 < n1) && !strcmp(ptrs1[i1], oldp))
        i1++;
    }
  default:
    break;
  }
  free_lbuf(list1);
  free_lbuf(list2);
  return;
}

static void fun_setunion(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  char sep;

  varargs_preamble("SETUNION", 3);
  handle_sets(fargs, buff, bufc, SET_UNION, sep);
  return;
}

static void fun_setdiff(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  char sep;

  varargs_preamble("SETDIFF", 3);
  handle_sets(fargs, buff, bufc, SET_DIFF, sep);
  return;
}

static void fun_setinter(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  char sep;

  varargs_preamble("SETINTER", 3);
  handle_sets(fargs, buff, bufc, SET_INTERSECT, sep);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * rjust, ljust, center: Justify or center text, specifying fill character
 */

static void fun_ljust(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  int spaces, i;
  char sep;
  char new[LBUF_SIZE];

  varargs_preamble("LJUST", 3);
  strncpy(new, fargs[0], LBUF_SIZE - 1);
  spaces = atoi(fargs[1]) -
           (int)strlen((char *)strip_ansi_r(new, fargs[0], strlen(fargs[0])));

  /*
   * Sanitize number of spaces
   */

  if (spaces <= 0) {
    /*
     * no padding needed, just return string
     */
    safe_str(fargs[0], buff, bufc);
    return;
  } else if (spaces > LBUF_SIZE) {
    spaces = LBUF_SIZE;
  }
  safe_str(fargs[0], buff, bufc);
  for (i = 0; i < spaces; i++)
    safe_chr(sep, buff, bufc);
}

static void fun_rjust(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  int spaces, i;
  char sep;
  char new[LBUF_SIZE];

  varargs_preamble("RJUST", 3);
  strncpy(new, fargs[0], LBUF_SIZE - 1);
  spaces = atoi(fargs[1]) -
           (int)strlen((char *)strip_ansi_r(new, fargs[0], strlen(fargs[0])));

  /*
   * Sanitize number of spaces
   */

  if (spaces <= 0) {
    /*
     * no padding needed, just return string
     */
    safe_str(fargs[0], buff, bufc);
    return;
  } else if (spaces > LBUF_SIZE) {
    spaces = LBUF_SIZE;
  }
  for (i = 0; i < spaces; i++)
    safe_chr(sep, buff, bufc);
  safe_str(fargs[0], buff, bufc);
}

static void fun_center(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  char sep;
  int i, len, lead_chrs, trail_chrs, width;
  char new[LBUF_SIZE];

  varargs_preamble("CENTER", 3);

  width = atoi(fargs[1]);
  strncpy(new, fargs[0], LBUF_SIZE - 1);
  len = (int)strlen((char *)strip_ansi_r(new, fargs[0], strlen(fargs[0])));

  if (width > LBUF_SIZE) {
    safe_str("#-1 OUT OF RANGE", buff, bufc);
    return;
  }

  if (len >= width) {
    safe_str(fargs[0], buff, bufc);
    return;
  }

  lead_chrs = (int)((width / 2) - (len / 2) + .5);
  for (i = 0; i < lead_chrs; i++)
    safe_chr(sep, buff, bufc);
  safe_str(fargs[0], buff, bufc);
  trail_chrs = width - lead_chrs - len;
  for (i = 0; i < trail_chrs; i++)
    safe_chr(sep, buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * setq, setr, r: set and read global registers.
 */

static void fun_setq(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  int regnum;

  regnum = atoi(fargs[0]);
  if ((regnum < 0) || (regnum >= MAX_GLOBAL_REGS)) {
    safe_str("#-1 INVALID GLOBAL REGISTER", buff, bufc);
  } else {
    if (!context->registers[regnum])
      context->registers[regnum] = alloc_lbuf("fun_setq");
    StringCopy(context->registers[regnum], fargs[1]);
  }
}

static void fun_setr(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  int regnum;

  regnum = atoi(fargs[0]);
  if ((regnum < 0) || (regnum >= MAX_GLOBAL_REGS)) {
    safe_str("#-1 INVALID GLOBAL REGISTER", buff, bufc);
    return;
  } else {
    if (!context->registers[regnum])
      context->registers[regnum] = alloc_lbuf("fun_setq");
    StringCopy(context->registers[regnum], fargs[1]);
  }
  safe_str(fargs[1], buff, bufc);
}

static void fun_r(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  int regnum;

  regnum = atoi(fargs[0]);
  if ((regnum < 0) || (regnum >= MAX_GLOBAL_REGS)) {
    safe_str("#-1 INVALID GLOBAL REGISTER", buff, bufc);
  } else if (context->registers[regnum]) {
    safe_str(context->registers[regnum], buff, bufc);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * isnum: is the argument a number?
 */

static void fun_isnum(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  safe_str((is_number(fargs[0]) ? "1" : "0"), buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * isdbref: is the argument a valid dbref?
 */

static void fun_isdbref(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  char *p;
  DbRef dbitem;

  p = fargs[0];
  if (*p++ == NUMBER_TOKEN) {
    dbitem = parse_dbref(p);
    if (is_good_obj(context->world->database, dbitem)) {
      safe_str("1", buff, bufc);
      return;
    }
  }
  safe_str("0", buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * trim: trim off unwanted white space.
 */

static void fun_trim(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  char *p, *lastchar, *q, sep;
  int trim;

  if (nfargs == 0) {
    return;
  }
  mvarargs_preamble("TRIM", 1, 3);
  if (nfargs >= 2) {
    switch (ToLower(*fargs[1])) {
    case 'l':
      trim = 1;
      break;
    case 'r':
      trim = 2;
      break;
    default:
      trim = 3;
      break;
    }
  } else {
    trim = 3;
  }

  if (trim == 2 || trim == 3) {
    p = lastchar = fargs[0];
    while (*p != '\0') {
      if (*p != sep)
        lastchar = p;
      p++;
    }
    *(lastchar + 1) = '\0';
  }
  q = fargs[0];
  if (trim == 1 || trim == 3) {
    while (*q != '\0') {
      if (*q == sep)
        q++;
      else
        break;
    }
  }
  safe_str(q, buff, bufc);
}

#ifdef ARBITRARY_LOGFILES
static void fun_logf(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  if (!fargs[0] || !fargs[1]) {
    safe_str("#-1 INVALID ARGUMENTS", buff, bufc);
  }
  if (log_to_file(context, player, fargs[0], fargs[1]))
    return;

  safe_str("#-1 INVALID LOGFILE", buff, bufc);
  return;
}
#endif

/* ----------------------------------------------------------------------
 ** fun_pairs: take an attr off an object and count the # of
 ** {[()]} in that attribute and return it as a list
 ** Modified from fun_get
 ** Dany - 06/2005
 */

/* ----------------------------------------------------------------------
 ** fun_colorpairs: take an attr off an object and color the
 ** {[()]} in that attribute and return it.
 ** Modified from fun_get
 ** Dany - 09/2005
 */

/* ---------------------------------------------------------------------------
 * flist: List of existing functions in alphabetical order.
 */

FUN flist[] = {
    //      {"FUNCNAME", fun_function, args, ?, permissions},
    {"@@", fun_double_at, 1, 0, CA_PUBLIC},
    {"ABS", fun_abs, 1, 0, CA_PUBLIC},
    {"ACOS", fun_acos, 1, 0, CA_PUBLIC},
    {"ADD", fun_add, 0, FN_VARARGS, CA_PUBLIC},
    {"AFTER", fun_after, 0, FN_VARARGS, CA_PUBLIC},
    {"ALPHAMAX", fun_alphamax, 0, FN_VARARGS, CA_PUBLIC},
    {"ALPHAMIN", fun_alphamin, 0, FN_VARARGS, CA_PUBLIC},
    {"AND", fun_and, 0, FN_VARARGS, CA_PUBLIC},
    {"ANDFLAGS", fun_andflags, 2, 0, CA_PUBLIC},
    {"ANSI", fun_ansi, 2, 0, CA_PUBLIC},
    {"ANSISECURE", fun_ansi_secure, -1, 0, CA_PUBLIC},
    {"APOSS", fun_aposs, 1, 0, CA_PUBLIC},
    {"ART", fun_art, 1, 0, CA_PUBLIC},
    {"ASIN", fun_asin, 1, 0, CA_PUBLIC},
    {"ATAN", fun_atan, 1, 0, CA_PUBLIC},
    {"BEEP", fun_beep, 0, 0, CA_WIZARD},
    {"BEFORE", fun_before, 0, FN_VARARGS, CA_PUBLIC},
    {"BTADDSTORES", fun_btaddstores, 3, 0, CA_WIZARD},
    {"BTARMORSTATUS", fun_btarmorstatus, 2, 0, CA_WIZARD},
    {"BTARMORSTATUS_REF", fun_btarmorstatus_ref, 2, 0, CA_WIZARD},
    {"BTCHARLIST", fun_btcharlist, 1, FN_VARARGS, CA_WIZARD},
    {"BTCRITSLOT", fun_btcritslot, 0, FN_VARARGS, CA_WIZARD},
    {"BTCRITSLOT_REF", fun_btcritslot_ref, 0, FN_VARARGS, CA_WIZARD},
    {"BTSECTSTATUS", fun_btsectstatus, 2, 0, CA_WIZARD},
    {"BTCRITSTATUS", fun_btcritstatus, 2, 0, CA_WIZARD},
    {"BTCRITSTATUS_REF", fun_btcritstatus_ref, 2, 0, CA_WIZARD},
    {"BTDAMAGEMECH", fun_btdamagemech, 7, 0, CA_WIZARD},
    {"BTDAMAGES", fun_btdamages, 1, 0, CA_WIZARD},
    {"BTDESIGNEX", fun_btdesignex, 1, 0, CA_PUBLIC},
    {"BTENGRATE", fun_btengrate, 1, 0, CA_WIZARD},
    {"BTENGRATE_REF", fun_btengrate_ref, 1, 0, CA_WIZARD},
#ifdef BT_ADVANCED_ECON
    {"BTFASABASECOST_REF", fun_btfasabasecost_ref, 1, 0, CA_WIZARD},
#endif
    {"BTGETBV", fun_btgetbv, 1, 0, CA_WIZARD},
    {"BTGETBV_REF", fun_btgetbv_ref, 1, 0, CA_WIZARD},
    {"BTGETBV2_REF", fun_btgetbv2_ref, 1, 0, CA_WIZARD},
    {"BTGETDBV_REF", fun_btgetdbv_ref, 1, 0, CA_WIZARD},
    {"BTGETOBV_REF", fun_btgetobv_ref, 1, 0, CA_WIZARD},
    {"BTGETCHARVALUE", fun_btgetcharvalue, 3, 0, CA_WIZARD},
#ifdef BT_ADVANCED_ECON
    {"BTGETPARTCOST", fun_btgetpartcost, 1, 0, CA_WIZARD},
#endif
    {"BTGETRANGE", fun_btgetrange, 0, FN_VARARGS, CA_WIZARD},
    {"BTGETREALMAXSPEED", fun_btgetrealmaxspeed, 1, 0, CA_WIZARD},
    {"BTGETWEIGHT", fun_btgetweight, 1, 0, CA_WIZARD},
    {"BTGETXCODEVALUE", fun_btgetxcodevalue, 2, 0, CA_WIZARD},
    {"BTGETXCODEVALUE_REF", fun_btgetxcodevalue_ref, 2, 0, CA_WIZARD},
    {"BTHEXEMIT", fun_bthexemit, 4, 0, CA_WIZARD},
    {"BTHEXINBLZ", fun_bthexinblz, 3, 0, CA_WIZARD},
    {"BTHEXLOS", fun_bthexlos, 3, 0, CA_WIZARD},
    {"BTID2DB", fun_btid2db, 2, 0, CA_WIZARD},
    {"BTLAG", fun_btlag, 0, 0, CA_WIZARD},
    {"BTLISTBLZ", fun_btlistblz, 1, 0, CA_WIZARD},
    {"BTLOADMAP", fun_btloadmap, 2, FN_VARARGS, CA_WIZARD},
    {"BTLOADMECH", fun_btloadmech, 2, 0, CA_WIZARD},
    {"BTLOSM2M", fun_btlosm2m, 2, 0, CA_WIZARD},
    {"BTMAKEPILOTROLL", fun_btmakepilotroll, 3, 0, CA_WIZARD},
    {"BTMAPELEV", fun_btmapelev, 3, 0, CA_WIZARD},
    {"BTMAPEMIT", fun_btmapemit, 0, FN_VARARGS, CA_WIZARD},
    {"BTMAPTERR", fun_btmapterr, 3, 0, CA_WIZARD},
    {"BTMAPUNITS", fun_btmapunits, 0, FN_VARARGS, CA_WIZARD},
    {"BTMECHFREQS", fun_btmechfreqs, 1, 0, CA_WIZARD},
    {"BTNUMREPJOBS", fun_btnumrepjobs, 1, 0, CA_WIZARD},
    {"BTPARTTYPE", fun_btparttype, 1, 0, CA_WIZARD},
    {"BTPARTMATCH", fun_btpartmatch, 1, 0, CA_WIZARD},
    {"BTPARTNAME", fun_btpartname, 2, 0, CA_WIZARD},
    {"BTPARTSCATEGORYLIST", fun_btpartscategorylist, 0, 0, CA_WIZARD},
    {"BTPARTSLIST", fun_btpartslist, 1, 0, CA_WIZARD},
    {"BTPARTWEIGHT", fun_btgetweight, 1, 0, CA_WIZARD},
    {"BTPAYLOAD_REF", fun_btpayload_ref, 1, 0, CA_WIZARD},
    {"BTREMOVESTORES", fun_btremovestores, 3, 0, CA_WIZARD},
    {"BTSETARMORSTATUS", fun_btsetarmorstatus, 4, 0, CA_WIZARD},
    {"BTSETCHARVALUE", fun_btsetcharvalue, 4, 0, CA_WIZARD},
    {"BTSETMAXSPEED", fun_btsetmaxspeed, 2, 0, CA_WIZARD},
#ifdef BT_ADVANCED_ECON
    {"BTSETPARTCOST", fun_btsetpartcost, 2, 0, CA_WIZARD},
#endif
    {"BTSETTONS", fun_btsettons, 2, 0, CA_WIZARD},
    {"BTSETXCODEVALUE", fun_btsetxcodevalue, 3, 0, CA_WIZARD},
    {"BTSETXY", fun_btsetxy, 0, FN_VARARGS, CA_WIZARD},
    {"BTSHOWCRITSTATUS_REF", fun_btshowcritstatus_ref, 3, 0, CA_WIZARD},
    {"BTSHOWSTATUS_REF", fun_btshowstatus_ref, 2, 0, CA_WIZARD},
    {"BTSHOWWSPECS_REF", fun_btshowwspecs_ref, 2, 0, CA_WIZARD},
    {"BTSTORES", fun_btstores, 0, FN_VARARGS, CA_WIZARD},
    {"BTSTORES_SHORT", fun_btstores_short, 0, FN_VARARGS, CA_WIZARD},
    {"BTTECHLIST", fun_bttechlist, 1, 0, CA_WIZARD},
    {"BTTECHLIST_REF", fun_bttechlist_ref, 1, 0, CA_WIZARD},
    {"BTTECHSTATUS", fun_bttechstatus, 1, 0, CA_WIZARD},
    {"BTTECHTIME", fun_bttechtime, 0, 0, CA_WIZARD},
    {"BTTHRESHOLD", fun_btthreshold, 1, 0, CA_WIZARD},
    {"BTTICWEAPS", fun_btticweaps, 2, 0, CA_WIZARD},
    {"BTUNDERREPAIR", fun_btunderrepair, 1, 0, CA_WIZARD},
    {"BTUNITFIXABLE", fun_btunitfixable, 1, 0, CA_WIZARD},
    {"BTUNITPARTSLIST", fun_btunitpartslist, 1, 0, CA_WIZARD},
    {"BTUNITPARTSLIST_REF", fun_btunitpartslist_ref, 1, 0, CA_WIZARD},
    {"BTUPDATELINKS", fun_btupdatelinks, 1, 0, CA_WIZARD},
    /*AAA*/ {"BTWEAPONS", fun_btweapons, 1, 0, CA_WIZARD},
    {"BTWEAPONSTATUS", fun_btweaponstatus, 0, FN_VARARGS, CA_WIZARD},
    {"BTWEAPONSTATUS_REF", fun_btweaponstatus_ref, 0, FN_VARARGS, CA_WIZARD},
    {"BTWEAPSTAT", fun_btweapstat, 2, 0, CA_WIZARD},
    {"CAPSTR", fun_capstr, -1, 0, CA_PUBLIC},
    {"CART2HEX", fun_cart2hex, 2, 0, CA_PUBLIC},
    {"CASE", fun_case, 0, FN_VARARGS | FN_NO_EVAL, CA_PUBLIC},
    {"CAT", fun_cat, 0, FN_VARARGS, CA_PUBLIC},
    {"CEIL", fun_ceil, 1, 0, CA_PUBLIC},
    {"CENTER", fun_center, 0, FN_VARARGS, CA_PUBLIC},
    {"CIRCUMCENTER", fun_circumcenter, 6, 0, CA_PUBLIC},
    {"COBJ", fun_cobj, 1, 0, CA_PUBLIC},
    {"COLUMNS", fun_columns, 0, FN_VARARGS, CA_PUBLIC},
    {"COMP", fun_comp, 2, 0, CA_PUBLIC},
    {"CON", fun_con, 1, 0, CA_PUBLIC},
    {"CONFIG", fun_config, 1, 0, CA_WIZARD},
    {"CONN", fun_conn, 1, 0, CA_PUBLIC},
    {"CONNRECORD", fun_connrecord, 0, 0, CA_PUBLIC},
    {"CONTROLS", fun_controls, 2, 0, CA_PUBLIC},
    {"CONVSECS", fun_convsecs, 1, 0, CA_PUBLIC},
    {"CONVTIME", fun_convtime, 1, 0, CA_PUBLIC},
    {"CONVUPTIME", fun_convuptime, 1, 0, CA_PUBLIC},
    {"COS", fun_cos, 1, 0, CA_PUBLIC},
    {"CREATE", fun_create, 0, FN_VARARGS, CA_PUBLIC},
    {"CWHO", fun_cwho, 1, 0, CA_PUBLIC},
    {"CLIST", fun_clist, 1, 0, CA_PUBLIC},
    {"CEMIT", fun_cemit, 2, 0, CA_PUBLIC},
    {"DEC", fun_dec, 1, 0, CA_PUBLIC},
    {"DECRYPT", fun_decrypt, 2, 0, CA_PUBLIC},
    {"DELETE", fun_delete, 3, 0, CA_PUBLIC},
    {"DIGITTIME", fun_digittime, 1, 0, CA_PUBLIC},
    {"DIE", fun_die, 2, 0, CA_PUBLIC},
    {"DIST2D", fun_dist2d, 4, 0, CA_PUBLIC},
    {"DIST3D", fun_dist3d, 6, 0, CA_PUBLIC},
    {"DIV", fun_div, 2, 0, CA_PUBLIC},
    {"E", fun_e, 0, 0, CA_PUBLIC},
    {"EDIT", fun_edit, 3, 0, CA_PUBLIC},
    {"ELEMENTS", fun_elements, 0, FN_VARARGS, CA_PUBLIC},
    {"EMPTY", fun_empty, 0, FN_VARARGS, CA_PUBLIC},
    {"ENCRYPT", fun_encrypt, 2, 0, CA_PUBLIC},
    {"EQ", fun_eq, 2, 0, CA_PUBLIC},
    {"ESCAPE", fun_escape, -1, 0, CA_PUBLIC},
    {"EXIT", fun_exit, 1, 0, CA_PUBLIC},
    {"EXP", fun_exp, 1, 0, CA_PUBLIC},
    {"EXTRACT", fun_extract, 0, FN_VARARGS, CA_PUBLIC},
    {"EVAL", fun_eval, 0, FN_VARARGS, CA_PUBLIC},
    {"FDIV", fun_fdiv, 2, 0, CA_PUBLIC},
    {"FINDABLE", fun_findable, 2, 0, CA_PUBLIC},
    {"FIRST", fun_first, 0, FN_VARARGS, CA_PUBLIC},
    {"FLAGS", fun_flags, 1, 0, CA_PUBLIC},
    {"FLOOR", fun_floor, 1, 0, CA_PUBLIC},
    {"FULLNAME", fun_fullname, 1, 0, CA_PUBLIC},
    {"GRAB", fun_grab, 0, FN_VARARGS, CA_PUBLIC},
    {"GRABALL", fun_graball, 0, FN_VARARGS, CA_PUBLIC},
    {"GT", fun_gt, 2, 0, CA_PUBLIC},
    {"GTE", fun_gte, 2, 0, CA_PUBLIC},
    {"HASFLAG", fun_hasflag, 2, 0, CA_PUBLIC},
    {"HASPOWER", fun_haspower, 2, 0, CA_PUBLIC},
    {"HASTYPE", fun_hastype, 2, 0, CA_PUBLIC},
    {"HEX2CART", fun_hex2cart, 2, 0, CA_PUBLIC},
    {"HOME", fun_home, 1, 0, CA_PUBLIC},
    {"IDLE", fun_idle, 1, 0, CA_PUBLIC},
    {"IFELSE", fun_ifelse, 3, FN_NO_EVAL, CA_PUBLIC},
    {"INC", fun_inc, 1, 0, CA_PUBLIC},
    {"INDEX", fun_index, 4, 0, CA_PUBLIC},
    {"INSERT", fun_insert, 0, FN_VARARGS, CA_PUBLIC},
    {"INZONE", fun_inzone, 1, 0, CA_PUBLIC},
    {"ISDBREF", fun_isdbref, 1, 0, CA_PUBLIC},
    {"ISNUM", fun_isnum, 1, 0, CA_PUBLIC},
    {"ISWORD", fun_isword, 1, 0, CA_PUBLIC},
    {"ITEMS", fun_items, 0, FN_VARARGS, CA_PUBLIC},
    {"ITER", fun_iter, 0, FN_VARARGS | FN_NO_EVAL, CA_PUBLIC},
    {"LAST", fun_last, 0, FN_VARARGS, CA_PUBLIC},
    {"LCON", fun_lcon, 1, 0, CA_PUBLIC},
    {"LCSTR", fun_lcstr, -1, 0, CA_PUBLIC},
    {"LDELETE", fun_ldelete, 0, FN_VARARGS, CA_PUBLIC},
    {"LEXITS", fun_lexits, 1, 0, CA_PUBLIC},
    {"LIST", fun_list, 0, FN_VARARGS | FN_NO_EVAL, CA_PUBLIC},
    {"LIT", fun_lit, 1, FN_NO_EVAL, CA_PUBLIC},
    {"LJUST", fun_ljust, 0, FN_VARARGS, CA_PUBLIC},
    {"LINK", fun_link, 2, 0, CA_PUBLIC},
    {"LN", fun_ln, 1, 0, CA_PUBLIC},
    {"LNUM", fun_lnum, 0, FN_VARARGS, CA_PUBLIC},
    {"LOC", fun_loc, 1, 0, CA_WIZARD},
    {"LOCATE", fun_locate, 3, 0, CA_WIZARD},
    {"LOG", fun_log, 1, 0, CA_PUBLIC},
#ifdef ARBITRARY_LOGFILES
    {"LOGF", fun_logf, 2, 0, CA_WIZARD},
#endif
    {"LPLAYERS", fun_lplayers, 1, 0, CA_PUBLIC},
    {"LSTACK", fun_lstack, 0, FN_VARARGS, CA_PUBLIC},
    {"LT", fun_lt, 2, 0, CA_PUBLIC},
    {"LTE", fun_lte, 2, 0, CA_PUBLIC},
    {"LVPLAYERS", fun_lvplayers, 1, 0, CA_PUBLIC},
    {"LWHO", fun_lwho, 0, 0, CA_WIZARD},
    {"MATCH", fun_match, 0, FN_VARARGS, CA_PUBLIC},
    {"MATCHALL", fun_matchall, 0, FN_VARARGS, CA_PUBLIC},
    {"MAX", fun_max, 0, FN_VARARGS, CA_PUBLIC},
    {"MEMBER", fun_member, 0, FN_VARARGS, CA_PUBLIC},
    {"MERGE", fun_merge, 3, 0, CA_PUBLIC},
    {"MID", fun_mid, 3, 0, CA_PUBLIC},
    {"MIN", fun_min, 0, FN_VARARGS, CA_PUBLIC},
    {"MOD", fun_mod, 2, 0, CA_PUBLIC},
    {"MUDNAME", fun_mudname, 0, 0, CA_PUBLIC},
    {"MUL", fun_mul, 0, FN_VARARGS, CA_PUBLIC},
    {"NAME", fun_name, 1, 0, CA_PUBLIC},
    {"NEARBY", fun_nearby, 2, 0, CA_PUBLIC},
    {"NEQ", fun_neq, 2, 0, CA_PUBLIC},
    {"NEXT", fun_next, 1, 0, CA_PUBLIC},
    {"NOT", fun_not, 1, 0, CA_PUBLIC},
    {"NUM", fun_num, 1, 0, CA_PUBLIC},
    {"OBJ", fun_obj, 1, 0, CA_PUBLIC},
    {"OBJEVAL", fun_objeval, 2, FN_NO_EVAL, CA_PUBLIC},
    {"OBJMEM", fun_objmem, 1, 0, CA_PUBLIC},
    {"OR", fun_or, 0, FN_VARARGS, CA_PUBLIC},
    {"ORFLAGS", fun_orflags, 2, 0, CA_PUBLIC},
    {"OWNER", fun_owner, 1, 0, CA_PUBLIC},
    {"PARSE", fun_parse, 0, FN_VARARGS | FN_NO_EVAL, CA_PUBLIC},
    {"PEEK", fun_peek, 0, FN_VARARGS, CA_PUBLIC},
    {"PEMIT", fun_pemit, 2, 0, CA_WIZARD},
    {"PI", fun_pi, 0, 0, CA_PUBLIC},
    {"PLAYMEM", fun_playmem, 1, 0, CA_PUBLIC},
    {"PMATCH", fun_pmatch, 1, 0, CA_PUBLIC},
    {"POP", fun_pop, 0, FN_VARARGS, CA_PUBLIC},
    {"PORTS", fun_ports, 1, 0, CA_PUBLIC},
    {"POS", fun_pos, 2, 0, CA_PUBLIC},
    {"POSS", fun_poss, 1, 0, CA_PUBLIC},
    {"POWER", fun_power, 2, 0, CA_PUBLIC},
    {"PUSH", fun_push, 0, FN_VARARGS, CA_PUBLIC},
    {"R", fun_r, 1, 0, CA_PUBLIC},
    {"RAND", fun_rand, 1, 0, CA_PUBLIC},
    {"REGMATCH", fun_regmatch, 0, FN_VARARGS, CA_PUBLIC},
    {"REMOVE", fun_remove, 0, FN_VARARGS, CA_PUBLIC},
    {"REPEAT", fun_repeat, 2, 0, CA_PUBLIC},
    {"REPLACE", fun_replace, 0, FN_VARARGS, CA_PUBLIC},
    {"REST", fun_rest, 0, FN_VARARGS, CA_PUBLIC},
    {"REVERSE", fun_reverse, -1, 0, CA_PUBLIC},
    {"REVWORDS", fun_revwords, 0, FN_VARARGS, CA_PUBLIC},
    {"RJUST", fun_rjust, 0, FN_VARARGS, CA_PUBLIC},
    {"RLOC", fun_rloc, 2, 0, CA_PUBLIC},
    {"ROOM", fun_room, 1, 0, CA_PUBLIC},
    {"ROUND", fun_round, 2, 0, CA_PUBLIC},
    {"SCRAMBLE", fun_scramble, 1, 0, CA_PUBLIC},
    {"SEARCH", fun_search, -1, 0, CA_PUBLIC},
    {"SECS", fun_secs, 0, 0, CA_PUBLIC},
    {"SECURE", fun_secure, -1, 0, CA_PUBLIC},
    {"SETDIFF", fun_setdiff, 0, FN_VARARGS, CA_PUBLIC},
    {"SETINTER", fun_setinter, 0, FN_VARARGS, CA_PUBLIC},
    {"SETQ", fun_setq, 2, 0, CA_PUBLIC},
    {"SETR", fun_setr, 2, 0, CA_PUBLIC},
    {"SETUNION", fun_setunion, 0, FN_VARARGS, CA_PUBLIC},
    {"SHL", fun_shl, 2, 0, CA_PUBLIC},
    {"SHR", fun_shr, 2, 0, CA_PUBLIC},
    {"SHUFFLE", fun_shuffle, 0, FN_VARARGS, CA_PUBLIC},
    {"SIGN", fun_sign, 1, 0, CA_PUBLIC},
    {"SIN", fun_sin, 1, 0, CA_PUBLIC},
    {"SORT", fun_sort, 0, FN_VARARGS, CA_PUBLIC},
    {"SPACE", fun_space, 1, 0, CA_PUBLIC},
    {"SPLICE", fun_splice, 0, FN_VARARGS, CA_PUBLIC},
    {"SQRT", fun_sqrt, 1, 0, CA_PUBLIC},
    {"SQUISH", fun_squish, 1, 0, CA_PUBLIC},
    {"STARTSECS", fun_startsecs, 0, 0, CA_PUBLIC},
    {"STARTTIME", fun_starttime, 0, 0, CA_PUBLIC},
    {"STATS", fun_stats, 1, 0, CA_PUBLIC},
    {"STRCAT", fun_strcat, 0, FN_VARARGS, CA_PUBLIC},
    {"STRIPANSI", fun_stripansi, 1, 0, CA_PUBLIC},
    {"STRLEN", fun_strlen, -1, 0, CA_PUBLIC},
    {"STRMATCH", fun_strmatch, 2, 0, CA_PUBLIC},
    {"STRTRUNC", fun_strtrunc, 2, 0, CA_PUBLIC},
    {"SUB", fun_sub, 2, 0, CA_PUBLIC},
    {"SUBEVAL", fun_subeval, 1, 0, CA_PUBLIC},
    {"SUM", fun_sum, 0, FN_VARARGS | FN_NO_EVAL, CA_PUBLIC},
    {"SWITCH", fun_switch, 0, FN_VARARGS | FN_NO_EVAL, CA_PUBLIC},
    {"TAN", fun_tan, 1, 0, CA_PUBLIC},
    {"T", fun_t, 1, 0, CA_PUBLIC},
    {"TEL", fun_tel, 2, 0, CA_PUBLIC},
    {"TIME", fun_time, 0, 0, CA_PUBLIC},
    {"TRANSLATE", fun_translate, 2, 0, CA_PUBLIC},
    {"TRIM", fun_trim, 0, FN_VARARGS, CA_PUBLIC},
    {"TRUE", fun_t, 1, 0, CA_PUBLIC},
    {"TRUNC", fun_trunc, 1, 0, CA_PUBLIC},
    {"TYPE", fun_type, 1, 0, CA_PUBLIC},
    {"UCSTR", fun_ucstr, -1, 0, CA_PUBLIC},
    {"V", fun_v, 1, 0, CA_PUBLIC},
    {"VADD", fun_vadd, 0, FN_VARARGS, CA_PUBLIC},
    {"VALID", fun_valid, 2, FN_VARARGS, CA_PUBLIC},
    {"VDIM", fun_vdim, 0, FN_VARARGS, CA_PUBLIC},
    {"VERSION", fun_version, 0, 0, CA_PUBLIC},
    {"VISIBLE", fun_visible, 2, 0, CA_PUBLIC},
    {"VMAG", fun_vmag, 0, FN_VARARGS, CA_PUBLIC},
    {"VMUL", fun_vmul, 0, FN_VARARGS, CA_PUBLIC},
    {"VSUB", fun_vsub, 0, FN_VARARGS, CA_PUBLIC},
    {"VUNIT", fun_vunit, 0, FN_VARARGS, CA_PUBLIC},
    {"WHERE", fun_where, 1, 0, CA_PUBLIC},
    {"WORDPOS", fun_wordpos, 0, FN_VARARGS, CA_PUBLIC},
    {"WORDS", fun_words, 0, FN_VARARGS, CA_PUBLIC},
    {"XOR", fun_xor, 0, FN_VARARGS, CA_PUBLIC},
    {"ZEXITS", fun_zexits, 1, 0, CA_PUBLIC},
    {"ZMECHS", fun_zmechs, 1, 0, CA_PUBLIC},
    {"ZOBJECTS", fun_zobjects, 1, 0, CA_PUBLIC},
    {"ZONE", fun_zone, 1, 0, CA_PUBLIC},
    {"ZPLAYERS", fun_zplayers, 1, 0, CA_PUBLIC},
    {"ZROOMS", fun_zrooms, 1, 0, CA_PUBLIC},
    {"ZWHO", fun_zwho, 1, 0, CA_PUBLIC},
    {nullptr, nullptr, 0, 0, 0}};

/* *INDENT-ON* */

void init_functab(MuxServer *server) {
  FUN *fp;
  char *buff, *dp;
  const char *cp;

  buff = alloc_sbuf("init_functab");
  hash_table_initialize(&server->command_registry.functions, 100 * HASH_FACTOR);
  for (fp = flist; fp->name; fp++) {
    cp = fp->name;
    dp = buff;
    while (*cp) {
      *dp = ToLower(*cp);
      cp++;
      dp++;
    }
    *dp = '\0';
    hash_table_add(buff, (int *)fp, &server->command_registry.functions);
  }
  free_sbuf(buff);
  server->command_registry.user_functions = nullptr;
  hash_table_initialize(&server->command_registry.user_function_index, 11);
}

void do_function(CommandInvocation *invocation) {
  EvaluationContext *context = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *fname = invocation->first;
  char *target = invocation->second;
  CommandRegistry *registry = invocation->context->runtime->command_registry;
  UFUN *ufp, *ufp2;
  Attribute *ap;
  char *np, *bp;
  int atr;
  long aflags;
  DbRef obj, aowner;

  /*
   * Make a local uppercase copy of the function name
   */

  bp = np = alloc_sbuf("add_user_func");
  safe_sb_str(fname, np, &bp);
  *bp = '\0';
  for (bp = np; *bp; bp++)
    *bp = ToLower(*bp);

  /*
   * Verify that the function doesn't exist in the builtin table
   */

  if (hash_table_find(np, &registry->functions) != nullptr) {
    notify_quiet(context, player,
                 "Function already defined in builtin function table.");
    free_sbuf(np);
    return;
  }
  /*
   * Make sure the target object exists
   */

  if (!parse_attrib(&invocation->context->match, player, target, &obj, &atr)) {
    notify_quiet(context, player, "I don't see that here.");
    free_sbuf(np);
    return;
  }
  /*
   * Make sure the attribute exists
   */

  if (atr == NOTHING) {
    notify_quiet(context, player, "No such attribute.");
    free_sbuf(np);
    return;
  }
  /*
   * Make sure attribute is readably by me
   */

  ap = attribute_by_number(context->world->database, atr);
  if (!ap) {
    notify_quiet(context, player, "No such attribute.");
    free_sbuf(np);
    return;
  }
  attribute_get_info(context->world->database, obj, atr, &aowner, &aflags);
  if (!see_attr(context, player, obj, ap, aowner, aflags)) {
    notify_quiet(context, player, "Permission denied.");
    free_sbuf(np);
    return;
  }
  /*
   * Privileged functions require you control the obj.
   */

  if ((key & FN_PRIV) && !is_controls(context, player, obj)) {
    notify_quiet(context, player, "Permission denied.");
    free_sbuf(np);
    return;
  }
  /*
   * See if function already exists.  If so, redefine it
   */

  ufp = (UFUN *)hash_table_find(np, &registry->user_function_index);

  if (!ufp) {
    ufp = malloc(sizeof(UFUN));
    ufp->name = strsave(np);
    /* Upcase the name in place right after allocating it, before anyone
       else can hold a reference to it. */
    for (bp = ufp->name; *bp; bp++)
      *bp = ToUpper(*bp);
    ufp->obj = obj;
    ufp->atr = atr;
    ufp->perms = CA_PUBLIC;
    ufp->next = nullptr;
    if (!registry->user_functions) {
      registry->user_functions = ufp;
    } else {
      for (ufp2 = registry->user_functions; ufp2->next; ufp2 = ufp2->next)
        ;
      ufp2->next = ufp;
    }
    hash_table_add(np, (int *)ufp, &registry->user_function_index);
  }
  ufp->obj = obj;
  ufp->atr = atr;
  ufp->flags = key;
  free_sbuf(np);
  if (!is_quiet(context->world->database, player)) {
    char buffer[MBUF_SIZE];
    snprintf(buffer, MBUF_SIZE - 1, "Function %s defined.", fname);
    notify_quiet(context, player, buffer);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * list_functable: List available functions.
 */

void list_functable(EvaluationContext *context,
                    const ServerConfiguration *configuration,
                    CommandRegistry *registry, DbRef player) {
  FUN *fp;
  UFUN *ufp;
  char *buf, *bp;
  const char *cp;

  buf = alloc_lbuf("list_functable");
  bp = buf;

  /* Hardcoded Functions */
  for (cp = "Functions:"; *cp; cp++)
    *bp++ = *cp;
  for (fp = flist; fp->name; fp++) {
    if (check_access(context->world->database, configuration, player,
                     fp->perms)) {
      *bp++ = ' ';
      for (cp = fp->name; *cp; cp++)
        *bp++ = *cp;
    }
  }
  *bp = '\0';
  notify(context, player, buf);

  /* User-Defined functions (via @function) */
  bp = buf;
  safe_str("User-Functions:", buf, &bp);

  for (ufp = registry->user_functions; ufp; ufp = ufp->next) {
    if (check_access(context->world->database, configuration, player,
                     ufp->perms)) {
      *bp++ = ' ';
      for (cp = ufp->name; *cp; cp++)
        *bp++ = *cp;
    }
  }
  *bp = '\0';
  notify(context, player, buf);
  free_lbuf(buf);
}

/*
 * ---------------------------------------------------------------------------
 * * cf_func_access: set access on functions
 */

int cf_func_access(int *vp, char *str, long extra, DbRef player, char *cmd,
                   ConfigurationContext *context) {
  FUN *fp;
  UFUN *ufp;
  char *ap;

  for (ap = str; *ap && !isspace(*ap); ap++)
    ;
  if (*ap)
    *ap++ = '\0';

  for (fp = flist; fp->name; fp++) {
    if (!string_compare(context->configuration, fp->name, str)) {
      return configuration_modify_bits(&fp->perms, ap, extra, player, cmd,
                                       context);
    }
  }
  for (ufp = context->command_registry->user_functions; ufp; ufp = ufp->next) {
    if (!string_compare(context->configuration, ufp->name, str)) {
      return configuration_modify_bits(&ufp->perms, ap, extra, player, cmd,
                                       context);
    }
  }
  configuration_log_not_found(context, player, cmd, "Function", str);
  return -1;
}
