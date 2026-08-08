
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
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech_text_builder.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"

typedef struct EvaluationContext EvaluationContext;

void KillText(char **lines, size_t count);
void ShowText(EvaluationContext *evaluation, char **lines, size_t count,
              DbRef player);

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

static void display_line(BtechTextBuilder *output, int *len, CoolMenu *m) {
  (void)m;

  btech_text_builder_append(output, "[fg=blue]");
  for (int i = 0; i < *len; i++)
    btech_text_builder_append_character(output, '-');
  btech_text_builder_append(output, "[reset]");
  *len = 0;
}

static int compute_length(const char *text) {
  return (int)styled_text_width(nullptr, text);
}

static void display_string(BtechTextBuilder *output, int *len, CoolMenu *m) {
  char truncated[LBUF_SIZE];
  int visible = compute_length(m->text);
  int available = maximum_int(*len - 1, 0);
  int copied_width = minimum_int(visible, available);
  int p;
  int i;

  if (m->flags & CM_NOCUT) {
    *len = 1;
    btech_text_builder_append(output, m->text);
    return;
  }
  styled_text_truncate(nullptr, m->text, (size_t)copied_width, truncated,
                       sizeof(truncated));
  if (m->flags & CM_CENTER) {
    p = maximum_int((*len - copied_width) / 2, 0);
    for (i = 0; i < p; i++)
      btech_text_builder_append_character(output, ' ');
    btech_text_builder_append(output, "[fg=blue bold]");
    btech_text_builder_append(output, truncated);
    btech_text_builder_append(output, "[reset]");
    *len -= p + copied_width;
  } else {
    btech_text_builder_append(output, truncated);
    *len -= copied_width;
  }
}

static void display_toggle_end(BtechTextBuilder *output, CoolMenu *m) {
  if (m->value)
    btech_text_builder_append_format(
        output, " %s<[fg=blue]X[reset][bold]>[reset]",
        !(m->flags & CM_NO_HILITE) ? "[bold]" : "");
  else
    btech_text_builder_append(output, " < >");
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
    } while (v > 999 && *checked_string_suffix(suffixes, (size_t)i));

    if (!*checked_string_suffix(suffixes, (size_t)i))
      i--;
    snprintf(result.text, sizeof(result.text), "%d%c", BOUNDED(0, v, 999),
             *checked_string_suffix(suffixes, (size_t)i));
  } else
    snprintf(result.text, sizeof(result.text), "%d", BOUNDED(0, v, 999));
  return result;
}

static void display_number_end(BtechTextBuilder *output, CoolMenu *m) {
  if (m->value >= 0) {
    btech_text_builder_append_format(
        output, " [fg=green]%s%4s[reset]",
        (m->value > 0 && !(m->flags & CM_NO_HILITE)) ? "[bold]" : "",
        stringified_value(m->value).text);
  } else
    btech_text_builder_append(output, " ____");
}

static void display_entry(BtechTextBuilder *output, int maxlen, CoolMenu *c) {
  int i, j = 0, t = 0;

  /* returns: number of characters to forward the main pointer with.
     basically: strlen(ouradditions) */
  if ((c->flags & (LETTERFIRST)) && !(c->flags & CM_NOTOG)) {
    if (c->flags & CM_NUMBER)
      maxlen -= 5;
    else
      maxlen -= 4;
    t = ((c->flags & (CM_TOGGLE | CM_NUMBER)) && c->value);
    btech_text_builder_append_format(
        output, "%s[%c]%s ",
        (t && !(c->flags & CM_NO_HILITE)) ? "[fg=red bold]" : "[fg=red]",
        t ? (c->letter + 'A' - 'a') : c->letter, "[reset]");
  }
  if (c->flags & (RIGHTEDGES) && !(c->flags & CM_NORIGHT)) {
    if (c->flags & CM_NUMBER)
      maxlen -= 6;
    else
      maxlen -= 5;
    j = 1;
  }
  if (t && !(c->flags & (CM_NO_HILITE))) {
    btech_text_builder_append(output, "[bold]");
  }
  if (c->flags & CM_LINE)
    display_line(output, &maxlen, c);
  else
    display_string(output, &maxlen, c);
  if (t && !(c->flags & (CM_NO_HILITE))) {
    btech_text_builder_append(output, "[reset]");
  }
  if (maxlen > 0 && !(c->flags & CM_NOCUT)) {
    for (i = 0; i < maxlen; i++)
      btech_text_builder_append_character(output, ' ');
  }
  if (j) {
    if (c->flags & CM_TOGGLE)
      display_toggle_end(output, c);
    else if (c->flags & CM_NUMBER)
      display_number_end(output, c);
    btech_text_builder_append_character(output, ' ');
  }
}

static void display_entries(CoolMenu *c, int wnum, int num, char *text) {
  int i;
  int single_length = (MENU_CHAR_WIDTH / wnum);
  BtechTextBuilder output;
  btech_text_builder_initialize(&output, text, MAX_MENU_WIDTH);

  for (i = 0; i < num; i++) {
    display_entry(&output, single_length, c);
    c = c->next;
  }
}

char **MakeCoolMenuText(CoolMenu *c, size_t *line_count) {
  char **m;
  int pos = 0;
  int n, rn;

  Create(m, char *, MAX_MENU_LENGTH + 1);

  /* Whole whopping menu is ready to be written at.. */
  while (c)
    if ((n = number_of_entries(c)))
      if ((rn = count_following_with(c, n))) {
        char *line;
        Create(line, char, MAX_MENU_WIDTH);
        char **line_slot =
            checked_storage_at(m, MAX_MENU_LENGTH + 1, sizeof(*m), (size_t)pos);
        *line_slot = line;

        /* 	  display_entries(c,rn,m[pos++]); */
        display_entries(c, n, rn, line);
        pos++;
        while (rn > 0 && c) {
          rn--;
          c = c->next;
        }
      }
  *line_count = (size_t)pos;
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
  size_t line_count = 0;
  char **ch = MakeCoolMenuText(c, &line_count);
  ShowText(evaluation, ch, line_count, player);
  KillText(ch, line_count);
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

CoolMenu *SelCol_Menu(int columns, char *heading, char *const *strings,
                      size_t string_count, int type, int max) {
  CoolMenu *c = NULL;
  int i, co = 0;
  char buf[LBUF_SIZE];

  strlcpy(buf, heading, sizeof(buf));
  buf[0] = ascii_to_upper(buf[0]);
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  cool_menu_entry_simple(&c, buf, CM_ONE | CM_CENTER);
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  co = (int)string_count;
  if (columns < 0)
    columns = CoolMenu_FPWBit(co, 18);
  for (i = 0; i < co; i++) {
    const char *entry = *(char *const *)checked_storage_at_const(
        strings, string_count, sizeof(*strings), (size_t)i);
    cool_menu_entry_normal(&c, entry, columns | type, i + 1, max);
  }
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  return c;
}

CoolMenu *SelCol_ConstMenu(int columns, const char *heading,
                           const char *const strings[], size_t string_count,
                           int type, int max) {
  CoolMenu *c = nullptr;
  int count = 0;
  char heading_buffer[LBUF_SIZE];

  strlcpy(heading_buffer, heading, sizeof(heading_buffer));
  heading_buffer[0] = ascii_to_upper(heading_buffer[0]);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  cool_menu_entry_simple(&c, heading_buffer, CM_ONE | CM_CENTER);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  count = (int)string_count;
  if (columns < 0)
    columns = CoolMenu_FPWBit(count, 18);
  for (int index = 0; index < count; index++) {
    const char *entry = *(const char *const *)checked_storage_at_const(
        strings, string_count, sizeof(*strings), (size_t)index);
    cool_menu_entry_normal(&c, entry, columns | type, index + 1, max);
  }
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  return c;
}

CoolMenu *SelCol_FunStringMenuK(int columns, char *heading, char *(*fun)(int),
                                int last) {
  CoolMenu *c = NULL;
  int i;
  char buf[LBUF_SIZE];
  int sick = 0;

  strlcpy(buf, heading, sizeof(buf));
  buf[0] = ascii_to_upper(buf[0]);
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

  strlcpy(buf, heading, sizeof(buf));
  buf[0] = ascii_to_upper(buf[0]);
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
