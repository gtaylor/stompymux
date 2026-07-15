/* boolexp.c - Lock expression parsing, evaluation, and serialization. */

#include "mux/server/platform.h"

#include "mux/database/attrs.h"
#include "mux/database/boolexp.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"

static int parsing_internal = 0;

/**
 * Indicate if attribute ATTR on player passes key when checked by
 * the object lockobj
 */
static int check_attr(DbRef player, DbRef lockobj, Attribute *attr, char *key) {
  char *buff;
  DbRef aowner;
  long aflags;
  int checkit;

  buff = attribute_parent_get(player, attr->number, &aowner, &aflags);
  checkit = 0;

  if (attr->number == A_LENTER) {
    /* We can see enterlocks... else we'd break zones */
    checkit = 1;
  } else if (See_attr(lockobj, player, attr, aowner, aflags)) {
    checkit = 1;
  } else if (attr->number == A_NAME) {
    checkit = 1;
  }
  if (checkit && (!wild_match(key, buff))) {
    checkit = 0;
  }
  free_lbuf(buff);
  return checkit;
} /* end check_attr() */

int boolean_expression_evaluate(DbRef player, DbRef thing, DbRef from,
                                BooleanExpression *b) {
  DbRef aowner, obj, source;
  long aflags;
  int c, checkit;
  char *key, *buff, *buff2, *bp, *str;
  Attribute *a;

  if (b == TRUE_BOOLEXP)
    return 1;

  switch (b->type) {
  case BOOLEXP_AND:
    return (boolean_expression_evaluate(player, thing, from, b->sub1) &&
            boolean_expression_evaluate(player, thing, from, b->sub2));
  case BOOLEXP_OR:
    return (boolean_expression_evaluate(player, thing, from, b->sub1) ||
            boolean_expression_evaluate(player, thing, from, b->sub2));
  case BOOLEXP_NOT:
    return !boolean_expression_evaluate(player, thing, from, b->sub1);
  case BOOLEXP_INDIR:
    /*
     * BOOLEXP_INDIR (i.e. @) is a unary operation which is replaced at
     * evaluation time by the lock of the object whose number is the
     * argument of the operation.
     */

    mudstate.lock_nest_lev++;
    if (mudstate.lock_nest_lev >= mudconf.lock_nest_lim) {
      //            log_error(LOG_BUGS, "BUG", "LOCK", "
      STARTLOG(LOG_BUGS, "BUG", "LOCK") {
        log_name_and_loc(player);
        log_text((char *)": Lock exceeded recursion limit.");
        ENDLOG;
      }
      notify(player, "Sorry, broken lock!");
      mudstate.lock_nest_lev--;
      return (0);
    }
    if ((b->sub1->type != BOOLEXP_CONST) || (b->sub1->thing < 0)) {
      STARTLOG(LOG_BUGS, "BUG", "LOCK") {
        log_name_and_loc(player);
        buff = alloc_mbuf("boolean_expression_evaluate.LOG.indir");
        snprintf(buff, MBUF_SIZE, ": Lock had bad indirection (%c, type %d)",
                 INDIR_TOKEN, b->sub1->type);
        log_text(buff);
        free_mbuf(buff);
        ENDLOG;
      }
      notify(player, "Sorry, broken lock!");
      mudstate.lock_nest_lev--;
      return (0);
    }
    key = attribute_get(b->sub1->thing, A_LOCK, &aowner, &aflags);
    c = eval_boolexp_atr(player, b->sub1->thing, from, key);
    free_lbuf(key);
    mudstate.lock_nest_lev--;
    return (c);
  case BOOLEXP_CONST:
    return (b->thing == player || member(b->thing, Contents(player)));
  case BOOLEXP_ATR:
    a = attribute_by_number(b->thing);
    if (!a)
      return 0; /*
                 * no such attribute
                 */

    /*
     * First check the object itself, then its contents
     */

    if (check_attr(player, from, a, (char *)b->sub1))
      return 1;
    DOLIST(obj, Contents(player)) {
      if (check_attr(obj, from, a, (char *)b->sub1))
        return 1;
    }
    return 0;
  case BOOLEXP_EVAL:
    a = attribute_by_number(b->thing);
    if (!a)
      return 0; /*
                 * no such attribute
                 */
    source = from;
    buff = attribute_parent_get(from, a->number, &aowner, &aflags);
    if (!buff || !*buff) {
      free_lbuf(buff);
      buff = attribute_parent_get(thing, a->number, &aowner, &aflags);
      source = thing;
    }
    checkit = 0;

    if ((a->number == A_NAME) || (a->number == A_LENTER)) {
      checkit = 1;
    } else if (Read_attr(source, source, a, aowner, aflags)) {
      checkit = 1;
    }
    if (checkit) {
      buff2 = bp = alloc_lbuf("boolean_expression_evaluate");
      str = buff;
      exec(buff2, &bp, 0, source, player, EV_FIGNORE | EV_EVAL | EV_TOP, &str,
           (char **)NULL, 0);
      *bp = '\0';
      checkit = !string_compare(buff2, (char *)b->sub1);
      free_lbuf(buff2);
    }
    free_lbuf(buff);
    return checkit;
  case BOOLEXP_IS:

    /*
     * If an object check, do that
     */

    if (b->sub1->type == BOOLEXP_CONST)
      return (b->sub1->thing == player);

    /*
     * Nope, do an attribute check
     */

    a = attribute_by_number(b->sub1->thing);
    if (!a)
      return 0;
    return (check_attr(player, from, a, (char *)(b->sub1)->sub1));
  case BOOLEXP_CARRY:

    /*
     * If an object check, do that
     */

    if (b->sub1->type == BOOLEXP_CONST)
      return (member(b->sub1->thing, Contents(player)));

    /*
     * Nope, do an attribute check
     */

    a = attribute_by_number(b->sub1->thing);
    if (!a)
      return 0;
    DOLIST(obj, Contents(player)) {
      if (check_attr(obj, from, a, (char *)(b->sub1)->sub1))
        return 1;
    }
    return 0;
  case BOOLEXP_OWNER:
    return (Owner(b->sub1->thing) == Owner(player));
  default:
    abort(); /*
              * bad type
              */
    return 0;
  }
} /* end boolean_expression_evaluate() */

int eval_boolexp_atr(DbRef player, DbRef thing, DbRef from, char *key) {
  BooleanExpression *b;
  int ret_value;

  b = boolean_expression_parse(player, key, 1);
  if (b == NULL) {
    ret_value = 1;
  } else {
    ret_value = boolean_expression_evaluate(player, thing, from, b);
    boolean_expression_free(b);
  }
  return (ret_value);
} /* end eval_boolexp_atr() */

/*
 * If the parser returns TRUE_BOOLEXP, you lose
 */

/*
 * TRUE_BOOLEXP cannot be typed in by the user; use @unlock instead
 */

static const char *parsebuf;
static char parsestore[LBUF_SIZE];
static DbRef parse_player;

static void skip_whitespace(void) {
  while (*parsebuf && isspace(*parsebuf))
    parsebuf++;
}

static BooleanExpression *parse_boolexp_E(void); /* defined below */

static BooleanExpression *test_atr(char *s) {
  Attribute *attrib;
  BooleanExpression *b;
  char *buff, *s1;
  int anum, locktype;

  buff = alloc_lbuf("test_atr");
  StringCopy(buff, s);
  for (s = buff; *s && (*s != ':') && (*s != '/'); s++)
    ;
  if (!*s) {
    free_lbuf(buff);
    return ((BooleanExpression *)NULL);
  }
  if (*s == '/')
    locktype = BOOLEXP_EVAL;
  else
    locktype = BOOLEXP_ATR;

  *s++ = '\0';
  /*
   * see if left side is valid attribute.  Access to attr is checked on
   *
   * *  * *  * *  * *  * * eval * Also allow numeric references to *
   * attributes. * It * can't * hurt  * us, and * lets us import stuff
   * * that stores * attr * locks by * number * instead of by * name.
   */
  if (!(attrib = attribute_by_name(buff))) {

    /*
     * Only #1 can lock on numbers
     */
    if (!God(parse_player)) {
      free_lbuf(buff);
      return ((BooleanExpression *)NULL);
    }
    for (s1 = buff; isdigit(*s1); s1++)
      ;
    if (*s1) {
      free_lbuf(buff);
      return ((BooleanExpression *)NULL);
    }
    anum = atoi(buff);
  } else {
    anum = attrib->number;
  }

  /*
   * made it now make the parse tree node
   */
  b = alloc_bool("test_str");
  b->type = locktype;
  b->thing = (DbRef)anum;
  b->sub1 = (BooleanExpression *)strsave(s);
  free_lbuf(buff);
  return (b);
} /* end test_atr() */

/*
 * L -> (E); L -> object identifier
 */
static BooleanExpression *parse_boolexp_L(void) {
  BooleanExpression *b;
  char *p, *buf;
  MSTATE mstate;

  buf = NULL;
  skip_whitespace();

  switch (*parsebuf) {
  case '(':
    parsebuf++;
    b = parse_boolexp_E();
    skip_whitespace();
    if (b == TRUE_BOOLEXP || *parsebuf++ != ')') {
      boolean_expression_free(b);
      return TRUE_BOOLEXP;
    }
    break;
  default:

    /*
     * Must have hit an object ref. Load the name into our
     * buffer
     */

    buf = alloc_lbuf("parse_boolexp_L");
    p = buf;
    while (*parsebuf && (*parsebuf != AND_TOKEN) && (*parsebuf != OR_TOKEN) &&
           (*parsebuf != ')')) {
      *p++ = *parsebuf++;
    }

    /*
     * strip trailing whitespace
     */

    *p = '\0';
    while (p > buf && isspace((unsigned char)p[-1]))
      *--p = '\0';

    /*
     * check for an attribute
     */

    if ((b = test_atr(buf)) != NULL) {
      free_lbuf(buf);
      return (b);
    }
    b = alloc_bool("parse_boolexp_L");
    b->type = BOOLEXP_CONST;

    /*
     * do the match
     */

    /*
     * If we are parsing a boolexp that was a stored lock then
     * we know that object refs are all dbrefs, so we
     * skip the expensive match code.
     */

    if (parsing_internal) {
      if (buf[0] != '#') {
        free_lbuf(buf);
        free_bool(b);
        return TRUE_BOOLEXP;
      }
      b->thing = atoi(&buf[1]);
      if (!Good_obj(b->thing)) {
        free_lbuf(buf);
        free_bool(b);
        return TRUE_BOOLEXP;
      }
    } else {
      save_match_state(&mstate);
      init_match(parse_player, buf, TYPE_THING);
      match_everything(MAT_EXIT_PARENTS);
      b->thing = match_result();
      restore_match_state(&mstate);
    }

    if (b->thing == NOTHING) {
      notify_printf(parse_player, "I don't see %s here.", buf);
      free_lbuf(buf);
      free_bool(b);
      return TRUE_BOOLEXP;
    }
    if (b->thing == AMBIGUOUS) {
      notify_printf(parse_player, "I don't know which %s you mean!", buf);
      free_lbuf(buf);
      free_bool(b);
      return TRUE_BOOLEXP;
    }
    free_lbuf(buf);
  }
  return b;
} /* end parse_boolexp_L() */

/*
 * F -> !F; F -> @L; F -> =L; F -> +L; F -> $L
 */

/*
 * The argument L must be type BOOLEXP_CONST
 */

static BooleanExpression *parse_boolexp_F(void) {
  BooleanExpression *b2;

  skip_whitespace();
  switch (*parsebuf) {
  case NOT_TOKEN:
    parsebuf++;
    b2 = alloc_bool("parse_boolexp_F.not");
    b2->type = BOOLEXP_NOT;
    if ((b2->sub1 = parse_boolexp_F()) == TRUE_BOOLEXP) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  case INDIR_TOKEN:
    parsebuf++;
    b2 = alloc_bool("parse_boolexp_F.indir");
    b2->type = BOOLEXP_INDIR;
    b2->sub1 = parse_boolexp_L();
    if ((b2->sub1) == TRUE_BOOLEXP) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else if ((b2->sub1->type) != BOOLEXP_CONST) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  case IS_TOKEN:
    parsebuf++;
    b2 = alloc_bool("parse_boolexp_F.is");
    b2->type = BOOLEXP_IS;
    b2->sub1 = parse_boolexp_L();
    if ((b2->sub1) == TRUE_BOOLEXP) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else if (((b2->sub1->type) != BOOLEXP_CONST) &&
               ((b2->sub1->type) != BOOLEXP_ATR)) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  case CARRY_TOKEN:
    parsebuf++;
    b2 = alloc_bool("parse_boolexp_F.carry");
    b2->type = BOOLEXP_CARRY;
    b2->sub1 = parse_boolexp_L();
    if ((b2->sub1) == TRUE_BOOLEXP) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else if (((b2->sub1->type) != BOOLEXP_CONST) &&
               ((b2->sub1->type) != BOOLEXP_ATR)) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  case OWNER_TOKEN:
    parsebuf++;
    b2 = alloc_bool("parse_boolexp_F.owner");
    b2->type = BOOLEXP_OWNER;
    b2->sub1 = parse_boolexp_L();
    if ((b2->sub1) == TRUE_BOOLEXP) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else if ((b2->sub1->type) != BOOLEXP_CONST) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  default:
    return (parse_boolexp_L());
  }
} /* end parse_boolexp_F() */

/*
 * T -> F; T -> F & T
 */

static BooleanExpression *parse_boolexp_T(void) {
  BooleanExpression *b, *b2;

  if ((b = parse_boolexp_F()) != TRUE_BOOLEXP) {
    skip_whitespace();
    if (*parsebuf == AND_TOKEN) {
      parsebuf++;

      b2 = alloc_bool("parse_boolexp_T");
      b2->type = BOOLEXP_AND;
      b2->sub1 = b;
      if ((b2->sub2 = parse_boolexp_T()) == TRUE_BOOLEXP) {
        boolean_expression_free(b2);
        return TRUE_BOOLEXP;
      }
      b = b2;
    }
  }
  return b;
} /* end parse_boolexp_T() */

/*
 * E -> T; E -> T | E
 */
static BooleanExpression *parse_boolexp_E(void) {
  BooleanExpression *b, *b2;

  if ((b = parse_boolexp_T()) != TRUE_BOOLEXP) {
    skip_whitespace();
    if (*parsebuf == OR_TOKEN) {
      parsebuf++;

      b2 = alloc_bool("parse_boolexp_E");
      b2->type = BOOLEXP_OR;
      b2->sub1 = b;
      if ((b2->sub2 = parse_boolexp_E()) == TRUE_BOOLEXP) {
        boolean_expression_free(b2);
        return TRUE_BOOLEXP;
      }
      b = b2;
    }
  }
  return b;
} /* end parse_boolexp_E() */

BooleanExpression *boolean_expression_parse(DbRef player, const char *buf,
                                            int internal) {
  StringCopy(parsestore, buf);
  parsebuf = parsestore;
  parse_player = player;
  if ((buf == NULL) || (*buf == '\0'))
    return (TRUE_BOOLEXP);
  parsing_internal = internal;
  return parse_boolexp_E();
} /* end boolean_expression_parse() */

typedef enum BooleanExpressionUnparseFormat {
  BOOLEXP_UNPARSE_EXAMINE,
  BOOLEXP_UNPARSE_QUIET,
  BOOLEXP_UNPARSE_FUNCTION,
} BooleanExpressionUnparseFormat;

static char boolexp_unparse_buffer[LBUF_SIZE];
static char *boolexp_unparse_top;

static char *boolean_expression_unparse_object_quiet(DbRef object) {
  static char buffer[SBUF_SIZE];

  switch (object) {
  case NOTHING:
    return (char *)"-1";
  case HOME:
    return (char *)"-3";
  default:
    snprintf(buffer, sizeof(buffer), "(#%ld)", object);
    return buffer;
  }
}

static void
boolean_expression_unparse_internal(DbRef player, BooleanExpression *expression,
                                    char outer_type,
                                    BooleanExpressionUnparseFormat format) {
  Attribute *attribute;
  char *attribute_number, separator;
  char *buffer;

  if (expression == TRUE_BOOLEXP) {
    if (format == BOOLEXP_UNPARSE_EXAMINE)
      safe_str((char *)"*UNLOCKED*", boolexp_unparse_buffer,
               &boolexp_unparse_top);
    return;
  }

  switch (expression->type) {
  case BOOLEXP_AND:
    if (outer_type == BOOLEXP_NOT)
      safe_chr('(', boolexp_unparse_buffer, &boolexp_unparse_top);
    boolean_expression_unparse_internal(player, expression->sub1,
                                        expression->type, format);
    safe_chr(AND_TOKEN, boolexp_unparse_buffer, &boolexp_unparse_top);
    boolean_expression_unparse_internal(player, expression->sub2,
                                        expression->type, format);
    if (outer_type == BOOLEXP_NOT)
      safe_chr(')', boolexp_unparse_buffer, &boolexp_unparse_top);
    break;
  case BOOLEXP_OR:
    if (outer_type == BOOLEXP_NOT || outer_type == BOOLEXP_AND)
      safe_chr('(', boolexp_unparse_buffer, &boolexp_unparse_top);
    boolean_expression_unparse_internal(player, expression->sub1,
                                        expression->type, format);
    safe_chr(OR_TOKEN, boolexp_unparse_buffer, &boolexp_unparse_top);
    boolean_expression_unparse_internal(player, expression->sub2,
                                        expression->type, format);
    if (outer_type == BOOLEXP_NOT || outer_type == BOOLEXP_AND)
      safe_chr(')', boolexp_unparse_buffer, &boolexp_unparse_top);
    break;
  case BOOLEXP_NOT:
    safe_chr('!', boolexp_unparse_buffer, &boolexp_unparse_top);
    boolean_expression_unparse_internal(player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_INDIR:
    safe_chr(INDIR_TOKEN, boolexp_unparse_buffer, &boolexp_unparse_top);
    boolean_expression_unparse_internal(player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_IS:
    safe_chr(IS_TOKEN, boolexp_unparse_buffer, &boolexp_unparse_top);
    boolean_expression_unparse_internal(player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_CARRY:
    safe_chr(CARRY_TOKEN, boolexp_unparse_buffer, &boolexp_unparse_top);
    boolean_expression_unparse_internal(player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_OWNER:
    safe_chr(OWNER_TOKEN, boolexp_unparse_buffer, &boolexp_unparse_top);
    boolean_expression_unparse_internal(player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_CONST:
    if (format == BOOLEXP_UNPARSE_QUIET) {
      safe_str(boolean_expression_unparse_object_quiet(expression->thing),
               boolexp_unparse_buffer, &boolexp_unparse_top);
    } else if (format == BOOLEXP_UNPARSE_EXAMINE) {
      buffer = unparse_object(player, expression->thing, 0);
      safe_str(buffer, boolexp_unparse_buffer, &boolexp_unparse_top);
      free_lbuf(buffer);
    } else {
      if (Typeof(expression->thing) == TYPE_PLAYER) {
        safe_chr('*', boolexp_unparse_buffer, &boolexp_unparse_top);
        safe_str(Name(expression->thing), boolexp_unparse_buffer,
                 &boolexp_unparse_top);
      } else {
        buffer = alloc_sbuf("boolean_expression_unparse_internal");
        snprintf(buffer, SBUF_SIZE, "#%ld", expression->thing);
        safe_str(buffer, boolexp_unparse_buffer, &boolexp_unparse_top);
        free_sbuf(buffer);
      }
    }
    break;
  case BOOLEXP_ATR:
  case BOOLEXP_EVAL:
    separator = expression->type == BOOLEXP_EVAL ? '/' : ':';
    attribute = attribute_by_number(expression->thing);
    if (attribute && attribute->number) {
      safe_str((char *)attribute->name, boolexp_unparse_buffer,
               &boolexp_unparse_top);
      safe_chr(separator, boolexp_unparse_buffer, &boolexp_unparse_top);
      safe_str((char *)expression->sub1, boolexp_unparse_buffer,
               &boolexp_unparse_top);
    } else if (expression->thing > 0) {
      attribute_number =
          alloc_sbuf("boolean_expression_unparse_internal.attribute_number");
      snprintf(attribute_number, SBUF_SIZE, "%ld", expression->thing);
      safe_str(attribute_number, boolexp_unparse_buffer, &boolexp_unparse_top);
      safe_chr(separator, boolexp_unparse_buffer, &boolexp_unparse_top);
      safe_str((char *)expression->sub1, boolexp_unparse_buffer,
               &boolexp_unparse_top);
      free_sbuf(attribute_number);
    } else {
      safe_str((char *)expression->sub2, boolexp_unparse_buffer,
               &boolexp_unparse_top);
      safe_chr(separator, boolexp_unparse_buffer, &boolexp_unparse_top);
      safe_str((char *)expression->sub1, boolexp_unparse_buffer,
               &boolexp_unparse_top);
    }
    break;
  default:
    fprintf(stderr, "Fell off the end of switch in "
                    "boolean_expression_unparse_internal()\n");
    abort();
  }
}

static char *
boolean_expression_unparse_format(DbRef player, BooleanExpression *expression,
                                  BooleanExpressionUnparseFormat format) {
  boolexp_unparse_top = boolexp_unparse_buffer;
  boolean_expression_unparse_internal(player, expression, BOOLEXP_CONST,
                                      format);
  *boolexp_unparse_top = '\0';
  return boolexp_unparse_buffer;
}

char *boolean_expression_unparse_quiet(DbRef player,
                                       BooleanExpression *expression) {
  return boolean_expression_unparse_format(player, expression,
                                           BOOLEXP_UNPARSE_QUIET);
}

char *boolean_expression_unparse(DbRef player, BooleanExpression *expression) {
  return boolean_expression_unparse_format(player, expression,
                                           BOOLEXP_UNPARSE_EXAMINE);
}

char *boolean_expression_unparse_function(DbRef player,
                                          BooleanExpression *expression) {
  return boolean_expression_unparse_format(player, expression,
                                           BOOLEXP_UNPARSE_FUNCTION);
}
