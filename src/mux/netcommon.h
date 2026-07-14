#pragma once

#include <netinet/in.h>
#include <sys/time.h>

#include "db.h"
#include "descriptor.h"

void make_portlist(dbref player, dbref target, char *buff, char **bufc);
struct timeval timeval_sub(struct timeval now, struct timeval then);
int msec_diff(struct timeval now, struct timeval then);
struct timeval msec_add(struct timeval time, int milliseconds);
struct timeval update_quotas(struct timeval last, struct timeval current);
void raw_notify_raw(dbref player, const char *message, char *append);
void raw_notify(dbref player, const char *message);
void raw_notify_newline(dbref player);
void raw_broadcast(int inflags, char *template, ...);
void queue_write(DESC *descriptor, const char *buffer, int size);
void queue_string(DESC *descriptor, const char *string);
void welcome_user(DESC *descriptor);
void announce_disconnect(dbref player, DESC *descriptor, const char *reason);
int boot_off(dbref player, char *message);
int boot_by_port(int port, int no_god, char *message);
void run_command(DESC *d, char *command);
int do_command(DESC *d, char *command);
int do_unauth_command(DESC *d, char *command);
void handle_prog(DESC *d, char *message);
void desc_reload(dbref player);
void init_logout_cmdtab(void);
void list_siteinfo(dbref player);
int site_check(struct sockaddr_storage *address, int address_length,
               SITE *site_list);
int fetch_idle(dbref target);
int fetch_connect(dbref target);
void make_ulist(dbref player, char *buff, char **bufc);
dbref find_connected_name(dbref player, char *name);

extern NAMETAB logout_cmdtable[];
