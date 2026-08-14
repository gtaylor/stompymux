
/* Implements unit repair procedures. */

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "coolmenu.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_events_api.h"
#include "mech_tech_repairs_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mycool.h"
#include "registry_api.h"
#include "repair_job.h"

static void repair_append(char *buffer, size_t capacity, const char *format,
                          ...) __attribute__((format(printf, 3, 4)));

static void repair_append(char *buffer, size_t capacity, const char *format,
                          ...) {
  const size_t USED = strlen(buffer);
  if (USED >= capacity)
    return;
  char *destination =
      checked_storage_at(buffer, capacity, sizeof(*buffer), USED);
  va_list arguments;
  va_start(arguments, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(destination, capacity - USED, format, arguments);
  va_end(arguments);
}

static void describe_repairs(MuxEvent *e, void *menu_context) {
  CoolMenu **menu = (CoolMenu **)menu_context;
  int type = (unsigned char)e->type;
  Mech *mech = (Mech *)e->data;
  long earg = ((long)e->data2) % PLAYERPOS;
  DbRef player = ((long)e->data2) / PLAYERPOS;
  int loc;
  int pos;
  int extra;
  char buf[MBUF_SIZE] = {0};
  char buf2[LBUF_SIZE] = {0};
  int fail = (e->function == mech_event_failure_marker);
  BtechContext *context = mech_context(mech);

  RepairEventPayload payload = repair_event_payload_unpack(earg);
  loc = payload.location;
  pos = payload.position;
  extra = payload.extra;
  (void)snprintf(
      buf, sizeof(buf), "%s%s",
      armor_section_abbreviation(
          &(ArmorSectionReference){.unit_class = mech_class(mech),
                                   .movement_type = mech_movement_type(mech),
                                   .location = loc % 8})
          .text,
      loc >= 8 ? "(R)" : "");
  (void)snprintf(buf2, sizeof(buf2), "%-5ld ", player);
  repair_append(buf2, sizeof(buf2), "%-4d ",
                game_lag_time(context, (e->tick - e->scheduler->tick) / 60));
  switch (type) {
  case EVENT_REPAIR_REPL:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d Replacement of %s", buf,
                  pos + 1, pos_part_name(mech, loc, pos).text);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REPLG:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d Replacement of %s", buf,
                  pos + 1, pos_part_name(mech, loc, pos).text);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REAT:
    repair_append(buf2, sizeof(buf2), "%5s Reattachment", buf);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_RELO:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d %sload of %s", buf, pos + 1,
                  extra ? "Un" : "Re", pos_part_name(mech, loc, pos).text);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_FIX:
    if (fail)
      repair_append(buf2, sizeof(buf2), "%5s:%-2d Failed armor repair", buf, 0);
    else
      repair_append(buf2, sizeof(buf2),
                    "%5s:%-2d Repair of armor - possibly next point", buf, pos);
    break;
  case EVENT_REPAIR_FIXI:
    if (fail)
      repair_append(buf2, sizeof(buf2), "%5s:%-2d Failed internal repair", buf,
                    0);
    else
      repair_append(buf2, sizeof(buf2),
                    "%5s:%-2d Repair of internals - possibly next point", buf,
                    pos);
    break;
  case EVENT_REPAIR_SCRL:
    repair_append(buf2, sizeof(buf2), "%5s Removal", buf);
    break;
  case EVENT_REPAIR_SCRP:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d Scrapping of %s", buf, pos + 1,
                  pos_part_name(mech, loc, pos).text);
    break;
  case EVENT_REPAIR_SCRG:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d Scrapping of %s", buf, pos + 1,
                  pos_part_name(mech, loc, pos).text);
    break;
  case EVENT_REPAIR_REPAG:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d Repair of %s", buf, pos + 1,
                  pos_part_name(mech, loc, pos).text);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REPAP:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d Repair of %s", buf, pos + 1,
                  pos_part_name(mech, loc, pos).text);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REPENHCRIT:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d Repair of %s", buf, pos + 1,
                  pos_part_name(mech, loc, pos).text);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_MOB:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d Mounting of %s", buf, pos + 1,
                  pos_part_name(mech, loc, pos).text);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_UMOB:
    repair_append(buf2, sizeof(buf2), "%5s:%-2d Removing of %s", buf, pos + 1,
                  pos_part_name(mech, loc, pos).text);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REPSUIT:
    repair_append(buf2, sizeof(buf2), "%5s Replacing suit", buf);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  // Added Reseal description
  case EVENT_REPAIR_RESE:
    repair_append(buf2, sizeof(buf2), "%5s Reseal", buf);
    if (fail)
      repair_append(buf2, sizeof(buf2), " (Failure)");
    break;
  }

  cool_menu_entry_very_simple(menu, buf2);
}

void tech_repairs(DbRef player, Mech *mech, char *buffer) {
  char message_buffer[LBUF_SIZE];
  int i;
  CoolMenu *c = nullptr;
  BtechContext *context = mech_context(mech);
  bool is_wizard_player = is_wizard(btech_context_database(context), player);

  if (mech_event_count(mech, EVENT_STARTUP) && !is_wizard_player) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The mech's starting up! Please stop the sequence first.");
    return;
  }
  if (mech_is_started(mech) && !is_wizard_player) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The mech's started up ; please shut it down first.");
    return;
  }
  if (btech_context_limits_repairs_to_stalls(context) &&
      !mech_is_dropship(mech) && mech_repair_stall_dbref(mech) <= 0 &&
      !is_wizard_player) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The 'mech isn't in a repair stall!");
    return;
  }

  if (!figure_latest_tech_event(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This 'mech has no repairs pending!");
    return;
  }
  cool_menu_add_line(&c);
  (void)snprintf(message_buffer, sizeof(message_buffer),
                 "Repairs/Scrapping in progress (%s)",
                 mech_display_id(mech).text);
  cool_menu_add_centered(&c, message_buffer);
  (void)snprintf(message_buffer, sizeof(message_buffer), "%-5s %-4s %s", "Plr",
                 "Time", "Location + Description");
  cool_menu_add_text(&c, message_buffer);
  cool_menu_add_line(&c);
  for (i = FIRST_TECH_EVENT; i <= LAST_TECH_EVENT; i++)
    mech_event_visit(mech, i, describe_repairs, (void *)&c);
  cool_menu_add_line(&c);
  cool_menu_add_text(
      &c, "Note: Time = Time remaining in minutes. Plr = Tech's dbref");
  cool_menu_add_line(&c);
  show_cool_menu(btech_context_evaluation(context), player, c);
  kill_cool_menu(c);
}
