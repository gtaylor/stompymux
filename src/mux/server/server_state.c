/* server_state.c - Mutable server-state initialization */

#include "mux/server/server_state.h"

#include "mux/database/attrs.h"

ServerState mudstate;

void server_state_initialize(void) {
  int index;

  mudstate.events_flag = 0;
  mudstate.events_lasthour = -1;
  mudstate.initializing = 0;
  mudstate.panicking = 0;
  mudstate.dumping = 0;
  mudstate.logging = 0;
  mudstate.generation = 0;
  mudstate.curr_player = NOTHING;
  mudstate.curr_enactor = NOTHING;
  mudstate.shutdown_flag = 0;
  mudstate.attr_next = A_USER_START;
  mudstate.debug_cmd = (char *)"< init >";
  mudstate.access_list = NULL;
  mudstate.suspect_list = NULL;
  mudstate.qhead = NULL;
  mudstate.qtail = NULL;
  mudstate.qwait = NULL;
  mudstate.qsemfirst = NULL;
  mudstate.qsemlast = NULL;
  mudstate.badname_head = NULL;
  mudstate.mstat_ixrss[0] = 0;
  mudstate.mstat_ixrss[1] = 0;
  mudstate.mstat_idrss[0] = 0;
  mudstate.mstat_idrss[1] = 0;
  mudstate.mstat_isrss[0] = 0;
  mudstate.mstat_isrss[1] = 0;
  mudstate.mstat_secs[0] = 0;
  mudstate.mstat_secs[1] = 0;
  mudstate.mstat_curr = 0;
  mudstate.iter_alist.data = NULL;
  mudstate.iter_alist.len = 0;
  mudstate.iter_alist.next = NULL;
  mudstate.mod_alist = NULL;
  mudstate.mod_size = 0;
  mudstate.mod_al_id = NOTHING;
  mudstate.olist = NULL;
  mudstate.min_size = 0;
  mudstate.db_top = 0;
  mudstate.db_size = 0;
  mudstate.freelist = NOTHING;
  mudstate.markbits = NULL;
  mudstate.func_nest_lev = 0;
  mudstate.func_invk_ctr = 0;
  mudstate.ntfy_nest_lev = 0;
  mudstate.lock_nest_lev = 0;
  mudstate.zone_nest_num = 0;
  mudstate.inpipe = 0;
  mudstate.pout = NULL;
  mudstate.poutnew = NULL;
  mudstate.poutbufc = NULL;
  mudstate.poutobj = NOTHING;
  for (index = 0; index < MAX_GLOBAL_REGS; index++)
    mudstate.global_regs[index] = NULL;
}
