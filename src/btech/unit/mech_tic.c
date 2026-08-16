/* Implements BattleTech unit mechanics for unit tic. */

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "coolmenu.h"
#include "equipment_types.h"
#include "map.h" // IWYU pragma: keep
#include "map_terrain.h"
#include "mech_build_api.h"
#include "mech_combat_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_heat_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_status_types.h"
#include "mech_tic_api.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static const unsigned long *tic_word(const Mech *mech, int tic, int word) {
  if (tic < 0 || word < 0)
    abort();
  size_t index = ((size_t)tic * TICLONGS) + (size_t)word;
  return checked_storage_at_const(mech->tic,
                                  (size_t)NUM_TICS * (size_t)TICLONGS,
                                  sizeof(unsigned long), index);
}

static unsigned long *tic_word_mutable(Mech *mech, int tic, int word) {
  if (tic < 0 || word < 0)
    abort();
  size_t index = ((size_t)tic * TICLONGS) + (size_t)word;
  return checked_storage_at(mech->tic, (size_t)NUM_TICS * (size_t)TICLONGS,
                            sizeof(unsigned long), index);
}

static void tic_weapon_add(Mech *mech, TicWeaponReference reference) {
  int word = reference.weapon / SINGLE_TICLONG_SIZE;
  unsigned int bit = (unsigned int)reference.weapon % SINGLE_TICLONG_SIZE;
  *tic_word_mutable(mech, reference.tic, word) |= 1UL << bit;
}

static void tic_weapon_remove(Mech *mech, TicWeaponReference reference) {
  int word = reference.weapon / SINGLE_TICLONG_SIZE;
  unsigned int bit = (unsigned int)reference.weapon % SINGLE_TICLONG_SIZE;
  *tic_word_mutable(mech, reference.tic, word) &= ~(1UL << bit);
}

bool mech_tic_contains_weapon(const Mech *mech, TicWeaponReference reference) {
  const int WORD = reference.weapon / SINGLE_TICLONG_SIZE;
  const int BIT = reference.weapon % SINGLE_TICLONG_SIZE;
  return (*tic_word(mech, reference.tic, WORD) & (1UL << (unsigned int)BIT)) !=
         0U;
}
#include "registry_api.h"

typedef struct TicSelectionContext TicSelectionContext;
struct TicSelectionContext {
  int tic;
  int argument_count;
  char **arguments;
};

typedef struct ListTicContext ListTicContext;
struct ListTicContext {
  Mech *mech;
  int tic;
  int weapon_count;
};

/*****************************************************************************/

/* TIC Routines                                                              */

/*****************************************************************************/

static bool cleartic_sub_func(const MultiWeaponSelectionCall *call) {
  int i;
  int j;
  Mech *mech = call->mech;

  for (i = call->first; i <= call->last; i++) {
    for (j = 0; j < TICLONGS; j++)
      *tic_word_mutable(mech, i, j) = 0;
    notify_printf(btech_context_evaluation(mech->xcode.context), call->actor,
                  "TIC #%d cleared!", i);
  }
  return false;
}

void cleartic_sub(DbRef player, Mech *mech, char *buffer) {
  char *args[3];

  if (mech_parseattributes(buffer, args, 3) != 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments to function");
    return;
  }
  multi_weapon_select(&(MultiWeaponSelectionRequest){
      .mech = mech,
      .actor = player,
      .selection = args[0],
      .mode = 2,
      .callback = cleartic_sub_func,
  });
}

static bool addtic_sub_func(const MultiWeaponSelectionCall *call) {
  int i;
  Mech *mech = call->mech;
  const TicSelectionContext *selection = call->context;

  for (i = call->first; i <= call->last; i++) {
    tic_weapon_add(mech,
                   (TicWeaponReference){.tic = selection->tic, .weapon = i});
  }
  if (call->first != call->last)
    notify_printf(btech_context_evaluation(mech->xcode.context), call->actor,
                  "Weapons #%d - #%d added to TIC %d!", call->first, call->last,
                  selection->tic);
  else
    notify_printf(btech_context_evaluation(mech->xcode.context), call->actor,
                  "Weapon #%d added to TIC %d!", call->first, selection->tic);
  return false;
}

void addtic_sub(DbRef player, Mech *mech, char *buffer) {
  int ticnum;
  char *args[3];
  TicSelectionContext selection;

  if (mech_parseattributes(buffer, args, 3) != 2) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments to function!");
    return;
  }
  if (!parse_int_checked(args[0], &ticnum) ||
      !(ticnum >= 0 && ticnum < NUM_TICS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid tic number!");
    return;
  }
  selection = (TicSelectionContext){.tic = ticnum};
  multi_weapon_select(&(MultiWeaponSelectionRequest){
      .mech = mech,
      .actor = player,
      .selection = args[1],
      .mode = 0,
      .callback = addtic_sub_func,
      .context = &selection,
  });
}

static bool deltic_sub_func(const MultiWeaponSelectionCall *call) {
  int i;
  Mech *mech = call->mech;
  const TicSelectionContext *selection = call->context;

  for (i = call->first; i <= call->last; i++) {
    tic_weapon_remove(mech,
                      (TicWeaponReference){.tic = selection->tic, .weapon = i});
  }
  if (call->first != call->last)
    notify_printf(btech_context_evaluation(mech->xcode.context), call->actor,
                  "Weapons #%d - #%d removed from TIC %d!", call->first,
                  call->last, selection->tic);
  else
    notify_printf(btech_context_evaluation(mech->xcode.context), call->actor,
                  "Weapon #%d removed from TIC %d!", call->first,
                  selection->tic);
  return false;
}

void deltic_sub(DbRef player, Mech *mech, char *buffer) {
  int ticnum;
  int argc;
  char *args[3];
  TicSelectionContext selection;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc < 1 || argc > 2) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments to the function!");
    return;
  }
  if (argc == 1) {
    cleartic_sub(player, mech, buffer);
    return;
  }
  if (!parse_int_checked(args[0], &ticnum) ||
      !(ticnum >= 0 && ticnum < NUM_TICS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid tic number!");
    return;
  }
  selection = (TicSelectionContext){.tic = ticnum};
  multi_weapon_select(&(MultiWeaponSelectionRequest){
      .mech = mech,
      .actor = player,
      .selection = args[1],
      .mode = 0,
      .callback = deltic_sub_func,
      .context = &selection,
  });
}

static bool firetic_sub_func(const MultiWeaponSelectionCall *call) {
  int i;
  int j;
  int k;
  int count;
  int weapnum;
  Mech *mech = call->mech;
  const TicSelectionContext *selection = call->context;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  int f = mech_is_fallen(mech);

  for (i = call->first; i <= call->last; i++) {
    notify_printf(btech_context_evaluation(mech->xcode.context), call->actor,
                  "Firing weapons in tic #%d!", i);
    count = 0;
    for (k = 0; k < TICLONGS; k++) {
      if (*tic_word(mech, i, k)) {
        for (j = 0; j < SINGLE_TICLONG_SIZE; j++) {
          if (*tic_word(mech, i, k) & (1UL << (unsigned int)j)) {
            weapnum = (k * SINGLE_TICLONG_SIZE) + j;
            mech_weapon_fire_command(&(WeaponFireCommandRequest){
                .actor = call->actor,
                .mech = mech,
                .map = mech_map,
                .weapon_number = weapnum,
                .argument_count = selection->argument_count,
                .arguments = selection->arguments});
            if (f != (mech_is_fallen(mech))) {
              if (mech_is_started(mech))
                mech_notify(mech, MECHALL,
                            "That fall causes you to stop your fire!");
              return true;
            }
            if (!mech_is_started(mech))
              return true;
            count++;
          }
        }
      }
    }
    if (!count)
      notify_printf(btech_context_evaluation(mech->xcode.context), call->actor,
                    "*Click* (the tic contained no weapons)");
  }
  return false;
}

void firetic_sub(DbRef player, Mech *mech, char *buffer) {
  int ticnum;
  int argc;
  char *args[5];
  TicSelectionContext selection;

  argc = mech_parseattributes(buffer, args, 5);
  if (argc < 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Not enough arguments to function");
    return;
  }
  if (!parse_int_checked(args[0], &ticnum) ||
      !(ticnum >= 0 && ticnum < NUM_TICS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "TIC out of range!");
    return;
  }

  selection = (TicSelectionContext){
      .tic = ticnum,
      .argument_count = argc,
      .arguments = args,
  };
  multi_weapon_select(&(MultiWeaponSelectionRequest){
      .mech = mech,
      .actor = player,
      .selection = args[0],
      .mode = 2,
      .callback = firetic_sub_func,
      .context = &selection,
  });
}

static char *listtic_fun(void *context, int i, char buffer[static LBUF_SIZE]) {
  int j;
  int section;
  int critical;
  int count = 0;
  ListTicContext *list = context;
  Mech *mech = list->mech;
  int rtar;

  if (!list->weapon_count) {
    (void)snprintf(buffer, LBUF_SIZE, "No weapons in tic.");
    return buffer;
  }
  rtar = (i / 2) + (i % 2 ? ((list->weapon_count + 1) / 2) : 0);
  for (j = 0; j < MAX_WEAPONS_PER_MECH; j++) {
    if (mech_tic_contains_weapon(
            mech, (TicWeaponReference){.tic = list->tic, .weapon = j})) {
      if (count == rtar) {
        WeaponNumberLookupResult lookup = weapon_number_find(
            &(WeaponNumberLookupRequest){.mech = mech, .number = j});
        section = lookup.slot.section;
        critical = lookup.slot.critical;
        if (!lookup.found) {
          tic_weapon_remove(
              mech, (TicWeaponReference){.tic = list->tic, .weapon = j});
          j = MAX_WEAPONS_PER_MECH;
          continue;
        }
        (void)snprintf(
            buffer, LBUF_SIZE, "#%2d %3s %-16s %s", j,
            armor_section_abbreviation(
                &(ArmorSectionReference){
                    .unit_class = (UnitClass)((mech)->ud.type),
                    .movement_type = (MechMovementType)((mech)->ud.move),
                    .location = section})
                .text,
            checked_string_suffix(
                weapon_catalogue_name(weapon_from_equipment_index(
                    mech_critical_part_type(mech, section, critical))),
                3),
            mech_critical_is_nonfunctional(mech, section, critical) ? "(*)"
                                                                    : "");
        return buffer;
      }
      count++;
    }
  }
  (void)snprintf(buffer, LBUF_SIZE, "Unknown - error of some sort occured");
  return buffer;
}

void listtic_sub(DbRef player, Mech *mech, char *buffer) {
  char message_buffer[LBUF_SIZE];
  int ticnum;
  char *args[2];
  int i;
  int count = 0;
  CoolMenu *c;
  ListTicContext list;

  if (mech_parseattributes(buffer, args, 2) != 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  if (!parse_int_checked(args[0], &ticnum) ||
      !(ticnum >= 0 && ticnum < NUM_TICS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "TIC out of range!");
    return;
  }
  for (i = 0; i < MAX_WEAPONS_PER_MECH; i++) {
    if (mech_tic_contains_weapon(
            mech, (TicWeaponReference){.tic = ticnum, .weapon = i}))
      count++;
  }
  list = (ListTicContext){
      .mech = mech,
      .tic = ticnum,
      .weapon_count = count,
  };
  (void)snprintf(message_buffer, sizeof(message_buffer),
                 "TIC #%d listing for %s", ticnum, mech_display_id(mech).text);
  c = sel_col_fun_string_menu_context_k(2, message_buffer, listtic_fun, &list,
                                        max(1, count));
  show_cool_menu(btech_context_evaluation(mech->xcode.context), player, c);
  kill_cool_menu(c);
}

void mech_cleartic(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  cleartic_sub(player, mech, buffer);
}

void mech_addtic(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  addtic_sub(player, mech, buffer);
}

void mech_deltic(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  deltic_sub(player, mech, buffer);
}

void mech_firetic(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_condition_summary(mech).weapons_hold) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Currently in weapons hold. Unable to fire weapons.");
    return;
  }
  firetic_sub(player, mech, buffer);
}

void mech_listtic(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  listtic_sub(player, mech, buffer);
}

static void heat_cutoff_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (e->data2) {
    mech_notify(mech, MECHALL,
                "[fg=yellow]Heat dissipation cutoff engaged![reset]");
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_HEATCUTOFF);
  } else {
    mech_notify(mech, MECHALL,
                "[fg=green]Heat dissipation cutoff disengaged![reset]");
    mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_HEATCUTOFF);
  }
}

void heat_cutoff(DbRef player, Mech *mech, char *buffer [[maybe_unused]]) {
  if (mech->xcode.context->configuration->btech_heatcutoff < 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "This command has been disabled.");
    return;
  }

  if (!common_checks(player, mech, MECH_USUALSMO))
    return;
  if (mech_event_count(mech, EVENT_HEATCUTOFFCHANGING)) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), player,
        "You are already toggling heat cutoff status. Please be patient.");
    return;
  }
  if (mech_heat_cutoff_is_enabled(mech)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Disengaging heat dissipation cutoff...");
    mech_event_schedule(mech, EVENT_HEATCUTOFFCHANGING, heat_cutoff_event, 4,
                        0);
  } else {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Engaging heat dissipation cutoff...");
    mech_event_schedule(mech, EVENT_HEATCUTOFFCHANGING, heat_cutoff_event, 4,
                        1);
  }
}
