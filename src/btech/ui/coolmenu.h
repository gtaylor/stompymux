
/*
 * $Id: coolmenu_h,v 1.1 2005/06/13 20:50:52 murrayma Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Mon Sep 16 20:38:54 1996 fingon
 * Last modified: Wed Jun 24 22:37:38 1998 fingon
 *
 */

#pragma once

#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

typedef struct StringifiedValue {
  char text[5];
} StringifiedValue;

/* #define MAX_MENU_LENGTH 24 */
constexpr int MAX_MENU_LENGTH = 400;
constexpr int MAX_MENU_WIDTH = 240;
constexpr int MENU_CHAR_WIDTH = 78;

/*
non-ticked toggle
   [a] ....    < >
ticked toggle
   [A] ....    <X>
value
   [a] ....    ___
string that can be changed
   [a] ...
string
   ...
   */

constexpr int CM_ONE = 0x001;       /* Just one / line */
constexpr int CM_TWO = 0x002;       /* Two / line */
constexpr int CM_THREE = 0x004;     /* Three / line */
constexpr int CM_FOUR = 0x008;      /* Four / line */
constexpr int CM_CENTER = 0x010;    /* Stuff's centered, not left-edge */
constexpr int CM_TOGGLE = 0x020;    /* Field that can be toggled */
constexpr int CM_NUMBER = 0x040;    /* Field with number in it (add/lower) */
constexpr int CM_LINE = 0x080;      /* No text, just blank line */
constexpr int CM_STRING = 0x100;    /* String with letter ahead of it */
constexpr int CM_NO_HILITE = 0x200; /* No extra highlight */
constexpr int CM_NOTOG = 0x400;     /* Not really toggleable */
constexpr int CM_NORIGHT = 0x800;   /* No right-end field */
constexpr int CM_NOCUT = 0x1000;    /* Turn off cutoff */

#define LETTERFIRST (CM_TOGGLE | CM_NUMBER | CM_STRING)
#define RIGHTEDGES (CM_TOGGLE | CM_NUMBER)

typedef struct CoolMenu {
  int id;       /* Used for some purposes by external agency */
  char *text;   /* Text (varies) */
  int value;    /* toggle = 0/1, number=0-999 */
  int maxvalue; /* if maxvalue's < 999 */
  char letter;  /* Letter allocated to this entry */
  int flags;    /* This entry's flags */
  struct CoolMenu *next;
} CoolMenu;

#define CreateMenuEntry_VSimple(c, text)                                       \
  CreateMenuEntry_Normal(c, text, CM_ONE, 0, 999)
#define CreateMenuEntry_Simple(c, text, flag)                                  \
  CreateMenuEntry_Normal(c, text, flag, 0, 999)
#define CreateMenuEntry_Normal(c, text, flag, id, mv)                          \
  CreateMenuEntry_Killer(c, text, flag, id, 0, mv)
void CreateMenuEntry_Killer(CoolMenu **c, char *text, int flag, int id,
                            int value, int maxvalue);

void KillCoolMenu(CoolMenu *c);
void ShowCoolMenu(EvaluationContext *evaluation, DbRef player, CoolMenu *c);
char **MakeCoolMenuText(CoolMenu *c);
int CoolMenu_FPWBit(int number, int maxlen);

/* Automated 'nice' looking menus: */
CoolMenu *SelCol_Menu(int columns, char *heading, char **strings, int type,
                      int max);

/* last = how many entries we have */
CoolMenu *SelCol_FunStringMenuK(int columns, char *heading, char *(*fun)(int),
                                int last);
CoolMenu *SelCol_FunStringMenuContextK(int columns, char *heading,
                                       char *(*fun)(void *, int, char *buffer),
                                       void *context, int last);

/* Same, except we dunno how many entries we got */
CoolMenu *SelCol_FunStringMenu(int columns, char *heading, char *(*fun)(int));

#define AutoCol_Menu(hea, stri, typ) SelCol_Menu(-1, hea, stri, typ, 0)
#define AutoCol_StringMenu(head, str) AutoCol_Menu(head, str, 0)
#define AutoCol_FunStringMenuK(hea, fun, las)                                  \
  SelCol_FunStringMenuK(-1, hea, fun, las)
#define AutoCol_FunStringMenu(hea, fun) SelCol_FunStringMenuK(-1, hea, fun)
#define SelCol_StringMenu(col, head, str) SelCol_Menu(col, head, str, 0, 0)
