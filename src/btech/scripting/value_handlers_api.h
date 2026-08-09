/* Declares the BattleTech value handlers API. */

#pragma once

#include "mux/server/platform.h"

#include "mux/commands/command_context.h"

/* values.c */
char *mechIDfunc(Mech *mech, char buffer[static LBUF_SIZE]);
char *mechTypefunc(int mode, Mech *mech, char *arg);
char *mechMovefunc(int mode, Mech *mech, char *arg);
char *mechTechTimefunc(Mech *mech, char buffer[static LBUF_SIZE]);
void apply_mechDamage(Mech *omech, char *buf);
char *mechDamagefunc(int mode, Mech *mech, char *arg,
                     char buffer[static LBUF_SIZE]);
char *mechCentBearingfunc(Mech *mech, char buffer[static LBUF_SIZE]);
char *mechCentDistfunc(Mech *mech, char buffer[static LBUF_SIZE]);
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
