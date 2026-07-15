/* netcommon.h - Shared networking, descriptor, and socket utility interfaces.
 */

#pragma once

#include <netinet/in.h>
#include <sys/time.h>

#include "mux/database/db.h"
#include "mux/network/descriptor.h"
#include "mux/network/program_input.h"
#include "mux/support/name_table.h"

typedef struct SiteData SiteData;

void make_portlist(DbRef player, DbRef target, char *buff, char **bufc);
struct timeval timeval_sub(struct timeval now, struct timeval then);
int msec_diff(struct timeval now, struct timeval then);
struct timeval msec_add(struct timeval time, int milliseconds);
struct timeval update_quotas(struct timeval last, struct timeval current);
void raw_notify_raw(DbRef player, const char *message, char *append);
void raw_notify(DbRef player, const char *message);
void raw_notify_newline(DbRef player);
void raw_broadcast(int inflags, char *template, ...)
    __attribute__((format(printf, 2, 3)));
void descriptor_queue_write(Descriptor *descriptor, const char *buffer,
                            int size);
void descriptor_queue_string(Descriptor *descriptor, const char *string);
void descriptor_welcome(Descriptor *descriptor);
void descriptor_announce_disconnect(DbRef player, Descriptor *descriptor,
                                    const char *reason);
int boot_off(DbRef player, char *message);
int boot_by_port(int port, int no_god, char *message);
void descriptor_run_command(Descriptor *d, char *command);
int descriptor_command(Descriptor *d, char *command);
int descriptor_unauthenticated_command(Descriptor *d, char *command);
void descriptor_reload(DbRef player);
void init_logout_cmdtab(void);
void list_siteinfo(DbRef player);
int site_data_check(struct sockaddr_storage *address, int address_length,
                    SiteData *site_list);
int fetch_idle(DbRef target);
int fetch_connect(DbRef target);
void make_ulist(DbRef player, char *buff, char **bufc);
DbRef find_connected_name(DbRef player, char *name);

extern NameTable logout_cmdtable[];
