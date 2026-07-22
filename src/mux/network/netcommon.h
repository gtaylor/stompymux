/* netcommon.h - Shared networking, descriptor, and socket utility interfaces.
 */

#pragma once

#include <netinet/in.h>
#include <sys/time.h>

#include "mux/network/descriptor.h"
#include "mux/objects/db.h"

typedef struct SiteData SiteData;
typedef struct AccessControlStore AccessControlStore;
typedef struct EvaluationContext EvaluationContext;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerConfiguration ServerConfiguration;

void make_portlist(DescriptorRegistry *descriptors, DbRef player, DbRef target,
                   char *buff, char **bufc);
struct timeval timeval_sub(struct timeval now, struct timeval then);
int msec_diff(struct timeval now, struct timeval then);
struct timeval msec_add(struct timeval time, int milliseconds);
struct timeval update_quotas(const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors,
                             struct timeval last, struct timeval current);
void raw_notify_raw(EvaluationContext *evaluation, DbRef player,
                    const char *message, const char *append);
void raw_notify(EvaluationContext *evaluation, DbRef player,
                const char *message);
void raw_notify_newline(EvaluationContext *evaluation, DbRef player);
void raw_broadcast(DescriptorRegistry *descriptors, int inflags,
                   const char *template, ...)
    __attribute__((format(printf, 3, 4)));
void descriptor_queue_write(Descriptor *descriptor, const char *buffer,
                            int size);
void descriptor_queue_string(Descriptor *descriptor, const char *string);
void descriptor_welcome(Descriptor *descriptor);
void set_lastsite(Descriptor *descriptor, char *lastsite);
void announce_connect(DbRef player, Descriptor *descriptor);
void descriptor_announce_disconnect(DbRef player, Descriptor *descriptor,
                                    const char *reason);
int boot_off(DescriptorRegistry *descriptors, DbRef player,
             const char *message);
int boot_by_port(DescriptorRegistry *descriptors, int port, int no_god,
                 char *message);
void descriptor_run_command(Descriptor *d, char *command);
int descriptor_command(Descriptor *d, char *command);
void descriptor_reload(GameDatabase *database,
                       const ServerConfiguration *configuration,
                       DescriptorRegistry *descriptors, DbRef player);
void list_siteinfo(EvaluationContext *evaluation,
                   AccessControlStore *access_control, DbRef player);
int site_data_check(struct sockaddr_storage *address, int address_length,
                    SiteData *site_list);
int fetch_idle(DescriptorRegistry *descriptors, RuntimeClock *clock,
               DbRef target);
int fetch_connect(DescriptorRegistry *descriptors, RuntimeClock *clock,
                  DbRef target);
void make_ulist(GameDatabase *database, DescriptorRegistry *descriptors,
                DbRef player, char *buff, char **bufc);
DbRef find_connected_name(GameDatabase *database,
                          DescriptorRegistry *descriptors, DbRef player,
                          char *name);
