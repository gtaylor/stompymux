/*
 * stringutil.c -- string utilities
 */

#include "mux/server/platform.h"

#include <limits.h>

#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"

#ifdef __linux__
char *___strtok;

#endif

/**
 * Parses str as a base-10 long, clamping out-of-range values to
 * LONG_MIN/LONG_MAX and treating non-numeric input as 0.
 */
long clamped_atol(const char *str) {
  char *end;
  long value;

  errno = 0;
  value = strtol(str, &end, 10);
  if (errno == ERANGE)
    value = (str[0] == '-') ? LONG_MIN : LONG_MAX;
  return value;
}

/**
 * Parses str as a base-10 int, clamping out-of-range values to
 * INT_MIN/INT_MAX and treating non-numeric input as 0.
 */
int clamped_atoi(const char *str) {
  long value = clamped_atol(str);

  if (value > INT_MAX)
    return INT_MAX;
  if (value < INT_MIN)
    return INT_MIN;
  return (int)value;
}

/*
 * capitalizes an entire string
 */

char *upcasestr(char *s) {
  char *p;

  for (p = s; p && *p; p++)
    *p = ToUpper(*p);
  return s;
}

/**
 * Compress multiple spaces to one space, also remove leading and
 * trailing spaces.
 */
char *munge_space(char *string) {
  char *buffer, *p, *q;

  buffer = alloc_lbuf("munge_space");
  p = string;
  q = buffer;
  while (p && *p && isspace(*p))
    p++; /*
          * remove inital spaces
          */
  while (p && *p) {
    while (*p && !isspace(*p))
      *q++ = *p++;
    while (*p && isspace(*++p))
      ;
    if (*p)
      *q++ = ' ';
  }
  *q = '\0'; /*
              * remove terminal spaces and terminate * * *
              *
              * * string
              */
  return (buffer);
}

/**
 * Remove leading and trailing spaces.
 */
char *trim_spaces(char *string) {
  char *buffer, *p, *q;

  buffer = alloc_lbuf("trim_spaces");
  p = string;
  q = buffer;
  while (p && *p && isspace(*p)) /*
                                  * remove inital spaces
                                  */
    p++;
  while (p && *p) {
    while (*p && !isspace(*p)) /*
                                * copy nonspace chars
                                */
      *q++ = *p++;
    while (*p && isspace(*p)) /*
                               * compress spaces
                               */
      p++;
    if (*p)
      *q++ = ' '; /*
                   * leave one space
                   */
  }
  *q = '\0'; /*
              * terminate string
              */
  return (buffer);
}

/**
 * Return portion of a string up to the indicated character. Also
 * returns a modified pointer to the string ready for another call.
 */
char *grabto(char **str, char targ) {
  char *savec, *cp;

  if (!str || !*str || !**str)
    return nullptr;

  savec = cp = *str;
  while (*cp && *cp != targ)
    cp++;
  if (*cp)
    *cp++ = '\0';
  *str = cp;
  return savec;
}

int string_compare(const ServerConfiguration *configuration, const char *s1,
                   const char *s2) {
  if (!configuration->space_compress) {
    while (*s1 && *s2 && ToLower(*s1) == ToLower(*s2))
      s1++, s2++;

    return (ToLower(*s1) - ToLower(*s2));
  } else {
    while (isspace(*s1))
      s1++;
    while (isspace(*s2))
      s2++;
    while (*s1 && *s2 &&
           ((ToLower(*s1) == ToLower(*s2)) || (isspace(*s1) && isspace(*s2)))) {
      if (isspace(*s1) && isspace(*s2)) { /*
                                           * skip all
                                           * other
                                           * spaces
                                           */
        while (isspace(*s1))
          s1++;
        while (isspace(*s2))
          s2++;
      } else {
        s1++;
        s2++;
      }
    }
    if ((*s1) && (*s2))
      return (1);
    if (isspace(*s1)) {
      while (isspace(*s1))
        s1++;
      return (*s1);
    }
    if (isspace(*s2)) {
      while (isspace(*s2))
        s2++;
      return (*s2);
    }
    if ((*s1) || (*s2))
      return (1);
    return (0);
  }
}

int string_prefix(const char *string, const char *prefix) {
  int count = 0;

  while (*string && *prefix && ToLower(*string) == ToLower(*prefix))
    string++, prefix++, count++;
  if (*prefix == '\0') /*
                        * Matched all of prefix
                        */
    return (count);
  else
    return (0);
}

/**
 * Accepts only nonempty matches starting at the beginning of a word
 */
const char *string_match(const char *src, const char *sub) {
  if ((*sub != '\0') && (src)) {
    while (*src) {
      if (string_prefix(src, sub))
        return src;
      /*
       * else scan to beginning of next word
       */
      while (*src && isalnum(*src))
        src++;
      while (*src && !isalnum(*src))
        src++;
    }
  }
  return 0;
}

/**
 * Returns an lbuf containing string STRING with all occurances
 * of OLD replaced by NEW. OLD and NEW may be different lengths.
 */
char *replace_string(const char *old, const char *new, const char *string) {
  char *result, *r;
  const char *s;
  int olen;

  if (string == nullptr)
    return nullptr;
  s = string;
  olen = (int)strlen(old);
  r = result = alloc_lbuf("replace_string");
  while (*s) {

    /*
     * Copy up to the next occurrence of the first char of OLD
     */

    while (*s && *s != *old) {
      safe_chr(*s, result, &r);
      s++;
    }

    /*
     * If we are really at an OLD, append NEW to the result and *
     *
     * *  * *  * * bump the input string past the occurrence of
     * OLD. *  * * * Otherwise, copy the char and try again.
     */

    if (*s) {
      if (!strncmp(old, s, (size_t)olen)) {
        safe_str(new, result, &r);
        s += olen;
      } else {
        safe_chr(*s, result, &r);
        s++;
      }
    }
  }
  *r = '\0';
  return result;
}

int minmatch(const char *str, const char *target, int min) {
  while (*str && *target && (ToLower(*str) == ToLower(*target))) {
    str++;
    target++;
    min--;
  }
  if (*str)
    return 0;
  if (!*target)
    return 1;
  return ((min <= 0) ? 1 : 0);
}

char *strsave(const char *s) {
  char *p;
  p = (char *)malloc(sizeof(char) * (strlen(s) + 1));

  if (p)
    StringCopy(p, s);
  return p;
}

/**
 * Copy buffers, watching for overflows.
 */
int safe_copy_str(const char *src, char *buff, char **bufp, int max) {
  char *tp;

  tp = *bufp;
  if (src == nullptr)
    return 0;
  while (*src && ((tp - buff) < max))
    *tp++ = *src++;
  *bufp = tp;
  return (int)strlen(src);
}

/**
 * Copy buffers, watching for overflows.
 */
int safe_copy_chr(char src, char *buff, char **bufp, int max) {
  char *tp;
  int retval;

  tp = *bufp;
  retval = 0;
  if ((tp - buff) < max) {
    *tp++ = src;
  } else {
    retval = 1;
  }
  *bufp = tp;
  return retval;
}

int matches_exit_from_list(char *str, char *pattern) {
  char *s;

  while (*pattern) {
    for (s = str; /*
                   * check out this one
                   */
         (*s && (ToLower(*s) == ToLower(*pattern)) && *pattern &&
          (*pattern != EXIT_DELIMITER));
         s++, pattern++)
      ;

    /*
     * Did we match it all?
     */

    if (*s == '\0') {

      /*
       * Make sure nothing afterwards
       */

      while (*pattern && isspace(*pattern))
        pattern++;

      /*
       * Did we get it?
       */

      if (!*pattern || (*pattern == EXIT_DELIMITER))
        return 1;
    }
    /*
     * We didn't get it, find next string to test
     */

    while (*pattern && *pattern++ != EXIT_DELIMITER)
      ;
    while (isspace(*pattern))
      pattern++;
  }
  return 0;
}
