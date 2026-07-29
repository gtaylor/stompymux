
/*
 * $Id: coolmenu.c,v 1.1 2005/06/13 20:50:49 murrayma Exp $
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

#include "mux/objects/db.h"
#include "mux/support/alloc.h"
#include "mux/support/styled_text.h"
#include <stdio.h>
#include <string.h>

void KillText(char **mapt);
void ShowText(EvaluationContext *evaluation, char **mapt, DbRef player);

/*
   Simple menu system for cool menus ;-)
   */
#include "coolmenu.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/objects/db.h"

#ifndef MIN
#define MIN(a, b) ((a) < (b) ? (a) : (b))
#endif
#ifndef MAX
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#endif

int BOUNDED(int, int, int);

int number_of_entries(coolmenu *c) {
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

int count_following_with(coolmenu *c, int num) {
  int count = 0;

  for (; c && number_of_entries(c) >= num && count < num; c = c->next)
    count++;
  return count;
}

void display_line(char **c, int *len, coolmenu *m) {
  char *ch = *c;
  int i;

  (void)m;

  snprintf(ch, *len, "[fg=blue]");
  ch += strlen(ch);
  for (i = 0; i < *len; i++)
    *(ch++) = '-';
  snprintf(ch, *len, "[reset]");
  ch += strlen(ch);
  *len = 0;
  *c = ch;
}

static int compute_length(const char *text) {
  return (int)styled_text_width(nullptr, text);
}

void display_string(char **c, int *len, coolmenu *m) {
  char truncated[LBUF_SIZE];
  int visible = compute_length(m->text);
  int available = MAX(*len - 1, 0);
  int copied_width = MIN(visible, available);
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
    p = MAX((*len - copied_width) / 2, 0);
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

void display_toggle_end(char **c, int maxlen, coolmenu *m) {
  if (m->value)
    snprintf(*c, maxlen, " %s<[fg=blue]X[reset][bold]>[reset]",
             !(m->flags & CM_NO_HILITE) ? "[bold]" : "");
  else
    snprintf(*c, maxlen, " < >");
  *c += strlen(*c);
}

/* Turn value into equivalent with kilo, mega, giga, tera, peta, exa, zetta
   or yotta postfix. */
StringifiedValue stringified_value(int v) {
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

void display_number_end(char **c, int maxlen, coolmenu *m) {
  if (m->value >= 0) {
    snprintf(*c, maxlen, " [fg=green]%s%4s[reset]",
             (m->value > 0 && !(m->flags & CM_NO_HILITE)) ? "[bold]" : "",
             stringified_value(m->value).text);
  } else
    snprintf(*c, maxlen, " ____");
  *c += strlen(*c);
}

char *display_entry(char *ch, int maxlen, coolmenu *c) {
  int i, j = 0, t = 0;

  /* returns: number of characters to forward the main pointer with.
     basically: strlen(ouradditions) */
  if ((c->flags & (LETTERFIRST)) && !(c->flags & CM_NOTOG)) {
    if (c->flags & CM_NUMBER)
      maxlen -= 5;
    else
      maxlen -= 4;
    t = ((c->flags & (CM_TOGGLE | CM_NUMBER)) && c->value);
    snprintf(ch, maxlen, "%s[%c]%s ",
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
    snprintf(ch, maxlen, "[bold]");
    ch += strlen(ch);
  }
  if (c->flags & CM_LINE)
    display_line(&ch, &maxlen, c);
  else
    display_string(&ch, &maxlen, c);
  if (t && !(c->flags & (CM_NO_HILITE))) {
    snprintf(ch, maxlen, "[reset]");
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

void display_entries(coolmenu *c, int wnum, int num, char *text) {
  int i;
  char *ch = text;
  int single_length = (MENU_CHAR_WIDTH / wnum);

  for (i = 0; i < num; i++) {
    ch = display_entry(ch, single_length, c);
    c = c->next;
  }
}

char **MakeCoolMenuText(coolmenu *c) {
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

void CreateMenuEntry_Killer(coolmenu **c, char *text, int flag, int id,
                            int value, int maxvalue) {
  coolmenu *d, *e;
  char first = 'a';

  if (!*c) {
    Create(*c, coolmenu, 1);
    d = *c;
  } else {
    for (d = *c; d->next; d = d->next)
      ;
    Create(d->next, coolmenu, 1);
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

void KillCoolMenu(coolmenu *c) {
  coolmenu *d;

  for (; c; c = d) {
    d = c->next;
    if (c->text)
      free((void *)c->text);
    free((void *)c);
  }
}

void ShowCoolMenu(EvaluationContext *evaluation, DbRef player, coolmenu *c) {
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

coolmenu *SelCol_Menu(int columns, char *heading, char **strings, int type,
                      int max) {
  coolmenu *c = NULL;
  int i, co = 0;
  char buf[LBUF_SIZE];

  strcpy(buf, heading);
  buf[0] = toupper(buf[0]);
  CreateMenuEntry_Simple(&c, NULL, CM_ONE | CM_LINE);
  CreateMenuEntry_Simple(&c, buf, CM_ONE | CM_CENTER);
  CreateMenuEntry_Simple(&c, NULL, CM_ONE | CM_LINE);
  for (co = 0; strings[co]; co++)
    ;
  if (columns < 0)
    columns = CoolMenu_FPWBit(co, 18);
  for (i = 0; i < co; i++)
    CreateMenuEntry_Normal(&c, strings[i], columns | type, i + 1, max);
  CreateMenuEntry_Simple(&c, NULL, CM_ONE | CM_LINE);
  return c;
}

coolmenu *SelCol_FunStringMenuK(int columns, char *heading, char *(*fun)(int),
                                int last) {
  coolmenu *c = NULL;
  int i;
  char buf[LBUF_SIZE];
  int sick = 0;

  strcpy(buf, heading);
  buf[0] = toupper(buf[0]);
  CreateMenuEntry_Simple(&c, NULL, CM_ONE | CM_LINE);
  CreateMenuEntry_Simple(&c, buf, CM_ONE | CM_CENTER);
  if (fun(0)[0] == '[') {
    CreateMenuEntry_Normal(&c, fun(0), columns, 1, 0);
    sick = 1;
  }
  CreateMenuEntry_Simple(&c, NULL, CM_ONE | CM_LINE);
  if (columns < 0)
    columns = CoolMenu_FPWBit(last, 18);
  for (i = sick; i < last; i++)
    CreateMenuEntry_Normal(&c, fun(i), columns, i + 1 - sick, 0);
  CreateMenuEntry_Simple(&c, NULL, CM_ONE | CM_LINE);
  return c;
}

coolmenu *SelCol_FunStringMenuContextK(int columns, char *heading,
                                       char *(*fun)(void *, int, char *buffer),
                                       void *context, int last) {
  coolmenu *c = nullptr;
  int i;
  char buf[LBUF_SIZE];
  char entry[LBUF_SIZE];
  int sick = 0;

  strcpy(buf, heading);
  buf[0] = toupper(buf[0]);
  CreateMenuEntry_Simple(&c, nullptr, CM_ONE | CM_LINE);
  CreateMenuEntry_Simple(&c, buf, CM_ONE | CM_CENTER);
  if (entry[0] == '[') {
    CreateMenuEntry_Normal(&c, entry, columns, 1, 0);
    sick = 1;
  }
  CreateMenuEntry_Simple(&c, nullptr, CM_ONE | CM_LINE);
  if (columns < 0)
    columns = CoolMenu_FPWBit(last, 18);
  for (i = sick; i < last; i++) {
    fun(context, i, entry);
    CreateMenuEntry_Normal(&c, entry, columns, i + 1 - sick, 0);
  }
  CreateMenuEntry_Simple(&c, nullptr, CM_ONE | CM_LINE);
  return c;
}

coolmenu *SelCol_FunStringMenu(int columns, char *heading, char *(*fun)(int)) {
  int co;

  for (co = 0; fun(co); co++)
    ;
  return SelCol_FunStringMenuK(columns, heading, fun, co);
}
