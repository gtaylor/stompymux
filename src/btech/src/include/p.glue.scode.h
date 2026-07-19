
/*
   p.glue.scode.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Feb 22 14:59:38 CET 1999 from glue.scode.c */

#pragma once

#include "mux/commands/command_context.h"

/* glue.scode.c */
char *mechIDfunc(MECH *mech, char buffer[static LBUF_SIZE]);
char *mechTypefunc(int mode, MECH *mech, char *arg);
char *mechMovefunc(int mode, MECH *mech, char *arg);
char *mechTechTimefunc(MECH *mech, char buffer[static LBUF_SIZE]);
void apply_mechDamage(MECH *omech, char *buf);
char *mechDamagefunc(int mode, MECH *mech, char *arg,
                     char buffer[static LBUF_SIZE]);
char *mechCentBearingfunc(MECH *mech, char buffer[static LBUF_SIZE]);
char *mechCentDistfunc(MECH *mech, char buffer[static LBUF_SIZE]);
void fun_btsetxcodevalue(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context);
void fun_btgetxcodevalue(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context);
void set_xcodestuff(DbRef player, void *data, char *buffer);
void list_xcodestuff(DbRef player, void *data, char *buffer);
void fun_btunderrepair(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context);
void fun_btstores(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context);
void fun_btstores_short(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context);
void fun_btmapterr(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context);
void fun_btmapelev(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context);
void list_xcodevalues(EvaluationContext *context, DbRef player);
void fun_btdesignex(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context);
void fun_btdamages(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context);
void fun_btcritstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context);
void fun_btsectstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context);
void fun_btarmorstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context);
