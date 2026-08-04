
/*
   p.coolmenu_h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Feb 22 14:59:35 CET 1999 from menu.c */

#pragma once

#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

/* menu.c */
int number_of_entries(CoolMenu *c);
int count_following_with(CoolMenu *c, int num);
void display_line(char **c, int *len, CoolMenu *m);
void display_string(char **c, int *len, CoolMenu *m);
void display_toggle_end(char **c, CoolMenu *m);
StringifiedValue stringified_value(int value);
void display_number_end(char **c, CoolMenu *m);
char *display_entry(char *ch, int maxlen, CoolMenu *c);
void display_entries(CoolMenu *c, int wnum, int num, char *text);
char **MakeCoolMenuText(CoolMenu *c);
void CreateMenuEntry_Killer(CoolMenu **c, char *text, int flag, int id,
                            int value, int maxvalue);
void KillCoolMenu(CoolMenu *c);
void ShowCoolMenu(EvaluationContext *evaluation, DbRef player, CoolMenu *c);
int CoolMenu_FPWBit(int number, int maxlen);
CoolMenu *SelCol_Menu(int columns, char *heading, char **strings, int type,
                      int max);
CoolMenu *SelCol_FunStringMenuK(int columns, char *heading, char *(*fun)(),
                                int last);
CoolMenu *SelCol_FunStringMenuContextK(int columns, char *heading,
                                       char *(*fun)(void *, int, char *buffer),
                                       void *context, int last);
CoolMenu *SelCol_FunStringMenu(int columns, char *heading, char *(*fun)());
