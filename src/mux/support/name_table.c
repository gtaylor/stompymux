/*
 * htab.c - table hashing routines
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/db.h"
#include "mux/server/configuration.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"

#include "mux/server/server_state.h"

/*
 * ---------------------------------------------------------------------------
 * * name_table_search: Search a name table for a match and return the flag
 * value.
 */
int name_table_search(DbRef player, NameTable *ntab, char *flagname) {
  NameTable *nt;

  for (nt = ntab; nt->name; nt++) {
    if (minmatch(flagname, nt->name, nt->minlen)) {
      if (check_access(player, nt->perm)) {
        return nt->flag;
      } else
        return -2;
    }
  }
  return -1;
}

/*
 * ---------------------------------------------------------------------------
 * * name_table_find_entry: Search a name table for a match and return a pointer
 * to it.
 */

NameTable *name_table_find_entry(DbRef player, NameTable *ntab,
                                 char *flagname) {
  NameTable *nt;

  for (nt = ntab; nt->name; nt++) {
    if (minmatch(flagname, nt->name, nt->minlen)) {
      if (check_access(player, nt->perm)) {
        return nt;
      }
    }
  }
  return NULL;
}

/*
 * ---------------------------------------------------------------------------
 * * name_table_display: Print out the names of the entries in a name table.
 */

void name_table_display(DbRef player, NameTable *ntab, char *prefix,
                        int list_if_none) {
  char *buf, *bp, *cp;
  NameTable *nt;
  int got_one;

  buf = alloc_lbuf("name_table_display");
  bp = buf;
  got_one = 0;
  for (cp = prefix; *cp; cp++)
    *bp++ = *cp;
  for (nt = ntab; nt->name; nt++) {
    if (is_god(player) || check_access(player, nt->perm)) {
      *bp++ = ' ';
      for (cp = nt->name; *cp; cp++)
        *bp++ = *cp;
      got_one = 1;
    }
  }
  *bp = '\0';
  if (got_one || list_if_none)
    notify(player, buf);
  free_lbuf(buf);
}

/*
 * ---------------------------------------------------------------------------
 * * name_table_interpret: Print values for flags defined in name table.
 */

void name_table_interpret(DbRef player, NameTable *ntab, int flagword,
                          char *prefix, char *true_text, char *false_text) {
  char *buf, *bp, *cp;
  NameTable *nt;

  buf = alloc_lbuf("name_table_interpret");
  bp = buf;
  for (cp = prefix; *cp; cp++)
    *bp++ = *cp;
  nt = ntab;
  while (nt->name) {
    if (is_god(player) || check_access(player, nt->perm)) {
      *bp++ = ' ';
      for (cp = nt->name; *cp; cp++)
        *bp++ = *cp;
      *bp++ = '.';
      *bp++ = '.';
      *bp++ = '.';
      if ((flagword & nt->flag) != 0)
        cp = true_text;
      else
        cp = false_text;
      while (*cp)
        *bp++ = *cp++;
      if ((++nt)->name)
        *bp++ = ';';
    }
  }
  *bp = '\0';
  notify(player, buf);
  free_lbuf(buf);
}

/*
 * ---------------------------------------------------------------------------
 * * name_table_list_set: Print values for flags defined in name table.
 */

void name_table_list_set(DbRef player, NameTable *ntab, int flagword,
                         char *prefix, int list_if_none) {
  char *buf, *bp, *cp;
  NameTable *nt;
  int got_one;

  buf = bp = alloc_lbuf("name_table_list_set");
  for (cp = prefix; *cp; cp++)
    *bp++ = *cp;
  nt = ntab;
  got_one = 0;
  while (nt->name) {
    if (((flagword & nt->flag) != 0) &&
        (is_god(player) || check_access(player, nt->perm))) {
      *bp++ = ' ';
      for (cp = nt->name; *cp; cp++)
        *bp++ = *cp;
      got_one = 1;
    }
    nt++;
  }
  *bp = '\0';
  if (got_one || list_if_none)
    notify(player, buf);
  free_lbuf(buf);
}

int cf_ntab_access(int *vp, char *str, long extra, DbRef player, char *cmd) {
  NameTable *np;
  char *ap;

  for (ap = str; *ap && !isspace(*ap); ap++)
    ;
  if (*ap)
    *ap++ = '\0';
  while (*ap && isspace(*ap))
    ap++;
  for (np = (NameTable *)vp; np->name; np++) {
    if (minmatch(str, np->name, np->minlen)) {
      return configuration_modify_bits(&(np->perm), ap, extra, player, cmd);
    }
  }
  configuration_log_not_found(player, cmd, "Entry", str);
  return -1;
}
