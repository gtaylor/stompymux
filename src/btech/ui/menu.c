
/* Implements terminal menu rendering and interaction. */

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

void kill_text(char **lines, size_t count);
void show_text(EvaluationContext *evaluation, char **lines, size_t count,
               DbRef player);

/*
   Simple menu system for cool menus ;-)
   */
#include "coolmenu.h"

static int minimum_int(int first, int second) {
  return first < second ? first : second;
}

static int maximum_int(int first, int second) {
  return first > second ? first : second;
}

int bounded(int, int, int);

static int menu_column_count(CoolMenu *c) {
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

static int menu_row_entry_count(CoolMenu *c, int column_count) {
  int count = 0;

  for (; c && menu_column_count(c) >= column_count && count < column_count;
       c = c->next)
    count++;
  return count;
}

static void display_line(BtechTextBuilder *output, int *len,
                         CoolMenu *m [[maybe_unused]]) {

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
  const char SUFFIXES[] = "KMGTPEZY";
  int i = -1;
  StringifiedValue result = {0};

  if (v > 999) {
    do {
      i++;
      v /= 1000;
    } while (v > 999 && *checked_string_suffix(SUFFIXES, (size_t)i));

    if (!*checked_string_suffix(SUFFIXES, (size_t)i))
      i--;
    (void)snprintf(result.text, sizeof(result.text), "%d%c", bounded(0, v, 999),
                   *checked_string_suffix(SUFFIXES, (size_t)i));
  } else {
    (void)snprintf(result.text, sizeof(result.text), "%d", bounded(0, v, 999));
  }
  return result;
}

static void display_number_end(BtechTextBuilder *output, CoolMenu *m) {
  if (m->value >= 0) {
    btech_text_builder_append_format(
        output, " [fg=green]%s%4s[reset]",
        (m->value > 0 && !(m->flags & CM_NO_HILITE)) ? "[bold]" : "",
        stringified_value(m->value).text);
  } else {
    btech_text_builder_append(output, " ____");
  }
}

static void display_entry(BtechTextBuilder *output, int maxlen, CoolMenu *c) {
  int i;
  int j = 0;
  int t = 0;

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

typedef struct MenuDisplayEntriesRequest {
  CoolMenu *first_entry;
  int column_count;
  int row_entry_count;
  char *text;
} MenuDisplayEntriesRequest;

static void display_entries(const MenuDisplayEntriesRequest *request) {
  int i;
  CoolMenu *entry = request->first_entry;
  int single_length = MENU_CHAR_WIDTH / request->column_count;
  BtechTextBuilder output;
  btech_text_builder_initialize(&output, request->text, MAX_MENU_WIDTH);

  for (i = 0; i < request->row_entry_count; i++) {
    display_entry(&output, single_length, entry);
    entry = entry->next;
  }
}

char **make_cool_menu_text(CoolMenu *c, size_t *line_count) {
  char **lines;
  int line_index = 0;
  int column_count;
  int row_entry_count;

  lines = (char **)checked_storage_allocate_array(MAX_MENU_LENGTH + 1,
                                                  sizeof(*lines));

  /* Whole whopping menu is ready to be written at.. */
  while (c) {
    column_count = menu_column_count(c);
    if (!column_count) {
      c = c->next;
      continue;
    }
    row_entry_count = menu_row_entry_count(c, column_count);
    if (!row_entry_count) {
      c = c->next;
      continue;
    }
    char *line = checked_storage_allocate_array(MAX_MENU_WIDTH, sizeof(*line));
    char **line_slot = (char **)checked_storage_at(
        (void *)lines, MAX_MENU_LENGTH + 1, sizeof(*lines), (size_t)line_index);
    *line_slot = line;

    display_entries(&(MenuDisplayEntriesRequest){
        .first_entry = c,
        .column_count = column_count,
        .row_entry_count = row_entry_count,
        .text = line,
    });
    line_index++;
    while (row_entry_count > 0 && c) {
      row_entry_count--;
      c = c->next;
    }
  }
  *line_count = (size_t)line_index;
  return lines;
}

void cool_menu_entry_add(const CoolMenuEntryRequest *request) {
  CoolMenu **c = request->menu;
  const char *text = request->text;
  const int FLAG = request->flags;
  CoolMenu *d;
  CoolMenu *e;
  char first = 'a';

  if (!*c) {
    *c = checked_storage_allocate(sizeof(**c));
    d = *c;
  } else {
    for (d = *c; d->next; d = d->next)
      ;
    d->next = checked_storage_allocate(sizeof(*d->next));
    d = d->next;
  }
  if (text)
    d->text = strdup(text);
  d->flags = FLAG;
  if ((FLAG & LETTERFIRST) && !(FLAG & CM_NOTOG)) {
    /* gasp, s'pose we need a letter for this thingy */
    for (e = *c; e; e = e->next)
      if (e->letter)
        if (e->letter >= first)
          first = e->letter + 1;
    d->letter = first;
  }
  d->id = request->id;
  d->value = request->value;
  d->maxvalue = request->maximum_value;
}

void kill_cool_menu(CoolMenu *c) {
  CoolMenu *d;

  for (; c; c = d) {
    d = c->next;
    if (c->text)
      free((void *)c->text);
    free((void *)c);
  }
}

void show_cool_menu(EvaluationContext *evaluation, DbRef player, CoolMenu *c) {
  size_t line_count = 0;
  char **ch = make_cool_menu_text(c, &line_count);
  show_text(evaluation, ch, line_count, player);
  kill_text(ch, line_count);
}

int cool_menu_fpw_bit(int number, int maxlen) {
  if (number <= maxlen)
    return CM_ONE;
  if (number <= (maxlen * 2))
    return CM_TWO;
  if (number <= (maxlen * 3))
    return CM_THREE;
  return CM_FOUR;
}

CoolMenu *cool_menu_selection_create(const CoolMenuSelectionRequest *request) {
  CoolMenu *c = nullptr;
  int count = 0;
  int columns = request->columns;
  char heading_buffer[LBUF_SIZE];

  (void)string_copy_bounded(heading_buffer, sizeof(heading_buffer),
                            request->heading);
  heading_buffer[0] = ascii_to_upper(heading_buffer[0]);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  cool_menu_entry_simple(&c, heading_buffer, CM_ONE | CM_CENTER);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  count = (int)request->string_count;
  if (columns < 0)
    columns = cool_menu_fpw_bit(count, 18);
  for (int index = 0; index < count; index++) {
    const char *entry = *(const char *const *)checked_storage_at_const(
        (const void *)request->strings, request->string_count,
        sizeof(*request->strings), (size_t)index);
    cool_menu_entry_normal(&c, entry, columns | request->entry_type, index + 1,
                           request->maximum_value);
  }
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  return c;
}

CoolMenu *sel_col_fun_string_menu_k(int columns, char *heading,
                                    char *(*fun)(int), int last) {
  CoolMenu *c = nullptr;
  int i;
  char buf[LBUF_SIZE];
  int sick = 0;

  (void)string_copy_bounded(buf, sizeof(buf), heading);
  buf[0] = ascii_to_upper(buf[0]);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  cool_menu_entry_simple(&c, buf, CM_ONE | CM_CENTER);
  if (fun(0)[0] == '[') {
    cool_menu_entry_normal(&c, fun(0), columns, 1, 0);
    sick = 1;
  }
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  if (columns < 0)
    columns = cool_menu_fpw_bit(last, 18);
  for (i = sick; i < last; i++)
    cool_menu_entry_normal(&c, fun(i), columns, i + 1 - sick, 0);
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  return c;
}

CoolMenu *sel_col_fun_string_menu_context_k(int columns, const char *heading,
                                            char *(*fun)(void *, int,
                                                         char *buffer),
                                            void *context, int last) {
  CoolMenu *c = nullptr;
  int i;
  char *buf = alloc_lbuf("sel_col_fun_string_menu_context.heading");
  char *entry = alloc_lbuf("sel_col_fun_string_menu_context.entry");
  int sick = 0;

  (void)string_copy_bounded(buf, LBUF_SIZE, heading);
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
    columns = cool_menu_fpw_bit(last, 18);
  for (i = sick; i < last; i++) {
    fun(context, i, entry);
    cool_menu_entry_normal(&c, entry, columns, i + 1 - sick, 0);
  }
  cool_menu_entry_simple(&c, nullptr, CM_ONE | CM_LINE);
  free_buf(entry);
  free_buf(buf);
  return c;
}

CoolMenu *sel_col_fun_string_menu(int columns, char *heading,
                                  char *(*fun)(int)) {
  int co;

  for (co = 0; fun(co); co++)
    ;
  return sel_col_fun_string_menu_k(columns, heading, fun, co);
}
