/*
 * funceval.c - MUX function handlers
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include <limits.h>
#include <math.h>
#include <regex.h>

#include "mux/commands/command.h"
#include "mux/commands/functions.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/comsys.h"
#include "mux/communication/speech.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/network/netcommon.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include "mux/world/search.h"
#include "mux/world/walkdb.h"

#define NSUBEXP 10

/*
 * Note: Many functions in this file have been taken, whole or in part, from
 * PennMUSH 1.50, and TinyMUSH 2.2, for softcode compatibility. The
 * maintainers of MUX would like to thank those responsible for PennMUSH 1.50
 * and TinyMUSH 2.2, and hope we have adequately noted in the source where
 * credit is due.
 */

extern NameTable indiv_attraccess_nametab[];

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

/* Returns the dbref of the specified channel's channel object. */
void fun_cobj(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  struct channel *ch;

  if (!(ch = select_channel(context->runtime->channels, fargs[0]))) {
    safe_str("#-1 CHANNEL NOT FOUND", buff, bufc);
    return;
  }
  if (!context->world->configuration->have_comsys ||
      !is_comm_all(context->world->database, player)) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  if (ch->chan_obj == -1) {
    safe_str("#-1 NO CHANNEL OBJECT", buff, bufc);
    return;
  }
  safe_tprintf_str(buff, bufc, "#%d", ch->chan_obj);
}

/* Lists who is on a channel. */
void fun_cwho(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  struct channel *ch;
  struct comuser *user;
  size_t len = 0;
  static char smbuf[SBUF_SIZE];

  if (!(ch = select_channel(context->runtime->channels, fargs[0]))) {
    safe_str("#-1 CHANNEL NOT FOUND", buff, bufc);
    return;
  }
  if (!context->world->configuration->have_comsys ||
      !is_comm_all(context->world->database, player)) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  for (user = ch->on_users; user; user = user->on_next) {

    /*      if (is_connected(context->world->database, user->who)) */
    {
      if (len) {
        snprintf(smbuf, sizeof(smbuf), " #%ld", user->who);
        if ((strlen(smbuf) + len) > (LBUF_SIZE - SBUF_SIZE)) {
          safe_str(" #-1", buff, bufc);
          return;
        }
        safe_str(smbuf, buff, bufc);
        len += strlen(smbuf);
      } else {
        safe_tprintf_str(buff, bufc, "#%ld", user->who);
        len = strlen(buff);
      }
    }
  }
}

/* Returns a list of all channels. */
void fun_clist(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context) {
  struct channel *ch;

  if (!(ch = select_channel(context->runtime->channels, fargs[0]))) {
    safe_str("#-1 CHANNEL NOT FOUND", buff, bufc);
    return;
  }
  if (!context->world->configuration->have_comsys ||
      !is_comm_all(context->world->database, player)) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }

  for (ch = (struct channel *)hash_table_first_entry(
           &context->runtime->channels->channels);
       ch; ch = (struct channel *)hash_table_next_entry(
               &context->runtime->channels->channels)) {
    safe_tprintf_str(buff, bufc, "%s ", ch->name);
  }
}

void fun_beep(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  safe_chr(BEEP_CHAR, buff, bufc);
}

/*
 * This function was originally taken from PennMUSH 1.50
 */

void fun_ansi(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  char *s = fargs[0];

  while (*s) {
    switch (*s) {
    case 'h': /*
               * hilite
               */
      safe_str(ANSI_HILITE, buff, bufc);
      break;
    case 'i': /*
               * inverse
               */
      safe_str(ANSI_INVERSE, buff, bufc);
      break;
    case 'f': /*
               * flash
               */
      safe_str(ANSI_BLINK, buff, bufc);
      break;
    case 'u': /* underline */
      safe_str(ANSI_UNDER, buff, bufc);
      break;
    case 'n': /*
               * normal
               */
      safe_str(ANSI_NORMAL, buff, bufc);
      break;
    case 'x': /*
               * black fg
               */
      safe_str(ANSI_BLACK, buff, bufc);
      break;
    case 'r': /*
               * red fg
               */
      safe_str(ANSI_RED, buff, bufc);
      break;
    case 'g': /*
               * green fg
               */
      safe_str(ANSI_GREEN, buff, bufc);
      break;
    case 'y': /*
               * yellow fg
               */
      safe_str(ANSI_YELLOW, buff, bufc);
      break;
    case 'b': /*
               * blue fg
               */
      safe_str(ANSI_BLUE, buff, bufc);
      break;
    case 'm': /*
               * magenta fg
               */
      safe_str(ANSI_MAGENTA, buff, bufc);
      break;
    case 'c': /*
               * cyan fg
               */
      safe_str(ANSI_CYAN, buff, bufc);
      break;
    case 'w': /*
               * white fg
               */
      safe_str(ANSI_WHITE, buff, bufc);
      break;
    case 'X': /*
               * black bg
               */
      safe_str(ANSI_BBLACK, buff, bufc);
      break;
    case 'R': /*
               * red bg
               */
      safe_str(ANSI_BRED, buff, bufc);
      break;
    case 'G': /*
               * green bg
               */
      safe_str(ANSI_BGREEN, buff, bufc);
      break;
    case 'Y': /*
               * yellow bg
               */
      safe_str(ANSI_BYELLOW, buff, bufc);
      break;
    case 'B': /*
               * blue bg
               */
      safe_str(ANSI_BBLUE, buff, bufc);
      break;
    case 'M': /*
               * magenta bg
               */
      safe_str(ANSI_BMAGENTA, buff, bufc);
      break;
    case 'C': /*
               * cyan bg
               */
      safe_str(ANSI_BCYAN, buff, bufc);
      break;
    case 'W': /*
               * white bg
               */
      safe_str(ANSI_BWHITE, buff, bufc);
      break;
    default:
      break;
    }
    s++;
  }
  safe_str(fargs[1], buff, bufc);
  safe_str(ANSI_NORMAL, buff, bufc);
}

void fun_zone(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  DbRef it;

  if (!context->world->configuration->have_zones) {
    return;
  }
  it = match_thing(&context->command->match, player, fargs[0]);
  if (it == NOTHING || !is_examinable(context, player, it)) {
    safe_str("#-1", buff, bufc);
    return;
  }
  safe_tprintf_str(buff, bufc, "#%ld",
                   game_object_zone(context->world->database, it));
}

void fun_link(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  CommandInvocation invocation = {.context = context->command,
                                  .player = player,
                                  .cause = cause,
                                  .first = fargs[0],
                                  .second = fargs[1]};

  do_link(&invocation);
}

void fun_tel(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
             int nfargs, char *cargs[], int ncargs,
             EvaluationContext *context) {
  CommandInvocation invocation = {.context = context->command,
                                  .player = player,
                                  .cause = cause,
                                  .first = fargs[0],
                                  .second = fargs[1]};

  do_teleport(&invocation);
}

void fun_pemit(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context) {
  do_pemit_list(context, context->world->configuration, player, fargs[0],
                fargs[1]);
}

/*------------------------------------------------------------------------
 * fun_create: Creates a room, thing or exit
 */

static int check_command(GameDatabase *database,
                         const ServerConfiguration *configuration,
                         CommandRegistry *registry, DbRef player,
                         const char *name, char *buff, char **bufc) {
  CMDENT *cmd;

  if ((cmd = (CMDENT *)hash_table_find(name, &registry->commands)))
    if (!check_access(database, configuration, player, cmd->perms)) {
      safe_str("#-1 PERMISSION DENIED", buff, bufc);
      return (1);
    }
  return (0);
}

void fun_create(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  DbRef thing;
  char sep, *name;

  varargs_preamble("CREATE", 2);
  name = fargs[0];

  if (!name || !*name) {
    safe_str("#-1 ILLEGAL NAME", buff, bufc);
    return;
  }
  if (fargs[2] && *fargs[2])
    sep = *fargs[2];
  else
    sep = 't';

  switch (sep) {
  case 'r':
    if (check_command(context->world->database, context->world->configuration,
                      context->runtime->command_registry, player, "@dig", buff,
                      bufc)) {
      safe_str("#-1 PERMISSION DENIED", buff, bufc);
      return;
    }
    thing = create_obj(context, player, TYPE_ROOM, name);
    break;
  case 'e':
    if (check_command(context->world->database, context->world->configuration,
                      context->runtime->command_registry, player, "@open", buff,
                      bufc)) {
      safe_str("#-1 PERMISSION DENIED", buff, bufc);
      return;
    }
    thing = create_obj(context, player, TYPE_EXIT, name);
    if (thing != NOTHING) {
      game_object_set_exits(context->world->database, thing, player);
      game_object_set_next(context->world->database, thing,
                           game_object_exits(context->world->database, player));
      game_object_set_exits(context->world->database, player, thing);
    }
    break;
  default:
    if (check_command(context->world->database, context->world->configuration,
                      context->runtime->command_registry, player, "@create",
                      buff, bufc)) {
      safe_str("#-1 PERMISSION DENIED", buff, bufc);
      return;
    }
    thing = create_obj(context, player, TYPE_THING, name);
    if (thing != NOTHING) {
      move_via_generic(context, thing, player, NOTHING, 0);
      game_object_set_link(context->world->database, thing,
                           new_home(context, player));
    }
    break;
  }
  safe_tprintf_str(buff, bufc, "#%ld", thing);
}

/*---------------------------------------------------------------------------
 * fun_set: sets an attribute on an object
 */

/*
 * Code for encrypt() and decrypt() was taken from the DarkZone server
 */

/*
 * Copy over only alphanumeric chars
 */
static char *crunch_code(char *code) {
  char *in;
  char *out;
  static char output[LBUF_SIZE];

  out = output;
  in = code;
  while (*in) {
    if ((*in >= 32) || (*in <= 126)) {
      printf("%c", *in);
      *out++ = *in;
    }
    in++;
  }
  *out = '\0';
  return (output);
}

static char *crypt_code(char *code, char *text, int type) {
  static char textbuff[LBUF_SIZE];
  char codebuff[LBUF_SIZE];
  int start = 32;
  int end = 126;
  int mod = end - start + 1;
  char *p, *q, *r;

  /* This function's other paths return the mutable textbuff above; the
     return type can't be const. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  if (!text || !*text)
    return (char *)"";
#pragma clang diagnostic pop
  if (!code || !*code)
    return text;
  StringCopy(codebuff, crunch_code(code));
  if (!*codebuff)
    return text;
  textbuff[0] = '\0';

  p = text;
  q = codebuff;
  r = textbuff;
  /*
   * Encryption: Simply go through each character of the text, get its
   * * * * ascii value, subtract start, add the ascii value (less
   * start) * of * * the code, mod the result, add start. Continue
   */
  while (*p) {
    if ((*p < start) || (*p > end)) {
      p++;
      continue;
    }
    if (type)
      *r++ = (char)((((*p++ - start) + (*q++ - start)) % mod) + start);
    else
      *r++ = (char)((((*p++ - *q++) + 2 * mod) % mod) + start);
    if (!*q)
      q = codebuff;
  }
  *r = '\0';
  return (textbuff);
}

/*
 * Borrowed from DarkZone
 */
void fun_zwho(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);
  DbRef i;
  size_t len = 0;

  if (!context->world->configuration->have_zones ||
      (!is_controls(context, player, it) &&
       !is_wizard(context->world->database, player))) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  for (i = 0; i < context->world->database->top; i++)
    if (typeof_obj(context->world->database, i) == TYPE_PLAYER) {
      if (game_object_zone(context->world->database, i) == it) {
        if (len) {
          static char smbuf[SBUF_SIZE];

          snprintf(smbuf, sizeof(smbuf), " #%ld", i);
          if ((strlen(smbuf) + len) > (LBUF_SIZE - SBUF_SIZE)) {
            safe_str(" #-1", buff, bufc);
            return;
          }
          safe_str(smbuf, buff, bufc);
          len += strlen(smbuf);
        } else {
          safe_tprintf_str(buff, bufc, "#%ld", i);
          len = strlen(buff);
        }
      }
    }
}

void fun_zrooms(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);
  DbRef i;
  size_t len = 0;

  if (!context->world->configuration->have_zones ||
      (!is_controls(context, player, it) &&
       !is_wizard(context->world->database, player))) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  for (i = 0; i < context->world->database->top; i++)
    if (typeof_obj(context->world->database, i) == TYPE_ROOM) {
      if (game_object_zone(context->world->database, i) == it) {
        if (len) {
          static char smbuf[SBUF_SIZE];

          snprintf(smbuf, sizeof(smbuf), " #%ld", i);
          if ((strlen(smbuf) + len) > (LBUF_SIZE - SBUF_SIZE)) {
            safe_str(" #-1", buff, bufc);
            return;
          }
          safe_str(smbuf, buff, bufc);
          len += strlen(smbuf);
        } else {
          safe_tprintf_str(buff, bufc, "#%ld", i);
          len = strlen(buff);
        }
      }
    }
}

void fun_zexits(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);
  DbRef i;
  size_t len = 0;

  if (!context->world->configuration->have_zones ||
      (!is_controls(context, player, it) &&
       !is_wizard(context->world->database, player))) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  for (i = 0; i < context->world->database->top; i++)
    if (typeof_obj(context->world->database, i) == TYPE_EXIT) {
      if (game_object_zone(context->world->database, i) == it) {
        if (len) {
          static char smbuf[SBUF_SIZE];

          snprintf(smbuf, sizeof(smbuf), " #%ld", i);
          if ((strlen(smbuf) + len) > (LBUF_SIZE - SBUF_SIZE)) {
            safe_str(" #-1", buff, bufc);
            return;
          }
          safe_str(smbuf, buff, bufc);
          len += strlen(smbuf);
        } else {
          safe_tprintf_str(buff, bufc, "#%ld", i);
          len = strlen(buff);
        }
      }
    }
}

void fun_zobjects(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);
  DbRef i;
  size_t len = 0;

  if (!context->world->configuration->have_zones ||
      (!is_controls(context, player, it) &&
       !is_wizard(context->world->database, player))) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  for (i = 0; i < context->world->database->top; i++)
    if (typeof_obj(context->world->database, i) == TYPE_THING) {
      if (game_object_zone(context->world->database, i) == it) {
        if (len) {
          static char smbuf[SBUF_SIZE];

          snprintf(smbuf, SBUF_SIZE, " #%ld", i);
          if ((strlen(smbuf) + len) > (LBUF_SIZE - SBUF_SIZE)) {
            safe_str(" #-1", buff, bufc);
            return;
          }
          safe_str(smbuf, buff, bufc);
          len += strlen(smbuf);
        } else {
          safe_tprintf_str(buff, bufc, "#%ld", i);
          len = strlen(buff);
        }
      }
    }
}

void fun_zplayers(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);
  DbRef i;
  size_t len = 0;

  if (!context->world->configuration->have_zones ||
      (!is_controls(context, player, it) &&
       !is_wizard(context->world->database, player))) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  for (i = 0; i < context->world->database->top; i++)
    if (typeof_obj(context->world->database, i) == TYPE_PLAYER)
      if (game_object_zone(context->world->database, i) == it) {
        if (!(is_connected(context->world->database, i)))
          continue;
        if (len) {
          static char smbuf[SBUF_SIZE];

          snprintf(smbuf, sizeof(smbuf), " #%ld", i);
          if ((strlen(smbuf) + len) > (LBUF_SIZE - SBUF_SIZE)) {
            safe_str(" #-1", buff, bufc);
            return;
          }
          safe_str(smbuf, buff, bufc);
          len += strlen(smbuf);
        } else {
          safe_tprintf_str(buff, bufc, "#%ld", i);
          len = strlen(buff);
        }
      }
}

/*
 * Borrowed from DarkZone
 */
void fun_inzone(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);
  DbRef i;
  size_t len = 0;

  if (!context->world->configuration->have_zones ||
      !(is_controls(context, player, it) ||
        !is_wizard(context->world->database, player))) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  for (i = 0; i < context->world->database->top; i++)
    if (typeof_obj(context->world->database, i) == TYPE_ROOM)
      if (game_object_zone(context->world->database, i) == it) {
        if (len) {
          static char smbuf[SBUF_SIZE];

          snprintf(smbuf, SBUF_SIZE, " #%ld", i);
          if ((strlen(smbuf) + len) > (LBUF_SIZE - SBUF_SIZE)) {
            safe_str(" #-1", buff, bufc);
            return;
          }
          safe_str(smbuf, buff, bufc);
          len += strlen(smbuf);
        } else {
          safe_tprintf_str(buff, bufc, "#%ld", i);
          len = strlen(buff);
        }
      }
}

void fun_encrypt(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  safe_str(crypt_code(fargs[1], fargs[0], 1), buff, bufc);
}

void fun_decrypt(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  safe_str(crypt_code(fargs[1], fargs[0], 0), buff, bufc);
}

void fun_objeval(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  DbRef obj;
  char *name, *bp, *str;

  if (!*fargs[0]) {
    return;
  }
  name = bp = alloc_lbuf("fun_objeval");
  str = fargs[0];
  exec(context, name, &bp, 0, player, cause, EV_FCHECK | EV_STRIP | EV_EVAL,
       &str, cargs, ncargs);
  *bp = '\0';
  obj = match_thing(&context->command->match, player, name);

  if ((obj == NOTHING) ||
      ((game_object_owner(context->world->database, obj) != player) &&
       (!(is_wizard(context->world->database, player)))) ||
      (obj == GOD))
    obj = player;

  str = fargs[1];
  exec(context, buff, bufc, 0, obj, obj, EV_FCHECK | EV_STRIP | EV_EVAL, &str,
       cargs, ncargs);
  free_lbuf(name);
}

void fun_squish(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  char *p, *q, *bp;
  bp = alloc_lbuf("fun_squish");
  StringCopy(bp, fargs[0]);
  p = q = bp;
  while (*p) {
    while (*p && (*p != ' '))
      *q++ = *p++;
    while (*p && (*p == ' '))
      p++;
    if (*p)
      *q++ = ' ';
  }
  *q = '\0';

  safe_str(bp, buff, bufc);
  free_lbuf(bp);
}

void fun_stripansi(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  char new[LBUF_SIZE];

  strncpy(new, fargs[0], LBUF_SIZE - 1);
  safe_str((char *)strip_ansi_r(new, fargs[0], strlen(fargs[0])), buff, bufc);
}

/*
 * Borrowed from PennMUSH 1.50
 */

void fun_columns(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  int spaces, number, ansinumber, count, i;
  static char buf[LBUF_SIZE];
  char *p, *q;
  int isansi = 0, rturn = 1;
  char *curr, *objstring, *bp, *cp, sep, *str;
  char new[MBUF_SIZE];

  evarargs_preamble("COLUMNS", 3);

  number = atoi(fargs[1]);
  if ((number < 1) || (number > 78)) {
    safe_str("#-1 OUT OF RANGE", buff, bufc);
    return;
  }
  cp = curr = bp = alloc_lbuf("fun_columns");
  str = fargs[0];
  exec(context, curr, &bp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
       &str, cargs, ncargs);
  *bp = '\0';
  cp = trim_space_sep(cp, sep);
  if (!*cp) {
    free_lbuf(curr);
    return;
  }
  safe_chr(' ', buff, bufc);

  while (cp) {
    objstring = split_token(&cp, sep);
    strncpy(new, objstring, MBUF_SIZE - 1);
    ansinumber = number;
    if ((size_t)ansinumber >
        strlen((char *)strip_ansi_r(new, objstring, strlen(objstring))))
      ansinumber =
          (int)strlen((char *)strip_ansi_r(new, objstring, strlen(objstring)));

    p = objstring;
    q = buf;
    count = 0;
    while (p && *p) {
      if (count == number) {
        break;
      }
      if (*p == ESC_CHAR) {
        /*
         * Start of ANSI code. Skip to end.
         */
        isansi = 1;
        while (*p && !isalpha(*p))
          *q++ = *p++;
        if (*p)
          *q++ = *p++;
      } else {
        *q++ = *p++;
        count++;
      }
    }
    if (isansi)
      safe_str(ANSI_NORMAL, buf, &q);
    *q = '\0';
    isansi = 0;

    spaces = number - (int)strlen((char *)strip_ansi_r(new, objstring,
                                                       strlen(objstring)));

    /*
     * Sanitize number of spaces
     */

    if (spaces > LBUF_SIZE) {
      spaces = LBUF_SIZE;
    }
    safe_str(buf, buff, bufc);
    for (i = 0; i < spaces; i++)
      safe_chr(' ', buff, bufc);

    if (!(rturn % (int)(78 / number)))
      safe_str("\r\n ", buff, bufc);

    rturn++;
  }
  free_lbuf(curr);
}

/*
 * Code for objmem and playmem borrowed from PennMUSH 1.50
 */
static int mem_usage(GameDatabase *database, DbRef thing) {
  GameObject *object = game_database_object(database, thing);
  int bytes =
      (int)sizeof(*object) + (int)strlen(game_object_name(database, thing)) + 1;
  for (int index = 0; index < object->at_count; index++)
    bytes += (int)strlen(object->ahead[index].name) + object->ahead[index].size;
  return bytes;
}

void fun_objmem(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  DbRef thing;

  thing = match_thing(&context->command->match, player, fargs[0]);
  if (thing == NOTHING || !is_examinable(context, player, thing)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  safe_tprintf_str(buff, bufc, "%d",
                   mem_usage(context->world->database, thing));
}

void fun_playmem(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  int tot = 0;
  DbRef thing;
  DbRef j;

  thing = match_thing(&context->command->match, player, fargs[0]);
  if (thing == NOTHING || !is_examinable(context, player, thing)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  DO_WHOLE_DB(context->world->database, j)
  if (game_object_owner(context->world->database, j) == thing)
    tot += mem_usage(context->world->database, j);
  safe_tprintf_str(buff, bufc, "%d", tot);
}

/*
 * Code for andflags() and orflags() borrowed from PennMUSH 1.50.
 * type = 0 for orflags, 1 for andflags.
 */
static int handle_flaglists(EvaluationContext *context, DbRef player,
                            char *name, char *fstr, int type) {
  char *s;
  char flagletter[2];
  FLAGSET fset;
  Flag p_type;
  int negate, temp;
  int ret = type;
  DbRef it = match_thing(&context->command->match, player, name);

  negate = temp = 0;

  if (it == NOTHING)
    return 0;

  for (s = fstr; *s; s++) {

    /*
     * Check for a negation sign. If we find it, we note it and
     * * * * * increment the pointer to the next character.
     */

    if (*s == '!') {
      negate = 1;
      s++;
    } else {
      negate = 0;
    }

    if (!*s) {
      return 0;
    }
    flagletter[0] = *s;
    flagletter[1] = '\0';

    if (!convert_flags(context, player, flagletter, &fset, &p_type)) {

      /*
       * Either we got a '!' that wasn't followed by a * *
       * * letter, or * we couldn't find that flag. For
       * AND, * * * since we've failed * a check, we can
       * return * * false.  * Otherwise we just go on.
       */

      if (type == 1)
        return 0;
      else
        continue;

    } else {

      /*
       * does the object have this flag?
       */

      if ((game_object_flags(context->world->database, it) & fset.word1) ||
          (game_object_flags2(context->world->database, it) & fset.word2) ||
          (typeof_obj(context->world->database, it) == p_type)) {
        if (is_player(context->world->database, it) &&
            (fset.word2 == CONNECTED) &&
            ((game_object_flags(context->world->database, it) &
              (WIZARD | DARK)) == (WIZARD | DARK)) &&
            !is_wizard(context->world->database, player))
          temp = 0;
        else
          temp = 1;
      } else {
        temp = 0;
      }

      if ((type == 1) && ((negate && temp) || (!negate && !temp))) {

        /*
         * Too bad there's no NXOR function... * At *
         *
         * *  * * this point we've either got a flag
         * and * we * * don't want * it, or we don't
         * have a  * flag * * and we want it. Since
         * it's * AND,  * we * * return false.
         */
        return 0;

      } else if ((type == 0) && ((!negate && temp) || (negate && !temp))) {

        /*
         * We've found something we want, in an OR. *
         *
         * *  * * We OR a * true with the current
         * value.
         */

        ret |= 1;
      }
      /*
       * Otherwise, we don't need to do anything.
       */
    }
  }
  return (ret);
}

void fun_orflags(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%d",
                   handle_flaglists(context, player, fargs[0], fargs[1], 0));
}

void fun_andflags(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  safe_tprintf_str(buff, bufc, "%d",
                   handle_flaglists(context, player, fargs[0], fargs[1], 1));
}

void fun_strtrunc(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  int number, count = 0;
  static char buf[LBUF_SIZE];
  char *p = (char *)fargs[0];
  char *q = buf;
  int isansi = 0;
  char new[LBUF_SIZE];

  number = atoi(fargs[1]);
  strncpy(new, fargs[0], LBUF_SIZE - 1);
  if ((size_t)number >
      strlen((char *)strip_ansi_r(new, fargs[0], strlen(fargs[0]))))
    number = (int)strlen((char *)strip_ansi_r(new, fargs[0], strlen(fargs[0])));

  if (number < 0) {
    safe_str("#-1 OUT OF RANGE", buff, bufc);
    return;
  }
  while (p && *p) {
    if (count == number) {
      break;
    }
    if (*p == ESC_CHAR) {
      /*
       * Start of ANSI code. Skip to end.
       */
      isansi = 1;
      while (*p && !isalpha(*p))
        *q++ = *p++;
      if (*p)
        *q++ = *p++;
    } else {
      *q++ = *p++;
      count++;
    }
  }
  if (isansi)
    safe_str(ANSI_NORMAL, buf, &q);
  *q = '\0';
  safe_str(buf, buff, bufc);
}

void fun_ifelse(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  /* This function now assumes that its arguments have not been
     evaluated. */

  char *str, *mbuff, *bp;

  mbuff = bp = alloc_lbuf("fun_ifelse");
  str = fargs[0];
  exec(context, mbuff, &bp, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
       &str, cargs, ncargs);
  *bp = '\0';

  if (!xlate(mbuff)) {
    str = fargs[2];
    exec(context, buff, bufc, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
         &str, cargs, ncargs);
  } else {
    str = fargs[1];
    exec(context, buff, bufc, 0, player, cause, EV_STRIP | EV_FCHECK | EV_EVAL,
         &str, cargs, ncargs);
  }
  free_lbuf(mbuff);
}

void fun_inc(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
             int nfargs, char *cargs[], int ncargs,
             EvaluationContext *context) {
  int number;

  if (!is_number(fargs[0])) {
    safe_str("#-1 ARGUMENT MUST BE A NUMBER", buff, bufc);
    return;
  }
  number = atoi(fargs[0]);
  safe_tprintf_str(buff, bufc, "%d", (++number));
}

void fun_dec(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
             int nfargs, char *cargs[], int ncargs,
             EvaluationContext *context) {
  int number;

  if (!is_number(fargs[0])) {
    safe_str("#-1 ARGUMENT MUST BE A NUMBER", buff, bufc);
    return;
  }
  number = atoi(fargs[0]);
  safe_tprintf_str(buff, bufc, "%d", (--number));
}

/*
 * ---------------------------------------------------------------------------
 * * fun_hasattr: does object X have attribute Y.
 */

/*
 * Hasattr (and hasattrp, which is derived from hasattr) borrowed from
 * * TinyMUSH 2.2.
 */

void fun_hasattr(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  DbRef thing, aowner;
  long aflags;
  Attribute *attr;
  char *tbuf;

  thing = match_thing(&context->command->match, player, fargs[0]);
  if (thing == NOTHING) {
    safe_str("#-1 NO MATCH", buff, bufc);
    return;
  } else if (!is_examinable(context, player, thing)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  attr = attribute_by_name(context->world->database, fargs[1]);
  if (!attr) {
    safe_str("0", buff, bufc);
    return;
  }
  attribute_get_info(context->world->database, thing, attr->number, &aowner,
                     &aflags);
  if (!see_attr(context, player, thing, attr, aowner, aflags))
    safe_str("0", buff, bufc);
  else {
    tbuf = attribute_get(context->world->database, thing, attr->number, &aowner,
                         &aflags);
    if (*tbuf)
      safe_str("1", buff, bufc);
    else
      safe_str("0", buff, bufc);
    free_lbuf(tbuf);
  }
}

void fun_hasattrp(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  DbRef thing, aowner;
  long aflags;
  Attribute *attr;
  char *tbuf;

  thing = match_thing(&context->command->match, player, fargs[0]);
  if (thing == NOTHING) {
    safe_str("#-1 NO MATCH", buff, bufc);
    return;
  } else if (!is_examinable(context, player, thing)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  attr = attribute_by_name(context->world->database, fargs[1]);
  if (!attr) {
    safe_str("0", buff, bufc);
    return;
  }
  attribute_get_info(context->world->database, thing, attr->number, &aowner,
                     &aflags);
  if (!see_attr(context, player, thing, attr, aowner, aflags))
    safe_str("0", buff, bufc);
  else {
    tbuf = attribute_get(context->world->database, thing, attr->number, &aowner,
                         &aflags);
    if (*tbuf)
      safe_str("1", buff, bufc);
    else
      safe_str("0", buff, bufc);
    free_lbuf(tbuf);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fun_default, fun_edefault, and fun_udefault:
 * * These check for the presence of an attribute. If it exists, then it
 * * is gotten, via the equivalent of get(), get_eval(), or u(), respectively.
 * * Otherwise, the default message is used.
 * * In the case of udefault(), the remaining arguments to the function
 * * are used as arguments to the u().
 */

/*
 * default(), edefault(), and udefault() borrowed from TinyMUSH 2.2
 */

/*
 * ---------------------------------------------------------------------------
 * * fun_findable: can X locate Y
 */

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_findable(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  DbRef obj = match_thing(&context->command->match, player, fargs[0]);
  DbRef victim = match_thing(&context->command->match, player, fargs[1]);

  if (obj == NOTHING)
    safe_str("#-1 ARG1 NOT FOUND", buff, bufc);
  else if (victim == NOTHING)
    safe_str("#-1 ARG2 NOT FOUND", buff, bufc);
  else
    safe_tprintf_str(
        buff, bufc, "%d",
        locatable(context, context->world->configuration, obj, victim, obj));
}

/*
 * ---------------------------------------------------------------------------
 * * isword: is every character in the argument a letter?
 */

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_isword(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  char *p;

  for (p = fargs[0]; *p; p++) {
    if (!isalpha(*p)) {
      safe_str("0", buff, bufc);
      return;
    }
  }
  safe_str("1", buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_visible:  Can X examine Y. If X does not exist, 0 is returned.
 * *               If Y, the object, does not exist, 0 is returned. If
 * *               Y the object exists, but the optional attribute does
 * *               not, X's ability to return Y the object is returned.
 */

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_visible(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  DbRef it, thing;

  if ((it = match_thing(&context->command->match, player, fargs[0])) ==
      NOTHING) {
    safe_str("0", buff, bufc);
    return;
  }
  thing = match_thing(&context->command->match, player, fargs[1]);
  if (!is_good_obj(context->world->database, thing)) {
    safe_str("0", buff, bufc);
    return;
  }
  safe_tprintf_str(buff, bufc, "%d", is_examinable(context, it, thing));
}

/*
 * ---------------------------------------------------------------------------
 * * fun_elements: given a list of numbers, get corresponding elements from
 * * the list.  elements(ack bar eep foof yay,2 4) ==> bar foof
 * * The function takes a separator, but the separator only applies to the
 * * first list.
 */

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_elements(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  int nwords, cur;
  char *ptrs[LBUF_SIZE / 2];
  char *wordlist, *s, *r, sep, *oldp;

  varargs_preamble("ELEMENTS", 3);
  oldp = *bufc;

  /*
   * Turn the first list into an array.
   */

  wordlist = alloc_lbuf("fun_elements.wordlist");
  StringCopy(wordlist, fargs[0]);
  nwords = list2arr(ptrs, LBUF_SIZE / 2, wordlist, sep);

  s = trim_space_sep(fargs[1], ' ');

  /*
   * Go through the second list, grabbing the numbers and finding the *
   *
   * *  * *  * * corresponding elements.
   */

  do {
    r = split_token(&s, ' ');
    cur = atoi(r) - 1;
    if ((cur >= 0) && (cur < nwords) && ptrs[cur]) {
      if (oldp != *bufc)
        safe_chr(sep, buff, bufc);
      safe_str(ptrs[cur], buff, bufc);
    }
  } while (s);
  free_lbuf(wordlist);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_grab: a combination of extract() and match(), sortof. We grab the
 * *           single element that we match.
 * *
 * *   grab(Test:1 Ack:2 Foof:3,*:2)    => Ack:2
 * *   grab(Test-1+Ack-2+Foof-3,*o*,+)  => Ack:2
 */

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_grab(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  char *r, *s, sep;

  varargs_preamble("GRAB", 3);

  /*
   * Walk the wordstring, until we find the word we want.
   */

  s = trim_space_sep(fargs[0], sep);
  do {
    r = split_token(&s, sep);
    if (quick_wild(fargs[1], r)) {
      safe_str(r, buff, bufc);
      return;
    }
  } while (s);
}

/* Same as grab, but return all matches */
void fun_graball(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  char *r, *s, sep, *b;

  varargs_preamble("GRABALL", 3);

  b = *bufc;
  s = trim_space_sep(fargs[0], sep);
  do {
    r = split_token(&s, sep);
    if (quick_wild(fargs[1], r)) {
      if (*bufc != b)
        safe_str(" ", buff, bufc);
      safe_str(r, buff, bufc);
    }
  } while (s);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_scramble:  randomizes the letters in a string.
 */

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_scramble(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  int n, i, j;
  char c, *old;

  if (!fargs[0] || !*fargs[0]) {
    return;
  }
  old = *bufc;

  safe_str(fargs[0], buff, bufc);
  **bufc = '\0';

  n = (int)strlen(old);

  for (i = 0; i < n; i++) {
    j = (int)(random() % (n - i)) + i;
    c = old[i];
    old[i] = old[j];
    old[j] = c;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fun_shuffle: randomize order of words in a list.
 */

/*
 * Borrowed from PennMUSH 1.50
 */
static void swap(char **p, char **q) {
  /*
   * swaps two points to strings
   */

  char *temp;

  temp = *p;
  *p = *q;
  *q = temp;
}

void fun_shuffle(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  char *words[LBUF_SIZE];
  int n, i, j;
  char sep;

  if (!nfargs || !fargs[0] || !*fargs[0]) {
    return;
  }
  varargs_preamble("SHUFFLE", 2);

  n = list2arr(words, LBUF_SIZE, fargs[0], sep);

  for (i = 0; i < n; i++) {
    j = (int)(random() % (n - i)) + i;
    swap(&words[i], &words[j]);
  }
  arr2list(words, n, buff, bufc, sep);
}

/*
 * sortby() code borrowed from TinyMUSH 2.2
 */

typedef struct UserFunctionComparatorContext UserFunctionComparatorContext;
struct UserFunctionComparatorContext {
  const char *code;
  DbRef player;
  DbRef cause;
  EvaluationContext *evaluation;
};

/*
 * ---------------------------------------------------------------------------
 * * fun_last: Returns last word in a string
 */

/*
 * Borrowed from TinyMUSH 2.2
 */
void fun_last(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  char *s, *last, sep;
  int len, i;

  /*
   * If we are passed an empty arglist return a null string
   */

  if (nfargs == 0) {
    return;
  }
  varargs_preamble("LAST", 2);
  s = trim_space_sep(fargs[0], sep); /*
                                      * trim leading spaces
                                      */

  /*
   * If we're dealing with spaces, trim off the trailing stuff
   */

  if (sep == ' ') {
    len = (int)strlen(s);
    for (i = len - 1; s[i] == ' '; i--)
      ;
    if (i + 1 <= len)
      s[i + 1] = '\0';
  }
  last = (char *)rindex(s, sep);
  if (last)
    safe_str(++last, buff, bufc);
  else
    safe_str(s, buff, bufc);
}

/*
 * Borrowed from TinyMUSH 2.2
 */
void fun_matchall(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  int wcount;
  char *r, *s, *old, sep, tbuf[16];

  varargs_preamble("MATCHALL", 3);
  old = *bufc;

  /*
   * Check each word individually, returning the word number of all * *
   *
   * *  * * that match. If none match, return 0.
   */

  wcount = 1;
  s = trim_space_sep(fargs[0], sep);
  do {
    r = split_token(&s, sep);
    if (quick_wild(fargs[1], r)) {
      snprintf(tbuf, sizeof(tbuf), "%d", wcount);
      if (old != *bufc)
        safe_chr(' ', buff, bufc);
      safe_str(tbuf, buff, bufc);
    }
    wcount++;
  } while (s);

  if (*bufc == old)
    safe_str("0", buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_ports: Returns a list of ports for a user.
 */

/*
 * Borrowed from TinyMUSH 2.2
 */
void fun_ports(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context) {
  DbRef target;

  if (!is_wizard(context->world->database, player)) {
    return;
  }
  target = lookup_player(context->world, player, fargs[0], 1);
  if (!is_good_obj(context->world->database, target) ||
      !is_connected(context->world->database, target)) {
    return;
  }
  make_portlist(context->runtime->descriptors, player, target, buff, bufc);
}

/*
 * ---------------------------------------------------------------------------
 * * fun_mix: Like map, but operates on two lists simultaneously, passing
 * * the elements as %0 as %1.
 */

/*
 * Borrowed from PennMUSH 1.50
 */

/*
 * ---------------------------------------------------------------------------
 * * fun_foreach: like map(), but it operates on a string, rather than on a
 * list,
 * * calling a user-defined function for each character in the string.
 * * No delimiter is inserted between the results.
 */

/*
 * Borrowed from TinyMUSH 2.2
 */

/*
 * ---------------------------------------------------------------------------
 * * fun_munge: combines two lists in an arbitrary manner.
 */

/*
 * Borrowed from TinyMUSH 2.2
 */

/*
 * die() code borrowed from PennMUSH 1.50
 */
static int getrandom(int x) {
  /*
   * In order to be perfectly anal about not introducing any further *
   * * * sources * of statistical bias, we're going to call random() *
   * until * * we get a number * less than the greatest representable *
   * multiple * of  * x. We'll then return * n mod x.
   */
  long n;

  if (x <= 0)
    return -1;

  do {
    n = random();
  } while (LONG_MAX - n < x);

  /*
   * N.B. This loop happens in randomized constant time, and pretty damn
   * * fast randomized constant time too, since P(LONG_MAX - n < x) < 0.5
   * * for any x, so for any X, the average number of times we should
   * * have to call random() is less than 2.
   */
  return (int)(n % x);
}

void fun_die(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
             int nfargs, char *cargs[], int ncargs,
             EvaluationContext *context) {
  int n, die, count;
  int total = 0;

  if (!fargs[0] || !fargs[1])
    return;

  n = atoi(fargs[0]);
  die = atoi(fargs[1]);

  if ((n < 1) || (n > 20)) {
    safe_str("#-1 NUMBER OUT OF RANGE", buff, bufc);
    return;
  }
  if (die > 100) {
    safe_str("#-1 DON'T BE AN ASSHOLE", buff, bufc);
    return;
  }
  for (count = 0; count < n; count++)
    total += getrandom(die) + 1;

  safe_tprintf_str(buff, bufc, "%d", total);
}

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_lit(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
             int nfargs, char *cargs[], int ncargs,
             EvaluationContext *context) {
  /*
   * Just returns the argument, literally
   */
  safe_str(fargs[0], buff, bufc);
}

/*
 * shl() and shr() borrowed from PennMUSH 1.50
 */
void fun_shl(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
             int nfargs, char *cargs[], int ncargs,
             EvaluationContext *context) {
  if (is_number(fargs[0]) && is_number(fargs[1]))
    safe_tprintf_str(buff, bufc, "%d", atoi(fargs[0]) << atoi(fargs[1]));
  else
    safe_str("#-1 ARGUMENTS MUST BE NUMBERS", buff, bufc);
}

void fun_shr(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
             int nfargs, char *cargs[], int ncargs,
             EvaluationContext *context) {
  if (is_number(fargs[0]) && is_number(fargs[1]))
    safe_tprintf_str(buff, bufc, "%d", atoi(fargs[0]) >> atoi(fargs[1]));
  else
    safe_str("#-1 ARGUMENTS MUST BE NUMBERS", buff, bufc);
}

/*
 * ------------------------------------------------------------------------
 * * Vector functions: VADD, VSUB, VMUL, VCROSS, VMAG, VUNIT, VDIM
 * * Vectors are space-separated numbers.
 */

/*
 * Vector functions borrowed from PennMUSH 1.50
 */
#define MAXDIM 20

void fun_vadd(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  char *v1[LBUF_SIZE], *v2[LBUF_SIZE];
  char vres[MAXDIM][LBUF_SIZE];
  int n, m, i;
  char sep;

  varargs_preamble("VADD", 3);

  /*
   * split the list up, or return if the list is empty
   */
  if (!fargs[0] || !*fargs[0] || !fargs[1] || !*fargs[1]) {
    return;
  }
  n = list2arr(v1, LBUF_SIZE, fargs[0], sep);
  m = list2arr(v2, LBUF_SIZE, fargs[1], sep);

  if (n != m) {
    safe_str("#-1 VECTORS MUST BE SAME DIMENSIONS", buff, bufc);
    return;
  }
  if (n > MAXDIM) {
    safe_str("#-1 TOO MANY DIMENSIONS ON VECTORS", buff, bufc);
    return;
  }
  /*
   * add it
   */
  for (i = 0; i < n; i++) {
    snprintf(vres[i], LBUF_SIZE, "%f", atof(v1[i]) + atof(v2[i]));
    v1[i] = (char *)vres[i];
  }

  arr2list(v1, n, buff, bufc, sep);
}

void fun_vsub(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  char *v1[LBUF_SIZE], *v2[LBUF_SIZE];
  char vres[MAXDIM][LBUF_SIZE];
  int n, m, i;
  char sep;

  varargs_preamble("VSUB", 3);

  /*
   * split the list up, or return if the list is empty
   */
  if (!fargs[0] || !*fargs[0] || !fargs[1] || !*fargs[1]) {
    return;
  }
  n = list2arr(v1, LBUF_SIZE, fargs[0], sep);
  m = list2arr(v2, LBUF_SIZE, fargs[1], sep);

  if (n != m) {
    safe_str("#-1 VECTORS MUST BE SAME DIMENSIONS", buff, bufc);
    return;
  }
  if (n > MAXDIM) {
    safe_str("#-1 TOO MANY DIMENSIONS ON VECTORS", buff, bufc);
    return;
  }
  /*
   * sub it
   */
  for (i = 0; i < n; i++) {
    snprintf(vres[i], LBUF_SIZE, "%f", atof(v1[i]) - atof(v2[i]));
    v1[i] = (char *)vres[i];
  }

  arr2list(v1, n, buff, bufc, sep);
}

void fun_vmul(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  char *v1[LBUF_SIZE], *v2[LBUF_SIZE];
  char vres[MAXDIM][LBUF_SIZE];
  int n, m, i;
  double scalar;
  char sep;

  varargs_preamble("VMUL", 3);

  /*
   * split the list up, or return if the list is empty
   */
  if (!fargs[0] || !*fargs[0] || !fargs[1] || !*fargs[1]) {
    return;
  }
  n = list2arr(v1, LBUF_SIZE, fargs[0], sep);
  m = list2arr(v2, LBUF_SIZE, fargs[1], sep);

  if ((n != 1) && (m != 1) && (n != m)) {
    safe_str("#-1 VECTORS MUST BE SAME DIMENSIONS", buff, bufc);
    return;
  }
  if (n > MAXDIM) {
    safe_str("#-1 TOO MANY DIMENSIONS ON VECTORS", buff, bufc);
    return;
  }
  /*
   * multiply it - if n or m is 1, it's scalar multiplication by a * *
   * * vector, otherwise it's a dot-product
   */

  if (n == 1) {
    scalar = atof(v1[0]);
    for (i = 0; i < m; i++) {
      snprintf(vres[i], LBUF_SIZE, "%f", atof(v2[i]) * scalar);
      v1[i] = (char *)vres[i];
    }
    n = m;
  } else if (m == 1) {
    scalar = atof(v2[0]);
    for (i = 0; i < n; i++) {
      snprintf(vres[i], LBUF_SIZE, "%f", atof(v1[i]) * scalar);
      v1[i] = (char *)vres[i];
    }
  } else {
    /*
     * dot product
     */
    scalar = 0;
    for (i = 0; i < n; i++) {
      scalar += atof(v1[i]) * atof(v2[i]);
      v1[i] = (char *)vres[i];
    }

    safe_tprintf_str(buff, bufc, "%f", scalar);
    return;
  }

  arr2list(v1, n, buff, bufc, sep);
}

void fun_vmag(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  char *v1[LBUF_SIZE];
  int n, i;
  double tmp, res = 0;
  char sep;

  varargs_preamble("VMAG", 2);

  /*
   * split the list up, or return if the list is empty
   */
  if (!fargs[0] || !*fargs[0]) {
    return;
  }
  n = list2arr(v1, LBUF_SIZE, fargs[0], sep);

  if (n > MAXDIM) {
    StringCopy(buff, "#-1 TOO MANY DIMENSIONS ON VECTORS");
    return;
  }
  /*
   * calculate the magnitude
   */
  for (i = 0; i < n; i++) {
    tmp = atof(v1[i]);
    res += tmp * tmp;
  }

  if (res > 0)
    safe_tprintf_str(buff, bufc, "%f", sqrt(res));
  else
    safe_str("0", buff, bufc);
}

void fun_vunit(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context) {
  char *v1[LBUF_SIZE];
  char vres[MAXDIM][LBUF_SIZE];
  int n, i;
  double tmp, res = 0;
  char sep;

  varargs_preamble("VUNIT", 2);

  /*
   * split the list up, or return if the list is empty
   */
  if (!fargs[0] || !*fargs[0]) {
    return;
  }
  n = list2arr(v1, LBUF_SIZE, fargs[0], sep);

  if (n > MAXDIM) {
    StringCopy(buff, "#-1 TOO MANY DIMENSIONS ON VECTORS");
    return;
  }
  /*
   * calculate the magnitude
   */
  for (i = 0; i < n; i++) {
    tmp = atof(v1[i]);
    res += tmp * tmp;
  }

  if (res <= 0) {
    safe_str("#-1 CAN'T MAKE UNIT VECTOR FROM ZERO-LENGTH VECTOR", buff, bufc);
    return;
  }
  for (i = 0; i < n; i++) {
    snprintf(vres[i], LBUF_SIZE, "%f", atof(v1[i]) / sqrt(res));
    v1[i] = (char *)vres[i];
  }

  arr2list(v1, n, buff, bufc, sep);
}

void fun_vdim(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  char sep;

  if (fargs == 0)
    safe_str("0", buff, bufc);
  else {
    varargs_preamble("VDIM", 2);
    safe_tprintf_str(buff, bufc, "%d", countwords(fargs[0], sep));
  }
}

void fun_strcat(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  int i;

  safe_str(fargs[0], buff, bufc);
  for (i = 1; i < nfargs; i++) {
    safe_str(fargs[i], buff, bufc);
  }
}

/*
 * grep() and grepi() code borrowed from PennMUSH 1.50
 */

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_art(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
             int nfargs, char *cargs[], int ncargs,
             EvaluationContext *context) {

  /*
   * checks a word and returns the appropriate article, "a" or "an"
   */
  char c = (char)tolower(*fargs[0]);

  if (c == 'a' || c == 'e' || c == 'i' || c == 'o' || c == 'u')
    safe_str("an", buff, bufc);
  else
    safe_str("a", buff, bufc);
}

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_alphamax(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  char *amax;
  int i = 1;

  if (!fargs[0]) {
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
    return;
  } else
    amax = fargs[0];

  while ((i < 10) && fargs[i]) {
    amax = (strcmp(amax, fargs[i]) > 0) ? amax : fargs[i];
    i++;
  }

  safe_tprintf_str(buff, bufc, "%s", amax);
}

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_alphamin(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  char *amin;
  int i = 1;

  if (!fargs[0]) {
    safe_str("#-1 TOO FEW ARGUMENTS", buff, bufc);
    return;
  } else
    amin = fargs[0];

  while ((i < 10) && fargs[i]) {
    amin = (strcmp(amin, fargs[i]) < 0) ? amin : fargs[i];
    i++;
  }

  safe_tprintf_str(buff, bufc, "%s", amin);
}

/*
 * Borrowed from PennMUSH 1.50
 */

void fun_valid(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context) {

  /*
   * Checks to see if a given <something> is valid as a parameter of a
   * * given type (such as an object name).
   */

  if (!fargs[0] || !*fargs[0] || !fargs[1] || !*fargs[1])
    safe_str("0", buff, bufc);
  else if (!strcasecmp(fargs[0], "name"))
    safe_tprintf_str(buff, bufc, "%d",
                     ok_name(context->world->configuration, fargs[1]));
  else
    safe_str("#-1", buff, bufc);
}

/*
 * Borrowed from PennMUSH 1.50
 */
void fun_hastype(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);

  if (it == NOTHING) {
    safe_str("#-1 NO MATCH", buff, bufc);
    return;
  }
  if (!fargs[1] || !*fargs[1]) {
    safe_str("#-1 NO SUCH TYPE", buff, bufc);
    return;
  }
  switch (*fargs[1]) {
  case 'r':
  case 'R':
    safe_str((typeof_obj(context->world->database, it) == TYPE_ROOM) ? "1"
                                                                     : "0",
             buff, bufc);
    break;
  case 'e':
  case 'E':
    safe_str((typeof_obj(context->world->database, it) == TYPE_EXIT) ? "1"
                                                                     : "0",
             buff, bufc);
    break;
  case 'p':
  case 'P':
    safe_str((typeof_obj(context->world->database, it) == TYPE_PLAYER) ? "1"
                                                                       : "0",
             buff, bufc);
    break;
  case 't':
  case 'T':
    safe_str((typeof_obj(context->world->database, it) == TYPE_THING) ? "1"
                                                                      : "0",
             buff, bufc);
    break;
  default:
    safe_str("#-1 NO SUCH TYPE", buff, bufc);
    break;
  };
}

/* stacksize - returns how many items are stuffed onto an object stack */

static int stacksize(GameDatabase *database, DbRef doer) {
  int i;
  AttributeStack *sp;

  for (i = 0, sp = game_object_stack(database, doer); sp != nullptr;
       sp = sp->next, i++)
    ;

  return i;
}

void fun_lstack(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  AttributeStack *sp;
  DbRef doer;

  if (nfargs > 1) {
    safe_str("#-1 FUNCTION (CSTACK) EXPECTS 0-1 ARGUMENTS", buff, bufc);
    return;
  }
  if (!fargs[0]) {
    doer = player;
  } else {
    doer = match_thing(&context->command->match, player, fargs[0]);
  }

  if (!is_controls(context, player, doer)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  for (sp = game_object_stack(context->world->database, doer); sp != nullptr;
       sp = sp->next) {
    safe_str(sp->data, buff, bufc);
    safe_chr(' ', buff, bufc);
  }

  if (sp)
    (*bufc)--;
}

void fun_empty(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context) {
  AttributeStack *sp, *next;
  DbRef doer;

  if (nfargs > 1) {
    safe_str("#-1 FUNCTION (CSTACK) EXPECTS 0-1 ARGUMENTS", buff, bufc);
    return;
  }
  if (!fargs[0]) {
    doer = player;
  } else {
    doer = match_thing(&context->command->match, player, fargs[0]);
  }

  if (!is_controls(context, player, doer)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  for (sp = game_object_stack(context->world->database, doer); sp != nullptr;
       sp = next) {
    next = sp->next;
    free_lbuf(sp->data);
    free(sp);
  }

  game_object_set_stack(context->world->database, doer, nullptr);
}

void fun_items(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context) {
  DbRef doer;

  if (nfargs > 1) {
    safe_str("#-1 FUNCTION (NUMSTACK) EXPECTS 0-1 ARGUMENTS", buff, bufc);
    return;
  }
  if (!fargs[0]) {
    doer = player;
  } else {
    doer = match_thing(&context->command->match, player, fargs[0]);
  }

  if (!is_controls(context, player, doer)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  safe_tprintf_str(buff, bufc, "%d", stacksize(context->world->database, doer));
}

void fun_peek(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  AttributeStack *sp;
  DbRef doer;
  int count, pos;

  if (nfargs > 2) {
    safe_str("#-1 FUNCTION (PEEK) EXPECTS 0-2 ARGUMENTS", buff, bufc);
    return;
  }
  if (!fargs[0]) {
    doer = player;
  } else {
    doer = match_thing(&context->command->match, player, fargs[0]);
  }

  if (!is_controls(context, player, doer)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  if (!fargs[1] || !*fargs[1]) {
    pos = 0;
  } else {
    pos = atoi(fargs[1]);
  }

  if (stacksize(context->world->database, doer) == 0) {
    return;
  }
  if (pos > (stacksize(context->world->database, doer) - 1)) {
    safe_str("#-1 POSITION TOO LARGE", buff, bufc);
    return;
  }
  count = 0;
  sp = game_object_stack(context->world->database, doer);
  while (count != pos) {
    if (sp == nullptr) {
      return;
    }
    count++;
    sp = sp->next;
  }

  safe_str(sp->data, buff, bufc);
}

void fun_pop(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
             int nfargs, char *cargs[], int ncargs,
             EvaluationContext *context) {
  AttributeStack *sp, *prev = nullptr;
  DbRef doer;
  int count, pos;

  if (nfargs > 2) {
    safe_str("#-1 FUNCTION (POP) EXPECTS 0-2 ARGUMENTS", buff, bufc);
    return;
  }

  if (!fargs[0]) {
    doer = player;
  } else {
    doer = match_thing(&context->command->match, player, fargs[0]);
  }

  if (!is_controls(context, player, doer)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }

  if (!fargs[1] || !*fargs[1]) {
    pos = 0;
  } else {
    pos = atoi(fargs[1]);
  }

  sp = game_object_stack(context->world->database, doer);
  count = 0;

  if (stacksize(context->world->database, doer) == 0) {
    return;
  }

  if (pos > (stacksize(context->world->database, doer) - 1)) {
    safe_str("#-1 POSITION TOO LARGE", buff, bufc);
    return;
  }

  while (count != pos) {
    if (sp == nullptr) {
      return;
    }
    prev = sp;
    sp = sp->next;
    count++;
  }

  safe_str(sp->data, buff, bufc);
  if (count == 0) {
    game_object_set_stack(context->world->database, doer, sp->next);
    free_lbuf(sp->data);
    free(sp);
  } else {
    prev->next = sp->next;
    free_lbuf(sp->data);
    free(sp);
  }
}

void fun_push(char *buff, char **bufc, DbRef player, DbRef cause, char *fargs[],
              int nfargs, char *cargs[], int ncargs,
              EvaluationContext *context) {
  AttributeStack *sp;
  DbRef doer;
  char *data;

  if ((nfargs > 2) || (nfargs < 1)) {
    safe_str("#-1 FUNCTION (PUSH) EXPECTS 1-2 ARGUMENTS", buff, bufc);
    return;
  }
  if (!fargs[1]) {
    doer = player;
    data = fargs[0];
  } else {
    doer = match_thing(&context->command->match, player, fargs[0]);
    data = fargs[1];
  }

  if (!is_controls(context, player, doer)) {
    safe_str("#-1 PERMISSION DENIED", buff, bufc);
    return;
  }
  if (stacksize(context->world->database, doer) >=
      context->world->configuration->stack_limit) {
    safe_str("#-1 STACK SIZE EXCEEDED", buff, bufc);
    return;
  }
  sp = malloc(sizeof(AttributeStack));
  sp->next = game_object_stack(context->world->database, doer);
  sp->data = alloc_lbuf("push");
  StringCopy(sp->data, data);
  game_object_set_stack(context->world->database, doer, sp);
}

/* ---------------------------------------------------------------------------
 * fun_regmatch: Return 0 or 1 depending on whether or not a regular
 * expression matches a string. If a third argument is specified, dump
 * the results of a regexp pattern match into a set of arbitrary r()-registers.
 *
 * regmatch(string, pattern, list of registers)
 * If the number of matches exceeds the registers, those bits are tossed
 * out.
 * If -1 is specified as a register number, the matching bit is tossed.
 * Therefore, if the list is "-1 0 3 5", the regexp $0 is tossed, and
 * the regexp $1, $2, and $3 become r(0), r(3), and r(5), respectively.
 *
 */

void fun_regmatch(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  int i, nqregs, curq, len;
  char *qregs[10];
  regex_t re;
  int errcode;
  static char errbuf[LINE_MAX];
  int got_match;
  regmatch_t pmatch[NSUBEXP];

  if (!fn_range_check("REGMATCH", nfargs, 2, 3, buff, bufc))
    return;

  if ((errcode = regcomp(&re, fargs[1], REG_EXTENDED)) != 0) {
    /* Matching error. */
    regerror(errcode, &re, errbuf, LINE_MAX);
    notify_quiet(context, player, errbuf);
    safe_chr('0', buff, bufc);
    return;
  }

  got_match = (regexec(&re, fargs[0], NSUBEXP, pmatch, 0) == 0);
  if (got_match) {
    if (re.re_nsub > 0)
      safe_tprintf_str(buff, bufc, "%zu", re.re_nsub);
    else
      safe_tprintf_str(buff, bufc, "1");
  } else
    safe_tprintf_str(buff, bufc, "0");

  /* If we don't have a third argument, we're done. */
  if (nfargs != 3) {
    regfree(&re);
    return;
  }

  /* We need to parse the list of registers. Anything that we don't get is
   * assumed to be -1.
   */
  nqregs = list2arr(qregs, 10, fargs[2], ' ');
  for (i = 0; i < nqregs; i++) {
    if (qregs[i] && *qregs[i] && isdigit(*qregs[i]))
      curq = atoi(qregs[i]);
    else
      continue;
    if (curq < 0 || curq > 9)
      continue;

    if (!context->registers[curq])
      context->registers[curq] = alloc_lbuf("fun_regmatch");

    if (!got_match || pmatch[i].rm_so == -1 || pmatch[i].rm_eo == -1) {
      context->registers[curq][0] = '\0';
      continue;
    }
    len = pmatch[i].rm_eo - pmatch[i].rm_so;
    if (len < 0)
      len = 0;
    if (len >= LBUF_SIZE)
      len = LBUF_SIZE - 1;
    strncpy(context->registers[curq], fargs[0] + pmatch[i].rm_so, (size_t)len);
    context->registers[curq][len] = '\0'; /* must null-terminate */
  }
  regfree(&re);
}

/* ---------------------------------------------------------------------------
 * fun_translate: Takes a string and a second argument. If the second argument
 * is 0 or s, control characters are converted to spaces. If it's 1 or p,
 * they're converted to percent substitutions.
 */

void fun_translate(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  int type = 0;

  if (fargs[0] && fargs[1]) {
    if (*fargs[1] && ((*fargs[1] == 's') || (*fargs[1] == '0')))
      type = 0;
    else if (*fargs[1] && ((*fargs[1] == 'p') || (*fargs[1] == '1')))
      type = 1;

    safe_str(translate_string(fargs[0], type), buff, bufc);
  }
}
