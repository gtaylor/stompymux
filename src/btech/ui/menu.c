
/*
 * $Id: menu.c,v 1.1 2005/06/13 20:50:49 murrayma Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Mon Sep 16 20:38:36 1996 fingon
 * Last modified: Wed Jun 24 22:41:40 1998 fingon
 *
 */

#include "mux/commands/command_context.h"
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/styled_text/markup.h"

typedef struct EvaluationContext EvaluationContext;

void KillText(char **mapt);
void ShowText(EvaluationContext *evaluation, char **mapt, DbRef player);

/*
   Simple menu system for cool menus ;-)
   */
#include "coolmenu.h"
#include "mux/network/mux_event_alloc.h"

static int minimum_int(int first, int second) {
  return first < second ? first : second;
}

static int maximum_int(int first, int second) {
  return first > second ? first : second;
}

static size_t menu_format_capacity(int length) {
  return length > 0 ? (size_t)length : 0;
}

int BOUNDED(int, int, int);

static int number_of_entries(CoolMenu *c) {
  if (c->flags & CM_ONE)
    return 1;
  if (c->flags & CM_TWO)
    return 2;

  if (c->flags & CM_THREE)
    return 3;
  if (c->flags & CM_FOUR)
    return 4;
  return 1;
}

static int count_following_with(CoolMenu *c, int num) {
  int count = 0;

  for (; c && number_of_entries(c) >= num && count < num; c = c->next)
    count++;
  return count;
}

static void display_line(char **c, int *len, CoolMenu *m) {
  char *ch = *c;
  int i;

  (void)m;

  snprintf(ch, menu_format_capacity(*len), "[fg=blue]");
  ch += strlen(ch);
  for (i = 0; i < *len; i++)
    *(ch++) = '-';
  snprintf(ch, menu_format_capacity(*len), "[reset]");
  ch += strlen(ch);
  *len = 0;
  *c = ch;
}

static int compute_length(const char *text) {
  return (int)styled_text_width(nullptr, text);
}

static void display_string(char **c, int *len, CoolMenu *m) {
  char truncated[LBUF_SIZE];
  int visible = compute_length(m->text);
  int available = maximum_int(*len - 1, 0);
  int copied_width = minimum_int(visible, available);
  int p;
  int i;

  if (m->flags & CM_NOCUT) {
    *len = 1;
    strcpy(*c, m->text);
    *c += strlen(*c);
    return;
  }
  styled_text_truncate(nullptr, m->text, (size_t)copied_width, truncated,
                       sizeof(truncated));
  if (m->flags & CM_CENTER) {
    p = maximum_int((*len - copied_width) / 2, 0);
    for (i = 0; i < p; i++)
      (*c)[i] = ' ';
    *c += p;
    strcpy(*c, "[fg=blue bold]");
    *c += strlen(*c);
    strcpy(*c, truncated);
    *c += strlen(*c);
    strcpy(*c, "[reset]");
    *c += strlen(*c);
    **c = 0;
    *len -= p + copied_width;
  } else {
    strcpy(*c, truncated);
    *c += strlen(*c);
    *len -= copied_width;
  }
}

static void display_toggle_end(char **c, int maxlen, CoolMenu *m) {
  if (m->value)
    snprintf(*c, menu_format_capacity(maxlen),
             " %s<[fg=blue]X[reset][bold]>[reset]",
             !(m->flags & CM_NO_HILITE) ? "[bold]" : "");
  else
    snprintf(*c, menu_format_capacity(maxlen), " < >");
  *c += strlen(*c);
}

/* Turn value into equivalent with kilo, mega, giga, tera, peta, exa, zetta
   or yotta postfix. */
static StringifiedValue stringified_value(int v) {
  const char suffixes[] = "KMGTPEZY";
  int i = -1;
  StringifiedValue result = {0};

  if (v > 999) {
    do {
      i++;
      v /= 1000;
    } while (v > 999 && suffixes[i]);

    if (!suffixes[i])
      i--;
    snprintf(result.text, sizeof(result.text), "%d%c", BOUNDED(0, v, 999),
             suffixes[i]);
  } else
    snprintf(result.text, sizeof(result.text), "%d", BOUNDED(0, v, 999));
  return result;
}

static void display_number_end(char **c, int maxlen, CoolMenu *m) {
  if (m->value >= 0) {
    snprintf(*c, menu_format_capacity(maxlen), " [fg=green]%s%4s[reset]",
             (m->value > 0 && !(m->flags & CM_NO_HILITE)) ? "[bold]" : "",
             stringified_value(m->value).text);
  } else
    snprintf(*c, menu_format_capacity(maxlen), " ____");
  *c += strlen(*c);
}

static char *display_entry(char *ch, int maxlen, CoolMenu *c) {
  int i, j = 0, t = 0;

  /* returns: number of characters to forward the main pointer with.
     basically: strlen(ouradditions) */
  if ((c->flags & (LETTERFIRST)) && !(c->flags & CM_NOTOG)) {
    if (c->flags & CM_NUMBER)
      maxlen -= 5;
    else
      maxlen -= 4;
    t = ((c->flags & (CM_TOGGLE | CM_NUMBER)) && c->value);
    snprintf(ch, menu_format_capacity(maxlen), "%s[%c]%s ",
             (t && !(c->flags & CM_NO_HILITE)) ? "[fg=red bold]" : "[fg=red]",
             t ? (c->letter + 'A' - 'a') : c->letter, "[reset]");
    ch += strlen(ch);
  }
  if (c->flags & (RIGHTEDGES) && !(c->flags & CM_NORIGHT)) {
    if (c->flags & CM_NUMBER)
      maxlen -= 6;
    else
      maxlen -= 5;
    j = 1;
  }
  if (t && !(c->flags & (CM_NO_HILITE))) {
    snprintf(ch, menu_format_capacity(maxlen), "[bold]");
    ch += strlen(ch);
  }
  if (c->flags & CM_LINE)
    display_line(&ch, &maxlen, c);
  else
    display_string(&ch, &maxlen, c);
  if (t && !(c->flags & (CM_NO_HILITE))) {
    snprintf(ch, menu_format_capacity(maxlen), "[reset]");
    ch += strlen(ch);
  }
  if (maxlen > 0 && !(c->flags & CM_NOCUT)) {
    for (i = 0; i < maxlen; i++)
      *(ch++) = ' ';
  }
  if (j) {
    if (c->flags & CM_TOGGLE)
      display_toggle_end(&ch, maxlen, c);
    else if (c->flags & CM_NUMBER)
      display_number_end(&ch, maxlen, c);
    *(ch++) = ' ';
  }
  *ch = 0;
  return ch;
}

static void display_entries(CoolMenu *c, int wnum, int num, char *text) {
  int i;
  char *ch = text;
  int single_length = (MENU_CHAR_WIDTH / wnum);

  for (i = 0; i < num; i++) {
    ch = display_entry(ch, single_length, c);
    c = c->next;
  }
}

char **MakeCoolMenuText(CoolMenu *c) {
  char **m;
  int pos = 0;
  int n, rn;

  Create(m, char *, MAX_MENU_LENGTH + 1);

  /* Whole whopping menu is ready to be written at.. */
  while (c)
    if ((n = number_of_entries(c)))
      if ((rn = count_following_with(c, n))) {
        Create(m[pos], char, MAX_MENU_WIDTH);

        /* 	  display_entries(c,rn,m[pos++]); */
        display_entries(c, n, rn, m[pos++]);
        while (rn > 0 && c) {
          rn--;
          c = c->next;
        }
      }
  return m;
}

void CreateMenuEntry_Killer(CoolMenu **c, const char *text, int flag, int id,
                            int value, int maxvalue) {
  CoolMenu *d, *e;
  char first = 'a';

  if (!*c) {
    Create(*c, CoolMenu, 1);
    d = *c;
  } else {
    for (d = *c; d->next; d = d->next)
      ;
    Create(d->next, CoolMenu, 1);
    d = d->next;
  }
  if (text)
    d->text = strdup(text);
  d->flags = flag;
  if ((flag & LETTERFIRST) && !(flag & CM_NOTOG)) {
    /* gasp, s'pose we need a letter for this thingy */
    for (e = *c; e; e = e->next)
      if (e->letter)
        if (e->letter >= first)
          first = e->letter + 1;
    d->letter = first;
  }
  d->id = id;
  d->value = value;
  d->maxvalue = maxvalue;
}

void KillCoolMenu(CoolMenu *c) {
  CoolMenu *d;

  for (; c; c = d) {
    d = c->next;
    if (c->text)
      free((void *)c->text);
    free((void *)c);
  }
}

void ShowCoolMenu(EvaluationContext *evaluation, DbRef player, CoolMenu *c) {
  char **ch;

  ch = MakeCoolMenuText(c);
  ShowText(evaluation, ch, player);
  KillText(ch);
}

int CoolMenu_FPWBit(int number, int maxlen) {
  if (number <= maxlen)
    return CM_ONE;
  if (number <= (maxlen * 2))
    return CM_TWO;
  if (number <= (maxlen * 3))
    return CM_THREE;
  return CM_FOUR;
}

CoolMenu *SelCol_Menu(int columns, char *heading, char **strings, int type,
                      int max) {
  CoolMenu *c = NULL;
  int i, co = 0;
  char buf[LBUF_SIZE];

  strcpy(buf, heading);
  buf[0] = (char)toupper((unsigned char)buf[0]);
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  cool_menu_entry_simple(&c, buf, CM_ONE | CM_CENTER);
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  for (co = 0; strings[co]; co++)
    ;
  if (columns < 0)
    columns = CoolMenu_FPWBit(co, 18);
  for (i = 0; i < co; i++)
    cool_menu_entry_normal(&c, strings[i], columns | type, i + 1, max);
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  return c;
}

CoolMenu *SelCol_ConstMenu(int columns, const char *heading,
                           const char *const strings[], int type, int max) {
  CoolMenu *c = nullptr;
  int count = 0;
  char heading_buffer[LBUF_SIZE];

  strcpy(heading_buffer, heading);
  heading_buffer[0] = (char)toupper((unsigned char)heading_buffer[0]);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  cool_menu_entry_simple(&c, heading_buffer, CM_ONE | CM_CENTER);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  for (; strings[count]; count++)
    ;
  if (columns < 0)
    columns = CoolMenu_FPWBit(count, 18);
  for (int index = 0; index < count; index++)
    cool_menu_entry_normal(&c, strings[index], columns | type, index + 1, max);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  return c;
}

CoolMenu *SelCol_FunStringMenuK(int columns, char *heading, char *(*fun)(int),
                                int last) {
  CoolMenu *c = NULL;
  int i;
  char buf[LBUF_SIZE];
  int sick = 0;

  strcpy(buf, heading);
  buf[0] = (char)toupper((unsigned char)buf[0]);
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  cool_menu_entry_simple(&c, buf, CM_ONE | CM_CENTER);
  if (fun(0)[0] == '[') {
    cool_menu_entry_normal(&c, fun(0), columns, 1, 0);
    sick = 1;
  }
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  if (columns < 0)
    columns = CoolMenu_FPWBit(last, 18);
  for (i = sick; i < last; i++)
    cool_menu_entry_normal(&c, fun(i), columns, i + 1 - sick, 0);
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  return c;
}

CoolMenu *SelCol_FunStringMenuContextK(int columns, const char *heading,
                                       char *(*fun)(void *, int, char *buffer),
                                       void *context, int last) {
  CoolMenu *c = nullptr;
  int i;
  char buf[LBUF_SIZE];
  char entry[LBUF_SIZE];
  int sick = 0;

  strcpy(buf, heading);
  buf[0] = (char)toupper((unsigned char)buf[0]);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  cool_menu_entry_simple(&c, buf, CM_ONE | CM_CENTER);
  fun(context, 0, entry);
  if (entry[0] == '[') {
    cool_menu_entry_normal(&c, entry, columns, 1, 0);
    sick = 1;
  }
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  if (columns < 0)
    columns = CoolMenu_FPWBit(last, 18);
  for (i = sick; i < last; i++) {
    fun(context, i, entry);
    cool_menu_entry_normal(&c, entry, columns, i + 1 - sick, 0);
  }
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  return c;
}

CoolMenu *SelCol_FunStringMenu(int columns, char *heading, char *(*fun)(int)) {
  int co;

  for (co = 0; fun(co); co++)
    ;
  return SelCol_FunStringMenuK(columns, heading, fun, co);
}
