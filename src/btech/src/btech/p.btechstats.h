
/*
   p.btechstats.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 10:40:18 CET 1999 from btechstats.c */

#include "mux/commands/command_context.h"
#include "mux/server/platform.h"

#pragma once

/* btechstats.c */
char *silly_get_uptime_to_string(int i);
void list_charvaluestuff(DbRef player, int flag);
int char_getvaluecode(char *name);
int char_rollsaving(void);
int char_rollunskilled(void);
int char_rollskilled(void);
int char_rolld6(int num);
int char_getvalue(DbRef player, char *name);
void char_setvalue(DbRef player, char *name, int value);
int char_getskilltargetbycode(DbRef player, int code, int modifier);
int char_getskilltarget(DbRef player, char *name, int modifier);
int char_getxpbycode(DbRef player, int code);
int char_gainxpbycode(DbRef player, int code, int amount, int override);
int char_gainxp(DbRef player, char *skill, int amount);
int char_getskillsuccess(DbRef player, char *name, int modifier, int loud);
int char_getskillmargsucc(DbRef player, char *name, int modifier);
int char_getopposedskill(DbRef first, char *skill1, DbRef second, char *skill2);
int char_getattrsave(DbRef player, char *name);
int char_getattrsavesucc(DbRef player, char *name);
void zap_unneccessary_stats(void);
void init_btechstats(void);
void do_charstatus(DbRef player, DbRef cause, int key, char *arg1);
void do_charclear(DbRef player, DbRef cause, int key, char *arg1);
DbRef char_lookupplayer(DbRef player, DbRef cause, int key, char *arg1);
void initialize_pc(DbRef player, MECH *mech);
void fix_pilotdamage(MECH *mech, DbRef player);
int mw_ic_bth(MECH *mech);
int handlemwconc(MECH *mech, int initial);
void headhitmwdamage(MECH *mech, MECH *attacker, int dam);
void mwlethaldam(MECH *mech, MECH *attacker, int dam);
void lower_xp(DbRef player, int promillage);
void AccumulateTechXP(DbRef pilot, MECH *mech, int reason);
void AccumulateTechWeaponsXP(DbRef pilot, MECH *mech, int reason);
void AccumulateCommXP(DbRef pilot, MECH *mech);
void AccumulatePilXP(DbRef pilot, MECH *mech, int reason, int addanyway);
void AccumulateSpotXP(DbRef pilot, MECH *attacker, MECH *wounded);
int MadePerceptionRoll(MECH *mech, int modifier);
void AccumulateArtyXP(DbRef pilot, MECH *attacker, MECH *wounded);
void AccumulateComputerXP(DbRef pilot, MECH *mech, int reason);
int HasBoolAdvantage(DbRef player, const char *name);
void AccumulateGunXP(DbRef pilot, MECH *attacker, MECH *wounded,
                     int numOccurences, float multiplier, int weapindx,
                     int bth);
void AccumulateGunXPold(DbRef pilot, MECH *attacker, MECH *wounded,
                        int numOccurences, float multiplier, int weapindx,
                        int bth);
void fun_btgetcharvalue(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context);
void fun_btsetcharvalue(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context);
void fun_btcharlist(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context);
void debug_xptop(DbRef player, void *data, char *buffer);
void debug_setxplevel(DbRef player, void *data, char *buffer);
int btthreshold_func(char *skillname);
struct chargen_struct *retrieve_chargen_struct(DbRef player);
int lowest_bit(int num);
int recursive_add(int lev);
int can_proceed(DbRef player, struct chargen_struct *st);
void cm_a_add(DbRef player, void *data, char *buffer);
void cm_a_minus(DbRef player, void *data, char *buffer);
void cm_a_toggle(DbRef player, void *data, char *buffer);
void cm_a_set(DbRef player, void *data, char *buffer);
void cm_b_add(DbRef player, void *data, char *buffer);
void cm_b_minus(DbRef player, void *data, char *buffer);
void cm_b_toggle(DbRef player, void *data, char *buffer);
void cm_b_set(DbRef player, void *data, char *buffer);
void cm_c_add(DbRef player, void *data, char *buffer);
void cm_c_minus(DbRef player, void *data, char *buffer);
void cm_c_toggle(DbRef player, void *data, char *buffer);
void cm_c_set(DbRef player, void *data, char *buffer);
void cm_d_add(DbRef player, void *data, char *buffer);
void cm_d_minus(DbRef player, void *data, char *buffer);
void cm_d_toggle(DbRef player, void *data, char *buffer);
void cm_d_set(DbRef player, void *data, char *buffer);
void cm_e_add(DbRef player, void *data, char *buffer);
void cm_e_minus(DbRef player, void *data, char *buffer);
void cm_e_toggle(DbRef player, void *data, char *buffer);
void cm_e_set(DbRef player, void *data, char *buffer);
void cm_f_add(DbRef player, void *data, char *buffer);
void cm_f_minus(DbRef player, void *data, char *buffer);
void cm_f_toggle(DbRef player, void *data, char *buffer);
void cm_f_set(DbRef player, void *data, char *buffer);
void cm_g_add(DbRef player, void *data, char *buffer);
void cm_g_minus(DbRef player, void *data, char *buffer);
void cm_g_toggle(DbRef player, void *data, char *buffer);
void cm_g_set(DbRef player, void *data, char *buffer);
void cm_h_add(DbRef player, void *data, char *buffer);
void cm_h_minus(DbRef player, void *data, char *buffer);
void cm_h_toggle(DbRef player, void *data, char *buffer);
void cm_h_set(DbRef player, void *data, char *buffer);
void cm_i_add(DbRef player, void *data, char *buffer);
void cm_i_minus(DbRef player, void *data, char *buffer);
void cm_i_toggle(DbRef player, void *data, char *buffer);
void cm_i_set(DbRef player, void *data, char *buffer);
void cm_j_add(DbRef player, void *data, char *buffer);
void cm_j_minus(DbRef player, void *data, char *buffer);
void cm_j_toggle(DbRef player, void *data, char *buffer);
void cm_j_set(DbRef player, void *data, char *buffer);
void cm_k_add(DbRef player, void *data, char *buffer);
void cm_k_minus(DbRef player, void *data, char *buffer);
void cm_k_toggle(DbRef player, void *data, char *buffer);
void cm_k_set(DbRef player, void *data, char *buffer);
void cm_l_add(DbRef player, void *data, char *buffer);
void cm_l_minus(DbRef player, void *data, char *buffer);
void cm_l_toggle(DbRef player, void *data, char *buffer);
void cm_l_set(DbRef player, void *data, char *buffer);
void cm_m_add(DbRef player, void *data, char *buffer);
void cm_m_minus(DbRef player, void *data, char *buffer);
void cm_m_toggle(DbRef player, void *data, char *buffer);
void cm_m_set(DbRef player, void *data, char *buffer);
void cm_n_add(DbRef player, void *data, char *buffer);
void cm_n_minus(DbRef player, void *data, char *buffer);
void cm_n_toggle(DbRef player, void *data, char *buffer);
void cm_n_set(DbRef player, void *data, char *buffer);
void cm_o_add(DbRef player, void *data, char *buffer);
void cm_o_minus(DbRef player, void *data, char *buffer);
void cm_o_toggle(DbRef player, void *data, char *buffer);
void cm_o_set(DbRef player, void *data, char *buffer);
void cm_p_add(DbRef player, void *data, char *buffer);
void cm_p_minus(DbRef player, void *data, char *buffer);
void cm_p_toggle(DbRef player, void *data, char *buffer);
void cm_p_set(DbRef player, void *data, char *buffer);
void cm_q_add(DbRef player, void *data, char *buffer);
void cm_q_minus(DbRef player, void *data, char *buffer);
void cm_q_toggle(DbRef player, void *data, char *buffer);
void cm_q_set(DbRef player, void *data, char *buffer);
void cm_r_add(DbRef player, void *data, char *buffer);
void cm_r_minus(DbRef player, void *data, char *buffer);
void cm_r_toggle(DbRef player, void *data, char *buffer);
void cm_r_set(DbRef player, void *data, char *buffer);
void cm_s_add(DbRef player, void *data, char *buffer);
void cm_s_minus(DbRef player, void *data, char *buffer);
void cm_s_toggle(DbRef player, void *data, char *buffer);
void cm_s_set(DbRef player, void *data, char *buffer);
void cm_t_add(DbRef player, void *data, char *buffer);
void cm_t_minus(DbRef player, void *data, char *buffer);
void cm_t_toggle(DbRef player, void *data, char *buffer);
void cm_t_set(DbRef player, void *data, char *buffer);
void cm_u_add(DbRef player, void *data, char *buffer);
void cm_u_minus(DbRef player, void *data, char *buffer);
void cm_u_toggle(DbRef player, void *data, char *buffer);
void cm_u_set(DbRef player, void *data, char *buffer);
void cm_v_add(DbRef player, void *data, char *buffer);
void cm_v_minus(DbRef player, void *data, char *buffer);
void cm_v_toggle(DbRef player, void *data, char *buffer);
void cm_v_set(DbRef player, void *data, char *buffer);
void cm_w_add(DbRef player, void *data, char *buffer);
void cm_w_minus(DbRef player, void *data, char *buffer);
void cm_w_toggle(DbRef player, void *data, char *buffer);
void cm_w_set(DbRef player, void *data, char *buffer);
void cm_x_add(DbRef player, void *data, char *buffer);
void cm_x_minus(DbRef player, void *data, char *buffer);
void cm_x_toggle(DbRef player, void *data, char *buffer);
void cm_x_set(DbRef player, void *data, char *buffer);
void cm_y_add(DbRef player, void *data, char *buffer);
void cm_y_minus(DbRef player, void *data, char *buffer);
void cm_y_toggle(DbRef player, void *data, char *buffer);
void cm_y_set(DbRef player, void *data, char *buffer);
void cm_z_add(DbRef player, void *data, char *buffer);
void cm_z_minus(DbRef player, void *data, char *buffer);
void cm_z_toggle(DbRef player, void *data, char *buffer);
void cm_z_set(DbRef player, void *data, char *buffer);
int can_advance_state(struct chargen_struct *st);
int can_go_back_state(struct chargen_struct *st);
void recalculate_skillpoints(struct chargen_struct *st);
void go_back_state(DbRef player, struct chargen_struct *st);
void chargen_look(DbRef player, void *data, char *buffer);
void chargen_begin(DbRef player, void *data, char *buffer);
void chargen_apply(DbRef player, void *data, char *buffer);
void chargen_done(DbRef player, void *data, char *buffer);
void chargen_next(DbRef player, void *data, char *buffer);
void chargen_prev(DbRef player, void *data, char *buffer);
void chargen_reset(DbRef player, void *data, char *buffer);
void chargen_help(DbRef player, void *data, char *buffer);
