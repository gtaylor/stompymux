#include "mech_template_api.h"
#include "values_internal.h"

#include "crit_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "registry_api.h"

void fun_bttechlist(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  DbRef it;
  Mech *mech;
  char *infostr;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player, fargs[0]);
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!(mech = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  infostr = techlist_func(mech, (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : " ");
}

void fun_bttechlist_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  Mech *mech;
  char *infostr;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }

  infostr = techlist_func(mech, (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1");
}

/* Function to return the 'payload' of a unit
 * ie: the Guns and Ammo
 * in a list format like <item_1> <# of 1>|...|<item_n> <# of n>
 * Dany - 06/2005 */
void fun_btpayload_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  Mech *mech;
  char *infostr;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }

  infostr = payloadlist_func(mech, (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1");
}

void fun_btshowstatus_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  DbRef outplayer;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }
  outplayer = match_thing(&context->command->match, player, fargs[1]);
  if (outplayer == NOTHING ||
      !is_examinable(context->world->database, player, outplayer) ||
      !is_player(context->btech->database, outplayer)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }

  mech_status(outplayer, (void *)mech, "R");
  safe_tprintf_str(buff, bufc, "1");
}

void fun_btshowwspecs_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  DbRef outplayer;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }
  outplayer = match_thing(&context->command->match, player, fargs[1]);
  if (outplayer == NOTHING ||
      !is_examinable(context->world->database, player, outplayer) ||
      !is_player(context->btech->database, outplayer)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }

  mech_weaponspecs(outplayer, (void *)mech, "");
  safe_tprintf_str(buff, bufc, "1");
}

void fun_btshowcritstatus_ref(char *buff, char **bufc, DbRef player,
                              DbRef cause, char *fargs[], int nfargs,
                              char *cargs[], int ncargs,
                              EvaluationContext *context) {
  DbRef outplayer;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }
  outplayer = match_thing(&context->command->match, player, fargs[1]);
  if (outplayer == NOTHING ||
      !is_examinable(context->world->database, player, outplayer) ||
      !is_player(context->btech->database, outplayer)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }

  mech_critstatus(outplayer, (void *)mech, fargs[2]);
  safe_tprintf_str(buff, bufc, "1");
}

void fun_btengrate(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  DbRef mechdb;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mechdb = match_thing(&context->command->match, player, fargs[0]);
  if (mechdb == NOTHING ||
      !is_examinable(context->world->database, player, mechdb)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, mechdb)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!(mech = btech_context_get_mech(context->btech, mechdb))) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }

  safe_tprintf_str(buff, bufc, "%d %d", mech_engine_rating(mech),
                   susp_factor(mech));
}

void fun_btengrate_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (!(mech = load_refmech(context->btech, fargs[0]))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID REF");
    return;
  }

  safe_tprintf_str(buff, bufc, "%d %d", mech_engine_rating(mech),
                   susp_factor(mech));
}

void fun_btfasabasecost_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                            char *fargs[], int nfargs, char *cargs[],
                            int ncargs, EvaluationContext *context) {
#ifdef BT_ADVANCED_ECON
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (!(mech = load_refmech(context->btech, fargs[0]))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID REF");
    return;
  }

  safe_tprintf_str(buff, bufc, "%llu", mech_fasa_cost(mech));
#else
  safe_tprintf_str(buff, bufc, "#-1 NO ECONDB SUPPORT");
#endif
}

void fun_btunitpartslist_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                             char *fargs[], int nfargs, char *cargs[],
                             int ncargs, EvaluationContext *context) {
  Mech *mech;
  char parts[LBUF_SIZE];

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (!(mech = load_refmech(context->btech, fargs[0]))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID REF");
    return;
  }

  unit_parts_list(mech, parts);
  safe_tprintf_str(buff, bufc, "%s", parts);
}

void fun_btunitpartslist(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {

  DbRef mechdb;
  Mech *mech;
  char parts[LBUF_SIZE];

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mechdb = match_thing(&context->command->match, player, fargs[0]);
  if (mechdb == NOTHING ||
      !is_examinable(context->world->database, player, mechdb)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, mechdb)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!(mech = btech_context_get_mech(context->btech, mechdb))) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }

  unit_parts_list(mech, parts);
  safe_tprintf_str(buff, bufc, "%s", parts);
}
