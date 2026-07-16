/*
 * macro.c - ported from BattleTech 3056 MUSE
 */

#include "mux/commands/macro.h"
#include "mux/communication/commac.h"
#include "mux/server/platform.h"

#include "mux/commands/functions.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"

int nummacros;
int maxmacros;
struct macros **macros;

MACENT macro_table[] = {{"add", do_add_macro},       {"clear", do_clear_macro},
                        {"chmod", do_chmod_macro},   {"chown", do_chown_macro},
                        {"create", do_create_macro}, {"def", do_def_macro},
                        {"del", do_del_macro},       {"name", do_desc_macro},
                        {"chslot", do_edit_macro},   {"ex", do_ex_macro},
                        {"gex", do_gex_macro},       {"glist", do_list_macro},
                        {"list", do_status_macro},   {"undef", do_undef_macro},
                        {(char *)nullptr, nullptr}};

void init_mactab(void) {
  MACENT *mp;

  hash_table_initialize(&mudstate.macro_htab, 5 * HASH_FACTOR);

  for (mp = macro_table; mp->cmdname; mp++)
    hash_table_add(mp->cmdname, (int *)mp, &mudstate.macro_htab);
}

int do_macro(DbRef player, char *in, char **out) {
  char *s;
  char *cmd;
  MACENT *mp;
  char *old;

  cmd = in + 1;

  if (!is_player(player)) {
    notify(player, "MACRO: Only players may use macros.");
    return 0;
  }
  old = alloc_lbuf("do_macro");

  StringCopy(old, in);

  for (s = cmd; *s && *s != ' '; s++)
    ;
  if (*s == ' ')
    *s++ = 0;

  mp = (MACENT *)hash_table_find(cmd, &mudstate.macro_htab);
  if (mp != nullptr) {
    (*(mp->handler))(player, s);
    free_lbuf(old);
    return 0;
  }
  if ((*out = do_process_macro(player, in, s)) != nullptr) {
    free_lbuf(old);
    return 1;
  } else {
    StringCopy(in, old);
    free_lbuf(old);
    return 2; /*
               * return any value > 1, and command * * *
               * processing will
               */
  } /*
     * continue
     */
}

void do_list_macro(DbRef player, char *s) {
  int i;
  int notified = 0;
  struct macros *m;
  char *unparse;

  for (i = 0; i < nummacros; i++) {
    m = macros[i];

    if (can_read_macros(player, m)) {
      if (!notified) {
        notify(player, "Num  Description                         Owner         "
                       "            LRW");
        notified = 1;
      }
      unparse = unparse_object(player, m->player, 0);
      notify_printf(player, "%-4d %-35.35s %-24.24s  %c%c%c", i, m->desc,
                    unparse, m->status & MACRO_L ? 'L' : '-',
                    m->status & MACRO_R ? 'R' : '-',
                    m->status & MACRO_W ? 'W' : '-');
      free_lbuf(unparse);
    }
  }

  if (!notified)
    notify(player, "MACRO: There are no macro sets you can read.");
}

void do_add_macro(DbRef player, char *s) {
  int first;
  int set;
  struct macros *m;
  struct commac *c;
  int i;

  c = get_commac(player);

  first = -1;
  for (i = 0; i < 5 && first < 0; i++)
    if (c->macros[i] == -1)
      first = i;

  if (first < 0) {
    notify(player, "MACRO: Sorry, you already have 5 sets defined on you.");
  } else if (is_number(s)) {
    set = clamped_atoi(s);
    if (set >= 0 && set < nummacros) {
      m = macros[set];
      if (can_read_macros(player, m)) {
        c->macros[first] = set;
        notify_printf(player, "MACRO: Macro set %d added in the %d slot.", set,
                      first);
      } else {
        notify(player, "MACRO: Permission denied.");
      }
    } else {
      notify(player, "MACRO: That macro set does not exist.");
      return;
    }
  } else {
    notify(player, "MACRO: What set do you want to add to your macro system?");
  }
}

void do_del_macro(DbRef player, char *s) {
  struct commac *c;
  int set;

  c = get_commac(player);

  if (is_number(s)) {
    set = clamped_atoi(s);
    if (set >= 0 && set < 5 && c->macros[set] >= 0) {
      c->macros[set] = -1;
      notify_printf(player, "MACRO: Macro slot %d cleared.", set);
      if (set == c->curmac) {
        c->curmac = -1;
        notify(player, "MACRO: Deleted current slot, resetting to none.");
      }
    } else
      notify(player, "MACRO: That is not a legal macro slot.");
  } else
    notify(player,
           "MACRO: What set did you want to delete from your macro system?");
}

void do_desc_macro(DbRef player, char *s) {
  struct macros *m;

  m = get_macro_set(player, -1);
  if (m) {
    free(m->desc);
    m->desc = malloc(strlen(s) + 1);
    StringCopy(m->desc, s);
    notify_printf(player, "MACRO: Current slot description to %s.", s);
  } else
    notify(player, "MACRO: You have no current slot set.");
}

void do_chmod_macro(DbRef player, char *s) {
  struct macros *m;
  int sign;

  m = get_macro_set(player, -1);

  if (m) {
    if ((m->player != player) && !is_wizard(player)) {
      notify(player, "MACRO: Permission denied.");
      return;
    }
    if (*s == '!') {
      sign = 0;
      s++;
    } else
      sign = 1;

    switch (*s) {
    case 'L':
    case 'l':
      if (sign) {
        m->status |= MACRO_L;
        notify(player,
               "MACRO: Default Macro Slot is now locked and unwritable.");
      } else {
        m->status &= ~MACRO_L;
        notify(player, "MACRO: Default Macro Slot is now unlocked.");
      }
      break;
    case 'R':
    case 'r':
      if (sign) {
        m->status |= MACRO_R;
        notify(player,
               "MACRO: Default Macro Slot set to be readable by others");
      } else {
        m->status &= ~MACRO_R;
        notify(player,
               "MACRO: Default Macro Slot set to be not readable by others");
      }
      break;
    case 'W':
    case 'w':
      if (sign) {
        m->status |= MACRO_W;
        notify(player,
               "MACRO: Default Macro Slot set to be writable by others");
      } else {
        m->status &= ~MACRO_W;
        notify(player,
               "MACRO: Default Macro Slot set to be not writable by others");
      }
      break;
    default:
      notify(player, "MACRO: Sorry, unknown mode.  Legal modes are: L R W");
    }
  } else
    notify(player, "MACRO: You have no current slot set.");
}

void do_gex_macro(DbRef player, char *s) {
  struct macros *m;
  int which;
  int i;
  char buffer[LBUF_SIZE];

  if (!s || !*s) {
    notify(player, "MACRO: You need to specify a macro set.");
    return;
  }
  if (is_number(s)) {
    which = clamped_atoi(s);
    if ((which >= nummacros) || (which < 0) || (nummacros == 0)) {
      notify_printf(player,
                    "MACRO: Illegal Macro Set.  Macros go from 0 to %d.",
                    nummacros - 1);
      return;
    } else
      m = macros[which];
  } else {
    notify(player, "MACRO: I do not see that set here.");
    return;
  }

  if (m && can_read_macros(player, m)) {
    notify_printf(player, "Macro Definitions for %s", m->desc);
    for (i = 0; i < m->nummacros; i++) {
      snprintf(buffer, sizeof(buffer), "  %-5.5s: %s", m->alias + i * 5,
               m->string[i]);
      notify(player, buffer);
    }
  } else
    notify(player, "MACRO: Permission denied.");
}

void do_edit_macro(DbRef player, char *s) {
  struct commac *c;
  int set;

  c = get_commac(player);

  if (is_number(s)) {
    set = clamped_atoi(s);
    if (set >= 0 && set < 5 && is_valid_macro_index(c->macros[set])) {
      c->curmac = set;
      notify_printf(player, "MACRO: Current slot set to %d.", set);
    } else
      notify(player, "MACRO: That is not a legal macro slot.");
  } else
    notify(player, "MACRO: What slot did you want to make current?");
}

void do_status_macro(DbRef player, char *s) {
  int i;
  struct commac *c;
  struct macros *m;
  char *unparse;

  c = get_commac(player);

  notify(player, "#: Num  Description                         Owner            "
                 "         LRW");
  for (i = 0; i < 5; i++) {
    if (c->macros[i] >= 0)
      if (!(is_valid_macro_index(c->macros[i])))
        notify_printf(player, "%d: INVALID MACRO SET!", i);
      else {
        m = macros[c->macros[i]];
        unparse = unparse_object(player, m->player, 0);
        notify_printf(
            player, "%d: %-4d %-35.35s %-24.24s  %c%c%c", i, c->macros[i],
            m->desc, unparse, m->status & MACRO_L ? 'L' : '-',
            m->status & MACRO_R ? 'R' : '-', m->status & MACRO_W ? 'W' : '-');
        free_lbuf(unparse);
      }
    else
      notify_printf(player, "%d:", i);
  }
  notify_printf(player, "Current Macro Slot: %d", c->curmac);
}

void do_ex_macro(DbRef player, char *s) {
  struct macros *m;
  int which;
  int i;
  char buffer[LBUF_SIZE];

  if (is_number(s)) {
    which = clamped_atoi(s);
    m = get_macro_set(player, which);
  } else
    m = get_macro_set(player, -1);

  if (m) {
    notify_printf(player, "Macro Definitions for %s", m->desc);
    for (i = 0; i < m->nummacros; i++) {
      snprintf(buffer, sizeof(buffer), "  %-5.5s: %s", m->alias + i * 5,
               m->string[i]);
      notify(player, buffer);
    }
  } else
    notify(player, "MACRO: Illegal macro set to examine.");
}

void do_chown_macro(DbRef player, char *cmd) {
  struct macros *m;
  DbRef thing;
  char *unparse;

  m = get_macro_set(player, -1);
  thing = match_thing(player, cmd);

  if (thing == NOTHING) {
    notify(player, "MACRO: I do not see that here.");
    return;
  }
  if (!m) {
    notify(player, "MACRO: No current active macro.");
    return;
  }
  if (!is_wizard(player)) {
    notify(player, "MACRO: Sorry, command limited to Wizards.");
    return;
  }
  m->player = (int)thing;
  unparse = unparse_object(player, thing, 0);
  notify_printf(player, "MACRO: Macro %s chowned to %s.", m->desc, unparse);
  free_lbuf(unparse);
}

void clear_macro_set(int set) {
  struct macros *m;
  struct commac *c;
  int i, j;

  if (is_valid_macro_index(set)) {
    m = macros[set];
    for (i = 0; i < m->nummacros; i++) {
      free(m->string[i]);
    }
    free(m->alias);
    free(m->string);
    free(m);

    nummacros--;
    for (i = set; i < nummacros; i++)
      macros[i] = macros[i + 1];
    macros[i] = nullptr;
  }
  for (i = 0; i < NUM_COMMAC; i++) {
    c = commac_table[i];
    while (c) {
      for (j = 0; j < 5; j++) {
        if (c->macros[j] == set) {
          c->macros[j] = -1;
          if (c->curmac == j)
            c->curmac = -1;
        } else if (c->macros[j] > set) {
          c->macros[j]--;
        }
      }
      c = c->next;
    }
  }
}

void do_clear_macro(DbRef player, char *s) {
  int set;
  struct macros *m;
  struct commac *c;

  c = get_commac(player);

  if (c->curmac == -1) {
    notify(player, "MACRO: You are not currently editing a macro set.");
    return;
  } else if (c->macros[c->curmac] == -1) {
    notify(player, "MACRO: That is not a valid macro set.");
    return;
  }
  set = c->macros[c->curmac];
  m = macros[set];

  if (is_valid_macro_index(set)) {
    if ((player != m->player) && !is_wizard(player)) {
      notify(player, "MACRO: You may only CLEAR your own macro sets.");
      return;
    } else if ((player == m->player) && (m->status & MACRO_L)) {
      notify(player, "MACRO: Sorry, that macro set is locked.");
      return;
    }
  }
  notify_printf(player, "MACRO: Clearing macro set %d: %s.", set,
                is_valid_macro_index(set) ? m->desc : "Nonexistent");
  clear_macro_set(set);
}

void do_def_macro(DbRef player, char *cmd) {
  int i, j, where;
  struct macros *m;
  char *alias;
  char *s;
  char buffer[LBUF_SIZE];
  char **ns;
  char *na;

  m = get_macro_set(player, -1);

  if (!m) {
    notify(player, "MACRO: No current set.");
    return;
  }
  if (!can_write_macros(player, m)) {
    notify(player, "MACRO: Permission denied.");
    return;
  }
  for (alias = cmd; *alias && *alias == ' '; alias++)
    *alias = 0;

  cmd = alias;
  for (; *cmd && *cmd != ' ' && *cmd != '='; cmd++)
    ;
  while (*cmd && *cmd == ' ')
    *cmd++ = '\0';
  if (*cmd != '=') {
    notify(player, "MACRO: You must specify an = in your macro definition");
    return;
  }
  *cmd++ = 0;
  while (*cmd && *cmd == ' ')
    *cmd++ = 0;

  s = cmd;

  if (!*s) {
    notify(player, "MACRO: You must specify a string to substitute for.");
    return;
  } else if (strlen(alias) > 4) {
    notify(player, "MACRO: Please limit aliases to 4 chars or less.");
    return;
  }
  for (j = 0; j < m->nummacros && (strcasecmp(alias, m->alias + j * 5) > 0);
       j++)
    ;
  if (j < m->nummacros && !strcasecmp(alias, m->alias + j * 5)) {
    notify(player, "MACRO: That alias is already defined in this set.");
    snprintf(buffer, sizeof(buffer), "%-4.4s:%s", m->alias + j * 5,
             m->string[j]);
    notify(player, buffer);
    return;
  }
  if (m->nummacros >= m->maxmacros) {
    m->maxmacros += 10;
    na = malloc(5 * (size_t)m->maxmacros);
    ns = malloc(sizeof(char *) * (size_t)m->maxmacros);

    for (i = 0; i < m->nummacros; i++) {
      StringCopy(na + i * 5, m->alias + i * 5);
      ns[i] = m->string[i];
    }
    free(m->alias);
    free(m->string);
    m->alias = na;
    m->string = ns;
  }
  where = m->nummacros++;
  for (i = where; i > j; i--) {
    StringCopy(m->alias + i * 5, m->alias + (i - 1) * 5);
    m->string[i] = m->string[i - 1];
  }

  where = j;
  StringCopy(m->alias + where * 5, alias);
  m->string[where] = malloc(strlen(s) + 1);
  StringCopy(m->string[where], s);
  snprintf(buffer, sizeof(buffer), "MACRO: Macro %s:%s defined.", alias, s);
  notify(player, buffer);
}

void do_undef_macro(DbRef player, char *cmd) {
  int i;
  struct macros *m;

  m = get_macro_set(player, -1);

  if (!m || !can_write_macros(player, m)) {
    notify(player, "MACRO: Permission denied.");
    return;
  }
  for (i = 0; i < m->nummacros; i++) {
    if (!strcmp(m->alias + i * 5, cmd)) {
      free(m->string[i]);
      m->nummacros--;
      for (; i < m->nummacros; i++) {
        StringCopy(m->alias + i * 5, m->alias + i * 5 + 5);
        m->string[i] = m->string[i + 1];
      }
      notify(player, "MACRO: Macro deleted from set.");
      return;
    }
  }
  notify(player, "MACRO: That macro is not in this set.");
}

char *do_process_macro(DbRef player, char *in, char *s) {
  char *tar;
  char *next;
  struct macros *m;
  int first, last, current = 0;
  int dir;
  int i;
  struct commac *c;
  char *buff;

  c = get_commac(player);
  buff = alloc_lbuf("do_process_macro");
  buff[0] = '\0'; /*
                   * End the string
                   */
  for (i = 0; i < 5; i++) {
    if (is_valid_macro_index(c->macros[i])) {
      m = macros[c->macros[i]];
      if (m->nummacros > 0) {
        first = 0;
        last = m->nummacros - 1;
        dir = 1;
        next = in + 1;
        while (dir && (first <= last)) {
          current = (first + last) / 2;
          dir = strcmp(next, m->alias + 5 * current);
          if (dir < 0)
            last = current - 1;
          else
            first = current + 1;
        }

        if (!dir) {
          tar = m->string[current];
#if 1 /* Original MUSE code */
          next = buff;
          while (*tar) {
            if (*tar == '%' && *(tar + 1) == '*') {
              *next++ = '*';
              tar += 2;
            } else if (*tar == '*') {
              *next = 0;
              strlcat(next, s, (size_t)(LBUF_SIZE - (next - buff)));
              tar++;
              next += strlen(next);
            } else
              *next++ = *tar++;
          }
          *next = 0;
#else
          while (*tar) {
            switch (*tar) {
            case '*':
              if (!buff)
                StringCopy(buff, s);
              else
                strcat(buff, s);
              break;
            case '%':
              if (!buff)
                StringCopy(buff, tar + 1);
              else
                sprintf(buff, "%s%c", buff, tar + 1);
              *tar++;
              break;
            default:
              if (!buff)
                StringCopy(buff, tar);
              else
                sprintf(buff, "%s%c", buff, *tar);
              break;
            }
            *tar++;
          }
#endif
          return buff;
        }
      }
    }
  }
  free_lbuf(buff);
  return nullptr;
}

struct macros *get_macro_set(DbRef player, int which) {
  int set;
  struct commac *c;

  c = get_commac(player);

  if (c) {
    set = -1;
    if (which >= 0 && which < 5)
      set = c->macros[which];
    else if (c->curmac >= 0)
      set = c->macros[c->curmac];

    if (set == -1)
      return nullptr;
    else
      return macros[set];
  } else
    return nullptr;
}

void do_create_macro(DbRef player, char *s) {
  int first;
  int i;
  struct commac *c;
  struct macros **nm;
  int set;

  c = get_commac(player);
  first = -1;
  for (i = 0; i < 5 && first < 0; i++)
    if (c->macros[i] == -1)
      first = i;
  if (first < 0) {
    notify(player, "MACRO: Sorry, you already have 5 sets defined on you.");
    return;
  }
  if (nummacros >= maxmacros) {
    maxmacros += 10;
    nm = (struct macros **)malloc(sizeof(struct macros *) * (size_t)maxmacros);

    for (i = 0; i < nummacros; i++)
      nm[i] = macros[i];
    free(macros);
    macros = nm;
  }
  set = nummacros++;
  macros[set] = (struct macros *)malloc(sizeof(struct macros));

  macros[set]->player = (int)player;
  macros[set]->status = 0;
  macros[set]->nummacros = 0;
  macros[set]->maxmacros = 0;
  macros[set]->alias = nullptr;
  macros[set]->string = nullptr;
  macros[set]->desc = malloc(strlen(s) + 1);
  StringCopy(macros[set]->desc, s);
  c->curmac = first;
  c->macros[first] = set;

  notify_printf(player, "MACRO: Macro set %d created with description %s.", set,
                s);
}

int can_write_macros(DbRef player, struct macros *m) {
  if (m->status & MACRO_L)
    return 0;

  if (m->player == player)
    return 1;
  else
    return m->status & MACRO_W;
}

int can_read_macros(DbRef player, struct macros *m) {
  if (is_wizard(player))
    return 1;

  if (!m)
    return 0;

  if (m->player == player)
    return 1;
  else
    return m->status & MACRO_R;
}
