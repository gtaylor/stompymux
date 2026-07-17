/* netcommon.h - Shared networking, descriptor, and socket utility interfaces.
 */

#pragma once

#include <netinet/in.h>
#include <sys/time.h>

#include "mux/database/db.h"
#include "mux/network/descriptor.h"

typedef struct SiteData SiteData;

void make_portlist(DbRef player, DbRef target, char *buff, char **bufc);
struct timeval timeval_sub(struct timeval now, struct timeval then);
int msec_diff(struct timeval now, struct timeval then);
struct timeval msec_add(struct timeval time, int milliseconds);
struct timeval update_quotas(struct timeval last, struct timeval current);
void raw_notify_raw(DbRef player, const char *message, const char *append);
void raw_notify(DbRef player, const char *message);
void raw_notify_newline(DbRef player);
void raw_broadcast(int inflags, const char *template, ...)
    __attribute__((format(printf, 2, 3)));
void descriptor_queue_write(Descriptor *descriptor, const char *buffer,
                            int size);
void descriptor_queue_string(Descriptor *descriptor, const char *string);
void descriptor_welcome(Descriptor *descriptor);
void set_lastsite(Descriptor *descriptor, char *lastsite);
void announce_connect(DbRef player, Descriptor *descriptor);
void descriptor_announce_disconnect(DbRef player, Descriptor *descriptor,
                                    const char *reason);
int boot_off(DbRef player, const char *message);
int boot_by_port(int port, int no_god, char *message);
void descriptor_run_command(Descriptor *d, char *command);
int descriptor_command(Descriptor *d, char *command);
void descriptor_reload(DbRef player);
void list_siteinfo(DbRef player);
int site_data_check(struct sockaddr_storage *address, int address_length,
                    SiteData *site_list);
int fetch_idle(DbRef target);
int fetch_connect(DbRef target);
void make_ulist(DbRef player, char *buff, char **bufc);
DbRef find_connected_name(DbRef player, char *name);
Descriptor *descriptor_find_by_fd(int fd);
