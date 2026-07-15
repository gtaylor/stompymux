
/* comsys.h - Channel-system types, commands, and function declarations. */

#pragma once

#include <time.h>

#include "mux/communication/commac.h"
#include "mux/database/db.h"
#include "mux/network/descriptor.h"
#ifdef CHANNEL_HISTORY
#include "mux/support/fifo.h"
#endif

typedef struct chanentry CHANENT;
struct chanentry {
  char *channame;
  struct channel *chan;
};

#define CHAN_NAME_LEN 50
struct comuser {
  DbRef who;
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
  Fifo *last_messages;
#endif
  int num_messages;
};

typedef struct {
  time_t time;
  char *msg;
} chmsg;

extern int num_channels;
extern int max_channels;

int is_in_character_location(DbRef player);
void init_chantab(void);
void send_channel(char *chan, const char *format, ...);
struct channel *select_channel(char *channel);
struct comuser *select_user(struct channel *ch, DbRef player);
void do_addcom(DbRef player, DbRef cause, int key, char *arg1, char *arg2);
void do_delcom(DbRef player, DbRef cause, int key, char *arg1);
void do_createchannel(DbRef player, DbRef cause, int key, char *channel);
void do_destroychannel(DbRef player, DbRef cause, int key, char *channel);
void do_comtitle(DbRef player, DbRef cause, int key, char *arg1, char *arg2);
void do_comlist(DbRef player, DbRef cause, int key);
void do_channelnuke(DbRef player);
void do_clearcom(DbRef player, DbRef cause, int key);
void do_allcom(DbRef player, DbRef cause, int key, char *arg1);
void do_channelwho(DbRef player, DbRef cause, int key, char *arg1);
void do_comdisconnect(DbRef player);
void do_comconnect(DbRef player, Descriptor *d);
void do_editchannel(DbRef player, DbRef cause, int flag, char *arg1,
                    char *arg2);
int do_comsystem(DbRef who, char *cmd);
void do_cemit(DbRef player, DbRef cause, int key, char *chan, char *text);
void do_chopen(DbRef player, DbRef cause, int key, char *chan, char *object);
void do_chboot(DbRef player, DbRef cause, int key, char *channel, char *victim);
void do_chanstatus(DbRef player, DbRef cause, int key, char *chan);
void do_chanlist(DbRef player, DbRef cause, int key);
void do_joinchannel(DbRef player, struct channel *ch);
void fun_cemit(char *buff, char **bufc, DbRef player, DbRef cause,
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
