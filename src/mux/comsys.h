
#pragma once

/* comsys.h */

/* $Id: comsys.h,v 1.1 2005/06/13 20:50:46 murrayma Exp $ */

#include <stdio.h>
#include <time.h>

#include "commac.h"
#include "db.h"
#include "interface.h"
#ifdef CHANNEL_HISTORY
#include "myfifo/myfifo.h"
#endif

typedef struct chanentry CHANENT;
struct chanentry {
  char *channame;
  struct channel *chan;
};

#define CHAN_NAME_LEN 50
struct comuser {
  dbref who;
  int on;
  char *title;
  struct comuser *on_next;
};

struct channel {
  char name[CHAN_NAME_LEN];
  int type;
  int temp1;
  int temp2;
  int charge;
  int charge_who;
  int amount_col;
  int num_users;
  int max_users;
  int chan_obj;
  struct comuser **users;
  struct comuser *on_users; /* Linked list of who is on */
#ifdef CHANNEL_HISTORY
  myfifo *last_messages;
#endif
  int num_messages;
};

typedef struct {
  time_t time;
  char *msg;
} chmsg;

extern int num_channels;
extern int max_channels;

int In_IC_Loc(dbref player);
void init_chantab(void);
void send_channel(char *chan, const char *format, ...);
void load_comsystem(FILE *fp);
void save_comsystem(FILE *fp);
struct channel *select_channel(char *channel);
struct comuser *select_user(struct channel *ch, dbref player);
void do_addcom(dbref player, dbref cause, int key, char *arg1, char *arg2);
void do_delcom(dbref player, dbref cause, int key, char *arg1);
void do_createchannel(dbref player, dbref cause, int key, char *channel);
void do_destroychannel(dbref player, dbref cause, int key, char *channel);
void do_comtitle(dbref player, dbref cause, int key, char *arg1, char *arg2);
void do_comlist(dbref player, dbref cause, int key);
void do_channelnuke(dbref player);
void do_clearcom(dbref player, dbref cause, int key);
void do_allcom(dbref player, dbref cause, int key, char *arg1);
void do_channelwho(dbref player, dbref cause, int key, char *arg1);
void do_comdisconnect(dbref player);
void do_comconnect(dbref player, DESC *d);
void do_editchannel(dbref player, dbref cause, int flag, char *arg1,
                    char *arg2);
int do_comsystem(dbref who, char *cmd);
void do_cemit(dbref player, dbref cause, int key, char *chan, char *text);
void do_chopen(dbref player, dbref cause, int key, char *chan, char *object);
void do_chboot(dbref player, dbref cause, int key, char *channel, char *victim);
void do_chanstatus(dbref player, dbref cause, int key, char *chan);
void do_chanlist(dbref player, dbref cause, int key);
void do_joinchannel(dbref player, struct channel *ch);
void sort_users(struct channel *ch);
void fun_cemit(char *buff, char **bufc, dbref player, dbref cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs);

#define CHANNEL_JOIN 0x001
#define CHANNEL_TRANSMIT 0x002
#define CHANNEL_RECIEVE 0x004

#define CHANNEL_PL_MULT 0x001
#define CHANNEL_OBJ_MULT 0x010
#define CHANNEL_LOUD 0x100
#define CHANNEL_PUBLIC 0x200
#define CHANNEL_TRANSPARENT 0x400

#define UNDEAD(x)                                                              \
  (((!God(Owner(x))) || !(Going(x))) &&                                        \
   ((Typeof(x) != TYPE_PLAYER) || (Connected(x))))

/* explanation of logic... If it's not owned by god, and it's either not a
player, or a connected player, it's good... If it is owned by god, then if
it's going, assume it's already gone, no matter what it is. :) */
