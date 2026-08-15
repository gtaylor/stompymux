/*
 * netcommon.c
 */

/*
 * This file contains routines used by the networking code that do not
 * depend on the implementation of the networking code.  The network-specific
 * portions of the descriptor data structure are not used.
 */

#include <arpa/inet.h>
#include <netinet/in.h>
#include <stdio.h>
#include <sys/socket.h>

#include "mux/network/connection_commands.h"
#include "mux/network/descriptor.h"
#include "mux/network/network_output.h"
#include "mux/network/site_access.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mux/world/player.h"
#include "mux/world/world_context.h"

struct sockaddr_storage;

int site_data_check(struct sockaddr_storage *saddr,
                    int address_length [[maybe_unused]], SiteData *site_list) {
  SiteData *this;
  for (this = site_list; this; this = this->next) {
    if ((((struct sockaddr_in *)saddr)->sin_addr.s_addr & this->mask.s_addr) ==
        this->address.s_addr) {
      return this->flag;
    }
  }
  return 0;
}

/*
 * --------------------------------------------------------------------------
 * * list_sites: Display information in a site list
 */

static constexpr int S_SUSPECT = 1;
static constexpr int S_ACCESS = 2;

typedef struct SiteStatusRequest {
  int type;
  int flag;
} SiteStatusRequest;

static const char *stat_string(const SiteStatusRequest *request) {
  const char *str;

  switch (request->type) {
  case S_SUSPECT:
    if (request->flag)
      str = "Suspected";
    else
      str = "Trusted";
    break;
  case S_ACCESS:
    switch (request->flag) {
    case H_FORBIDDEN:
      str = "Forbidden";
      break;
    case 0:
      str = "Unrestricted";
      break;
    default:
      str = "Strange";
    }
    break;
  default:
    str = "Strange";
  }
  return str;
}

static void list_sites(EvaluationContext *evaluation, DbRef player,
                       SiteData *site_list, const char *header_txt,
                       int stat_type) {
  char buff[MBUF_SIZE];
  char address[INET_ADDRSTRLEN];
  char mask[INET_ADDRSTRLEN];
  const char *str;
  SiteData *this;

  (void)snprintf(buff, MBUF_SIZE, "----- %s -----", header_txt);
  notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
  notify_checked(evaluation, player, player,
                 "Address              Mask                 Status",
                 MSG_ME_ALL | MSG_F_DOWN);
  for (this = site_list; this; this = this->next) {
    str = stat_string(
        &(SiteStatusRequest){.type = stat_type, .flag = this->flag});
    if (inet_ntop(AF_INET, &this->address, address, sizeof(address)) == nullptr)
      (void)string_copy_bounded(address, sizeof(address), "<invalid>");
    if (inet_ntop(AF_INET, &this->mask, mask, sizeof(mask)) == nullptr)
      (void)string_copy_bounded(mask, sizeof(mask), "<invalid>");
    (void)snprintf(buff, MBUF_SIZE, "%-20s %-20s %s", address, mask, str);
    notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * list_siteinfo: List information about specially-marked sites.
 */

void list_siteinfo(EvaluationContext *evaluation,
                   AccessControlStore *access_control, DbRef player) {
  list_sites(evaluation, player, access_control->access_sites, "Site Access",
             S_ACCESS);
  list_sites(evaluation, player, access_control->suspect_sites,
             "Suspected Sites", S_SUSPECT);
}

/*
 * ---------------------------------------------------------------------------
 * * make_ulist: Make a list of connected user numbers for the LWHO function.
 */

void make_ulist(GameDatabase *database, DescriptorRegistry *descriptors,
                DbRef player, char *buff, char **bufc) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_connected(descriptors);
  char *cp;

  cp = *bufc;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (!is_wizard(database, player) && is_hidden(database, d->player))
      continue;
    if (cp != *bufc)
      safe_chr(' ', buff, bufc);
    safe_chr('#', buff, bufc);
    safe_tprintf_str(buff, bufc, "%ld", d->player);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * find_connected_name: Resolve a playername from the list of connected
 * * players using prefix matching.  We only return a match if the prefix
 * * was unique.
 */

DbRef find_connected_name(GameDatabase *database,
                          DescriptorRegistry *descriptors, DbRef player,
                          const char *name) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_connected(descriptors);
  DbRef found;

  found = NOTHING;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (is_good_obj(database, player) && !is_wizard(database, player) &&
        is_hidden(database, d->player))
      continue;
    if (!string_prefix(game_object_pure_name(database, d->player), name))
      continue;
    if ((found != NOTHING) && (found != d->player))
      return NOTHING;
    found = d->player;
  }
  return found;
}

void descriptor_run_command(Descriptor *d, char *command) {
  if (!is_wizard(descriptor_runtime(d)->world->database, d->player)) {
    if (d->quota <= 0) {
      descriptor_queue_string(d, "quota exceed, dropping command.\n");
      return;
    }
    d->quota--;
  }
  descriptor_command(d, command);
}
