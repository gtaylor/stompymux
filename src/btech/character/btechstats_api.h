/* Declares the BattleTech btechstats API. */

#include <stdbool.h>

#include "btechstats.h"
#include "mux/commands/command_context.h"
#include "mux/server/platform.h"

#pragma once

#include "btech/scripting/script_functions_api.h"
typedef struct BtechContext BtechContext;
typedef struct CommandInvocation CommandInvocation;

typedef struct UptimeText {
  char text[MBUF_SIZE];
} UptimeText;

/* btechstats.c */
UptimeText uptime_text(int seconds);
void list_charvaluestuff(EvaluationContext *evaluation, DbRef player, int flag);
int char_getvaluecode(BtechContext *context, const char *name);
int char_rollsaving(BtechContext *context);
int char_rollunskilled(BtechContext *context);
int char_rollskilled(BtechContext *context);
int char_rolld6(BtechContext *context, int num);
int char_getvalue(BtechContext *context, DbRef player, const char *name);
void char_setvalue(BtechContext *context, DbRef player, const char *name,
                   int value);
int char_getskilltargetbycode(BtechContext *context, DbRef player, int code,
                              int modifier);
int char_getskilltarget(BtechContext *context, DbRef player, const char *name,
                        int modifier);
int char_getxpbycode(const CharacterValueRequest *request);
bool char_gainxpbycode(const CharacterExperienceChange *change);
bool char_gainxp(BtechContext *context, DbRef player, const char *skill,
                 int amount);
int char_getskillsuccess(const CharacterSkillCheck *check);
int char_getskillmargsucc(BtechContext *context, DbRef player, const char *name,
                          int modifier);
DbRef char_getopposedskill(BtechContext *context, DbRef first,
                           const char *skill1, DbRef second,
                           const char *skill2);
int char_getattrsave(BtechContext *context, DbRef player, const char *name);
int char_getattrsavesucc(BtechContext *context, DbRef player, const char *name);
void zap_unneccessary_stats(void);
void init_btechstats(BtechContext *context);
void btech_stats_destroy(BtechContext *context);
bool character_state_validate_all(BtechContext *context);
void do_charclear(CommandInvocation *invocation);
typedef struct CharacterLookupRequest {
  BtechContext *context;
  DbRef viewer;
  const char *name;
} CharacterLookupRequest;
DbRef character_lookup(const CharacterLookupRequest *request);
void initialize_pc(DbRef player, Mech *mech);
void fix_pilotdamage(Mech *mech, DbRef player);
int mw_ic_bth(Mech *mech);
bool handlemwconc(Mech *mech, int initial);
void headhitmwdamage(Mech *mech, Mech *attacker, int dam);
void mwlethaldam(Mech *mech, Mech *attacker, int dam);
typedef struct CharacterExperienceReduction {
  BtechContext *context;
  DbRef character;
  int per_mille;
} CharacterExperienceReduction;
void character_experience_reduce(const CharacterExperienceReduction *change);
void accumulate_tech_xp(BtechContext *context, DbRef pilot, Mech *mech,
                        int reason);
void accumulate_tech_weapons_xp(BtechContext *context, DbRef pilot, Mech *mech,
                                int reason);
void accumulate_comm_xp(DbRef pilot, Mech *mech);
typedef struct PilotingExperienceAward {
  DbRef pilot;
  Mech *mech;
  int reason;
  bool unconditional;
} PilotingExperienceAward;
void piloting_experience_award(const PilotingExperienceAward *award);
void accumulate_spot_xp(DbRef pilot, Mech *attacker, Mech *wounded);
bool made_perception_roll(Mech *mech, int modifier);
void accumulate_arty_xp(DbRef pilot, Mech *attacker, Mech *wounded);
void accumulate_computer_xp(DbRef pilot, Mech *mech, int reason);
bool has_bool_advantage(BtechContext *context, DbRef player, const char *name);
typedef struct GunneryExperienceAward {
  DbRef pilot;
  Mech *attacker;
  Mech *target;
  int damage;
  double multiplier;
  int weapon_index;
  int base_to_hit;
} GunneryExperienceAward;

void gunnery_experience_award(const GunneryExperienceAward *award);
void debug_xptop(DbRef player, void *data, const char *buffer);
void debug_setxplevel(DbRef player, void *data, char *buffer);
int btthreshold_func(BtechContext *context, const char *skillname);
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
