/*
 * eval.c - command evaluation and cracking
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/commands/eval.h"
#include "mux/commands/functions.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"

/**
 * Split a line at a character, obeying nesting. The line is
 * destructively modified (a null is inserted where the delimiter was found)
 * dstr is modified to point to the char after the delimiter, and the function
 * return value points to the found string (space compressed if specified).
 * If we ran off the end of the string without finding the delimiter, dstr is
 * returned as NULL.
 */

static char *parse_to_cleanup(const ServerConfiguration *configuration,
                              int eval, int first, char *cstr, char *rstr,
                              char *zstr) {
  if ((configuration->space_compress || (eval & EV_STRIP_TS)) &&
      !(eval & EV_NO_COMPRESS) && !first && (cstr[-1] == ' '))
    zstr--;
  if ((eval & EV_STRIP_AROUND) && (*rstr == '{') && (zstr[-1] == '}')) {
    rstr++;
    if (configuration->space_compress &&
        (!(eval & EV_NO_COMPRESS) || (eval & EV_STRIP_LS)))
      while (*rstr && isspace(*rstr))
        rstr++;
    rstr[-1] = '\0';
    zstr--;
    if (configuration->space_compress &&
        (!(eval & EV_NO_COMPRESS) || (eval & EV_STRIP_TS)))
      while (zstr[-1] && isspace(zstr[-1]))
        zstr--;
    *zstr = '\0';
  }
  *zstr = '\0';
  return rstr;
}

/* We can't change this to just '*zstr++ = *cstr++', because of the inherent
problems with copying a memory location to itself. */

#define NEXTCHAR                                                               \
  do {                                                                         \
    if (cstr == zstr) {                                                        \
      cstr++;                                                                  \
      zstr++;                                                                  \
    } else                                                                     \
      *zstr++ = *cstr++;                                                       \
  } while (0)

char *parse_to(const ServerConfiguration *configuration, char **dstr,
               char delim, int eval) {
#define stacklim 32
  char stack[stacklim];
  char *rstr, *cstr, *zstr;
  int sp, tp, first, bracketlev;

  if ((dstr == nullptr) || (*dstr == nullptr))
    return nullptr;
  if (**dstr == '\0') {
    rstr = *dstr;
    *dstr = nullptr;
    return rstr;
  }
  sp = 0;
  first = 1;
  rstr = *dstr;
  if ((configuration->space_compress || (eval & EV_STRIP_LS)) &&
      !(eval & EV_NO_COMPRESS)) {
    while (*rstr && isspace(*rstr))
      rstr++;
    *dstr = rstr;
  }
  zstr = cstr = rstr;
  while (*cstr) {
    switch (*cstr) {
    case '\\': /*
                * general escape
                */
    case '%':  /*
                * also escapes chars
                */
      if ((*cstr == '\\') && (eval & EV_STRIP_ESC))
        cstr++;
      else
        NEXTCHAR;
      if (*cstr)
        NEXTCHAR;
      first = 0;
      break;
    case ']':
    case ')':
      for (tp = sp - 1; (tp >= 0) && (stack[tp] != *cstr); tp--)
        ;

      /*
       * If we hit something on the stack, unwind to it
       * Otherwise (it's not on stack), if it's our
       * delim  we are done, and we convert the
       * delim to a null and return a ptr to the
       * char after the null. If it's not our
       * delimiter, skip over it normally
       */

      if (tp >= 0)
        sp = tp;
      else if (*cstr == delim) {
        rstr = parse_to_cleanup(configuration, eval, first, cstr, rstr, zstr);
        *dstr = ++cstr;
        return rstr;
      }
      first = 0;
      NEXTCHAR;
      break;
    case '{':
      bracketlev = 1;
      if (eval & EV_STRIP) {
        cstr++;
      } else {
        NEXTCHAR;
      }
      while (*cstr && (bracketlev > 0)) {
        switch (*cstr) {
        case '\\':
        case '%':
          if (cstr[1]) {
            if ((*cstr == '\\') && (eval & EV_STRIP_ESC))
              cstr++;
            else
              NEXTCHAR;
          }
          break;
        case '{':
          bracketlev++;
          break;
        case '}':
          bracketlev--;
          break;
        default:
          break;
        }
        if (bracketlev > 0) {
          NEXTCHAR;
        }
      }
      if ((eval & EV_STRIP) && (bracketlev == 0)) {
        cstr++;
      } else if (bracketlev == 0) {
        NEXTCHAR;
      }
      first = 0;
      break;
    default:
      if ((*cstr == delim) && (sp == 0)) {
        rstr = parse_to_cleanup(configuration, eval, first, cstr, rstr, zstr);
        *dstr = ++cstr;
        return rstr;
      }
      switch (*cstr) {
      case ' ': /*
                 * space
                 */
        if (configuration->space_compress && !(eval & EV_NO_COMPRESS)) {
          if (first)
            rstr++;
          else if (cstr[-1] == ' ')
            zstr--;
        }
        break;
      case '[':
        if (cstr != rstr && cstr[-1] == ESC_CHAR) {
          first = 0;
          break;
        }
        if (sp < stacklim)
          stack[sp++] = ']';
        first = 0;
        break;
      case '(':
        if (sp < stacklim)
          stack[sp++] = ')';
        first = 0;
        break;
      default:
        first = 0;
      }
      NEXTCHAR;
    }
  }
  rstr = parse_to_cleanup(configuration, eval, first, cstr, rstr, zstr);
  *dstr = nullptr;
  return rstr;
}

/*
 * ---------------------------------------------------------------------------
 * * parse_arglist: Parse a line into an argument list contained in lbufs.
 * * A pointer is returned to whatever follows the final delimiter.
 * * If the arglist is unterminated, a NULL is returned.  The original arglist
 * * is destructively modified.
 */

char *parse_arglist(EvaluationContext *context, DbRef player, DbRef cause,
                    char *dstr, char delim, DbRef eval, char *fargs[],
                    DbRef nfargs, char *cargs[], DbRef ncargs) {
  char *rstr, *tstr, *bp, *str;
  int arg, peval;

  for (arg = 0; arg < nfargs; arg++)
    fargs[arg] = nullptr;
  if (dstr == nullptr)
    return nullptr;
  rstr = parse_to(context->world->configuration, &dstr, delim, 0);
  arg = 0;

  peval = (int)(eval & ~EV_EVAL);

  while ((arg < nfargs) && rstr) {
    if (arg < (nfargs - 1))
      tstr = parse_to(context->world->configuration, &rstr, ',', peval);
    else
      tstr = parse_to(context->world->configuration, &rstr, '\0', peval);
    if (eval & EV_EVAL) {
      bp = fargs[arg] = alloc_lbuf("parse_arglist");
      str = tstr;
      exec(context, fargs[arg], &bp, 0, player, cause, (int)(eval | EV_FCHECK),
           &str, cargs, (int)ncargs);
      *bp = '\0';
    } else {
      fargs[arg] = alloc_lbuf("parse_arglist");
      StringCopy(fargs[arg], tstr);
    }
    arg++;
  }
  return dstr;
}

/*
 * ---------------------------------------------------------------------------
 * * exec: Process a command line, evaluating function calls and
 * %-substitutions.
 */

/*
 * ---------------------------------------------------------------------------
 * * Trace cache routines.
 */

typedef struct tcache_ent TCENT;
struct tcache_ent {
  char *orig;
  char *result;
  struct tcache_ent *next;
};

static int tcache_empty(EvaluationContext *context) {
  if (context->trace_top) {
    context->trace_top = false;
    context->trace_count = 0;
    return 1;
  }
  return 0;
}

static void tcache_add(EvaluationContext *context, char *orig, char *result) {
  char *tp;
  TCENT *xp;

  if (strcmp(orig, result)) {
    context->trace_count++;
    if (context->trace_count <= context->world->configuration->trace_limit) {
      xp = (TCENT *)alloc_sbuf("tcache_add.sbuf");
      tp = alloc_lbuf("tcache_add.lbuf");
      StringCopy(tp, result);
      xp->orig = orig;
      xp->result = tp;
      xp->next = context->trace_head;
      context->trace_head = xp;
    } else {
      free_lbuf(orig);
    }
  } else {
    free_lbuf(orig);
  }
}

static void tcache_finish(EvaluationContext *context, DbRef player) {
  TCENT *xp;

  while (context->trace_head != nullptr) {
    xp = context->trace_head;
    context->trace_head = xp->next;
    notify_printf(context, game_object_owner(context->world->database, player),
                  "%s(#%ld)} '%s' -> '%s'",
                  game_object_name(context->world->database, player), player,
                  xp->orig, xp->result);
    free_lbuf(xp->orig);
    free_lbuf(xp->result);
    free_sbuf(xp);
  }
  context->trace_top = true;
  context->trace_count = 0;
}

void exec(EvaluationContext *context, char *buff, char **bufc, int tflags,
          DbRef player, DbRef cause, int eval, char **dstr, char *cargs[],
          int ncargs) {
#define NFARGS 30
  char *fargs[NFARGS];
  char *preserve[MAX_GLOBAL_REGS];
  char *tstr, *tbuf, *tbufc, *savepos, *atr_gotten, *start, *oldp, *savestr;
  char savec, ch, *str;
  char *realbuff = nullptr, *realbp = nullptr;
  DbRef aowner;
  int at_space, nfargs, i, j, alldone, feval;
  long aflags;
  int do_trace, is_top, save_count;
  int ansi;
  FUN *fp;
  UFUN *ufp;

  if (*dstr == nullptr)
    return;

  // dprintk("%d/%s", player, *dstr);

  at_space = 1;
  alldone = 0;
  ansi = 0;

  do_trace = is_trace(context->world->database, player) && !(eval & EV_NOTRACE);
  is_top = 0;

  /* Extend the buffer if we need to. */

  if (((*bufc) - buff) > (LBUF_SIZE - SBUF_SIZE)) {
    realbuff = buff;
    realbp = *bufc;
    buff = malloc(LBUF_SIZE);
    *bufc = buff;
  }

  oldp = start = *bufc;

  /*
   * If we are tracing, save a copy of the starting buffer
   */

  savestr = nullptr;
  if (do_trace) {
    is_top = tcache_empty(context);
    savestr = alloc_lbuf("exec.save");
    StringCopy(savestr, *dstr);
  }
  while (**dstr && !alldone) {
    switch (**dstr) {
    case ' ':
      /*
       * A space.  Add a space if not compressing or if * *
       *
       * *  * * previous char was not a space
       */

      if (!(context->world->configuration->space_compress && at_space) ||
          (eval & EV_NO_COMPRESS)) {
        safe_chr(' ', buff, bufc);
        at_space = 1;
      }
      break;
    case '\\':
      /*
       * General escape.  Add the following char without *
       * * * * special processing
       */

      at_space = 0;
      (*dstr)++;
      if (**dstr)
        safe_chr(**dstr, buff, bufc);
      else
        (*dstr)--;
      break;
    case '[':
      /*
       * Function start.  Evaluate the contents of the * *
       * * * square brackets as a function.  If no closing
       * * * * * bracket, insert the [ and continue.
       */

      at_space = 0;
      tstr = (*dstr)++;
      if (eval & EV_NOFCHECK) {
        safe_chr('[', buff, bufc);
        *dstr = tstr;
        break;
      }
      tbuf = parse_to(context->world->configuration, dstr, ']', 0);
      if (*dstr == nullptr) {
        safe_chr('[', buff, bufc);
        *dstr = tstr;
      } else {
        str = tbuf;
        exec(context, buff, bufc, 0, player, cause,
             (eval | EV_FCHECK | EV_FMAND), &str, cargs, ncargs);
        (*dstr)--;
      }
      break;
    case '{':
      /*
       * Literal start.  Insert everything up to the * * *
       * * terminating } without parsing.  If no closing *
       * * * * brace, insert the { and continue.
       */

      at_space = 0;
      tstr = (*dstr)++;
      tbuf = parse_to(context->world->configuration, dstr, '}', 0);
      if (*dstr == nullptr) {
        safe_chr('{', buff, bufc);
        *dstr = tstr;
      } else {
        if (!(eval & EV_STRIP)) {
          safe_chr('{', buff, bufc);
        }
        /*
         * Preserve leading spaces (Felan)
         */

        if (*tbuf == ' ') {
          safe_chr(' ', buff, bufc);
          tbuf++;
        }
        str = tbuf;
        exec(context, buff, bufc, 0, player, cause,
             (eval & ~(EV_STRIP | EV_FCHECK)), &str, cargs, ncargs);
        if (!(eval & EV_STRIP)) {
          safe_chr('}', buff, bufc);
        }
        (*dstr)--;
      }
      break;
    case '%':
      /*
       * Percent-replace start.  Evaluate the chars * * *
       * following * and perform the appropriate * * *
       * substitution.
       */

      at_space = 0;
      (*dstr)++;
      savec = **dstr;
      savepos = *bufc;
      switch (savec) {
      case '\0': /*
                  * Null - all done
                  */
        (*dstr)--;
        break;
      case '|': /* piped command output */
        safe_str(context->pipe_output, buff, bufc);
        break;
      case '%': /*
                 * Percent - a literal %
                 */
        safe_chr('%', buff, bufc);
        break;
      case 'c':
      case 'C':
        (*dstr)++;
        if (!**dstr)
          (*dstr)--;
        ansi = 1;
        switch (**dstr) {
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
          ansi = 0;
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
          safe_chr(**dstr, buff, bufc);
        }
        break;
      case 'r': /*
                 * Carriage return
                 */
      case 'R':
        safe_str("\r\n", buff, bufc);
        break;
      case 't': /*
                 * Tab
                 */
      case 'T':
        safe_chr('\t', buff, bufc);
        break;
      case 'B': /*
                 * Blank
                 */
      case 'b':
        safe_chr(' ', buff, bufc);
        break;
      case '0': /*
                 * Command argument number N
                 */
      case '1':
      case '2':
      case '3':
      case '4':
      case '5':
      case '6':
      case '7':
      case '8':
      case '9':
        i = (**dstr - '0');
        if ((i < ncargs) && (cargs[i] != nullptr))
          safe_str(cargs[i], buff, bufc);
        break;
      case 'V': /*
                 * Variable attribute
                 */
      case 'v':
        (*dstr)++;
        ch = ToUpper(**dstr);
        if (!**dstr)
          (*dstr)--;
        if ((ch < 'A') || (ch > 'Z'))
          break;
        i = 100 + ch - 'A';
        atr_gotten = attribute_parent_get(context->world->database, player, i,
                                          &aowner, &aflags);
        safe_str(atr_gotten, buff, bufc);
        free_lbuf(atr_gotten);
        break;
      case 'Q':
      case 'q':
        (*dstr)++;
        i = (**dstr - '0');
        if ((i >= 0) && (i <= 9) && context->registers[i]) {
          safe_str(context->registers[i], buff, bufc);
        }
        if (!**dstr)
          (*dstr)--;
        break;
      case 'O': /*
                 * Objective pronoun
                 */
      case 'o':
        safe_str("it", buff, bufc);
        break;
      case 'P': /*
                 * Personal pronoun
                 */
      case 'p':
        safe_str("its", buff, bufc);
        break;
      case 'S': /*
                 * Subjective pronoun
                 */
      case 's':
        safe_str("it", buff, bufc);
        break;
      case 'A': /*
                 * Absolute posessive
                 */
      case 'a': /*
                 * idea from Empedocles
                 */
        safe_str("its", buff, bufc);
        break;
      case '#': /*
                 * Invoker DB number
                 */
        tbuf = alloc_sbuf("exec.invoker");
        snprintf(tbuf, SBUF_SIZE, "#%ld", cause);
        safe_str(tbuf, buff, bufc);
        free_sbuf(tbuf);
        break;
      case '!': /*
                 * Executor DB number
                 */
        tbuf = alloc_sbuf("exec.executor");
        snprintf(tbuf, SBUF_SIZE, "#%ld", player);
        safe_str(tbuf, buff, bufc);
        free_sbuf(tbuf);
        break;
      case 'N': /*
                 * Invoker name
                 */
      case 'n':
        safe_str(game_object_name(context->world->database, cause), buff, bufc);
        break;
      case 'L': /*
                 * Invoker location db#
                 */
      case 'l':
        if (!(eval & EV_NO_LOCATION)) {
          tbuf = alloc_sbuf("exec.exloc");
          snprintf(tbuf, SBUF_SIZE, "#%ld",
                   where_is(context->world->database, cause));
          safe_str(tbuf, buff, bufc);
          free_sbuf(tbuf);
        }

        break;
      default: /*
                * Just copy
                */
        safe_chr(**dstr, buff, bufc);
      }
      if ((*bufc > savepos) && isupper((unsigned char)savec))
        *savepos = ToUpper(*savepos);
      break;
    case '(':
      /*
       * Arglist start.  See if what precedes is a function. If so,
       * execute it if we should.
       */

      at_space = 0;
      if (!(eval & EV_FCHECK)) {
        safe_chr('(', buff, bufc);
        break;
      }
      /*
       * Load an sbuf with an uppercase version of the func name, and
       * see if the func exists.  Trim trailing spaces from the name
       * if configured.
       */

      **bufc = '\0';
      tbufc = tbuf = alloc_sbuf("exec.tbuf");
      safe_sb_str(oldp, tbuf, &tbufc);
      *tbufc = '\0';
      if (context->world->configuration->space_compress) {
        while ((--tbufc >= tbuf) && isspace(*tbufc))
          ;
        tbufc++;
        *tbufc = '\0';
      }
      for (tbufc = tbuf; *tbufc; tbufc++)
        *tbufc = ToLower(*tbufc);
      fp = (FUN *)hash_table_find(
          tbuf, &context->runtime->command_registry->functions);

      /*
       * If not a builtin func, check for global func
       */

      ufp = nullptr;
      if (fp == nullptr) {
        ufp = (UFUN *)hash_table_find(
            tbuf, &context->runtime->command_registry->user_function_index);
      }
      /*
       * Do the right thing if it doesn't exist
       */

      if (!fp && !ufp) {
        if (eval & EV_FMAND) {
          *bufc = oldp;
          safe_str("#-1 FUNCTION (", buff, bufc);
          safe_str(tbuf, buff, bufc);
          safe_str(") NOT FOUND", buff, bufc);
          alldone = 1;
        } else {
          safe_chr('(', buff, bufc);
        }
        free_sbuf(tbuf);
        eval &= ~EV_FCHECK;
        break;
      }
      free_sbuf(tbuf);

      /*
       * Get the arglist and count the number of args * Neg
       *
       * *  * *  * * # of args means catenate subsequent
       * args
       */

      if (!ufp && fp->nargs < 0)
        nfargs = -fp->nargs;
      else
        nfargs = NFARGS;
      tstr = *dstr;
      if (fp && (fp->flags & FN_NO_EVAL))
        feval = (eval & ~EV_EVAL) | EV_STRIP_ESC;
      else
        feval = eval;
      *dstr = parse_arglist(context, player, cause, *dstr + 1, ')', feval,
                            fargs, nfargs, cargs, ncargs);

      /*
       * If no closing delim, just insert the '(' and * * *
       *
       * * continue normally
       */

      if (!*dstr) {
        *dstr = tstr;
        safe_chr(**dstr, buff, bufc);
        for (i = 0; i < nfargs; i++)
          if (fargs[i] != nullptr)
            free_lbuf(fargs[i]);
        eval &= ~EV_FCHECK;
        break;
      }
      /*
       * Count number of args returned
       */

      (*dstr)--;
      j = 0;
      for (i = 0; i < nfargs; i++)
        if (fargs[i] != nullptr)
          j = i + 1;
      nfargs = j;

      /*
       * If it's a user-defined function, perform it now.
       */

      if (ufp) {
        context->function_nesting++;
        if (!check_access(context->world->database,
                          context->world->configuration, player, ufp->perms)) {
          safe_str("#-1 PERMISSION DENIED", buff, &oldp);
          *bufc = oldp;
        } else {
          tstr = attribute_get(context->world->database, ufp->obj, ufp->atr,
                               &aowner, &aflags);
          if (ufp->flags & FN_PRIV)
            i = (int)ufp->obj;
          else
            i = (int)player;
          str = tstr;

          if (ufp->flags & FN_PRES) {
            for (j = 0; j < MAX_GLOBAL_REGS; j++) {
              if (!context->registers[j])
                preserve[j] = nullptr;
              else {
                preserve[j] = alloc_lbuf("eval_regs");
                StringCopy(preserve[j], context->registers[j]);
              }
            }
          }

          exec(context, buff, &oldp, 0, i, cause, feval, &str, fargs, nfargs);
          *bufc = oldp;

          if (ufp->flags & FN_PRES) {
            for (j = 0; j < MAX_GLOBAL_REGS; j++) {
              if (preserve[j]) {
                if (!context->registers[j])
                  context->registers[j] = alloc_lbuf("eval_regs");
                StringCopy(context->registers[j], preserve[j]);
                free_lbuf(preserve[j]);
              } else {
                if (context->registers[j])
                  *(context->registers[i]) = '\0';
              }
            }
          }

          free_lbuf(tstr);
        }

        /*
         * Return the space allocated for the args
         */

        context->function_nesting--;
        for (i = 0; i < nfargs; i++)
          if (fargs[i] != nullptr)
            free_lbuf(fargs[i]);
        eval &= ~EV_FCHECK;
        break;
      }
      /*
       * If the number of args is right, perform the func.
       * Otherwise return an error message.  Note
       * that parse_arglist returns zero args as one
       * null arg, so we have to handle that case
       * specially.
       */

      if ((fp->nargs == 0) && (nfargs == 1)) {
        if (!*fargs[0]) {
          free_lbuf(fargs[0]);
          fargs[0] = nullptr;
          nfargs = 0;
        }
      }
      if ((nfargs == fp->nargs) || (nfargs == -fp->nargs) ||
          (fp->flags & FN_VARARGS)) {

        /*
         * Check recursion limit
         */

        context->function_nesting++;
        context->function_invocations++;
        if (context->function_nesting >=
            context->world->configuration->func_nest_lim) {
          safe_str("#-1 FUNCTION RECURSION LIMIT EXCEEDED", buff, bufc);
        } else if (context->function_invocations ==
                   context->world->configuration->func_invk_lim) {
          safe_str("#-1 FUNCTION INVOCATION LIMIT EXCEEDED", buff, bufc);
        } else if (!check_access(context->world->database,
                                 context->world->configuration, player,
                                 fp->perms)) {
          safe_str("#-1 PERMISSION DENIED", buff, &oldp);
          *bufc = oldp;
        } else if (context->function_invocations <
                   context->world->configuration->func_invk_lim) {
          fp->fun(buff, &oldp, player, cause, fargs, nfargs, cargs, ncargs,
                  context);
          *bufc = oldp;
        } else {
          **bufc = '\0';
        }
        context->function_nesting--;
      } else {
        *bufc = oldp;
        tstr = alloc_sbuf("exec.funcargs");
        snprintf(tstr, SBUF_SIZE, "%d", fp->nargs);
        safe_str("#-1 FUNCTION (", buff, bufc);
        safe_str(fp->name, buff, bufc);
        safe_str(") EXPECTS ", buff, bufc);
        safe_str(tstr, buff, bufc);
        safe_str(" ARGUMENTS", buff, bufc);
        free_sbuf(tstr);
      }

      /*
       * Return the space allocated for the arguments
       */

      for (i = 0; i < nfargs; i++)
        if (fargs[i] != nullptr)
          free_lbuf(fargs[i]);
      eval &= ~EV_FCHECK;
      break;
    default:
      /*
       * A mundane character.  Just copy it
       */

      at_space = 0;
      safe_chr(**dstr, buff, bufc);
    }
    (*dstr)++;
  }

  /*
   * If we're eating spaces, and the last thing was a space, eat it
   * up. Complicated by the fact that at_space is initially
   * true. So check to see if we actually put something in the
   * buffer, too.
   */

  if (context->world->configuration->space_compress && at_space &&
      !(eval & EV_NO_COMPRESS) && (start != *bufc))
    (*bufc)--;

  /*
   * The is_ansi(context->world->database, ) function knows how to take care of
   * itself. However, if the player used a %c sub in the string, and hasn't yet
   * terminated the color with a %cn yet, we'll have to do it for
   * them.
   */

  if (ansi == 1)
    safe_str(ANSI_NORMAL, buff, bufc);

  **bufc = '\0';

  /*
   * Report trace information
   */

  if (realbuff) {
    char *oldbuff = buff;
    **bufc = '\0';
    *bufc = realbp;
    safe_str(buff, realbuff, bufc);
    start = realbuff + (start - oldbuff);
    free(oldbuff);
    buff = realbuff;
  }

  if (do_trace) {
    tcache_add(context, savestr, start);
    save_count =
        context->trace_count - context->world->configuration->trace_limit;
    ;
    if (is_top || !context->world->configuration->trace_topdown)
      tcache_finish(context, player);
    if (is_top && (save_count > 0)) {
      tbuf = alloc_mbuf("exec.trace_diag");
      snprintf(tbuf, MBUF_SIZE, "%d lines of trace output discarded.",
               save_count);
      notify(context, player, tbuf);
      free_mbuf(tbuf);
    }
  }
}
