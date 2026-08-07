#include "mux/server/platform.h"

#pragma once

typedef struct BtechContext BtechContext;

#include <stdbool.h>

#include "mux/server/platform.h"
#include "mux/support/alloc.h"

typedef struct Mech Mech;

typedef struct MechId {
  char text[3];
} MechId;

/* mech.utils.c */
/* Misc Functions */
const char *mechtypename(Mech *foo);
int MNumber(Mech *mech, int low, int high);
MechId mech_id(Mech *mech, bool lowercase);
char *MyToUpper(char *string);
void MarkForLOSUpdate(Mech *mech);

int round_to_halfton(int weight);
int round_to_quarterton(int weight);

/* Self-inflicted kill types. (But flood might be accidental/intentional.) */
constexpr char KILL_TYPE_SELF_DESTRUCT[] = "SELF-DESTRUCT";
constexpr char KILL_TYPE_EJECT[] = "EJECT";
constexpr char KILL_TYPE_FLOOD[] = "FLOOD"; /* includes vacuum */
/* Accidental kill types. (But ice/heat might be intentional.) */
constexpr char KILL_TYPE_ICE[] = "FLOOD-ICE";
constexpr char KILL_TYPE_HEAT[] = "HEAT";
/* Intentional kill types.  */
#define KILL_TYPE_NORMAL                                                       \
  "DESTROYED" /* all other kills; includes carrier destruction */
#define KILL_TYPE_PILOT                                                        \
  "PILOT" /* Only happens on vehicles. Mainly crew death */
#define KILL_TYPE_MWDAMAGE                                                     \
  "MWDAMAGE" /* Failed MW Conc rolls once too many or one too many head hit */
constexpr char KILL_TYPE_BEHEADED[] = "BEHEADED";
constexpr char KILL_TYPE_XLENGINE[] = "XLENGINE";
constexpr char KILL_TYPE_FUELTANK[] = "FUELTANK"; /* Fuel Tank Crit Death */
#define KILL_TYPE_COCKPIT                                                      \
  "COCKPIT" /* Alot different than Pilot death. Cockpit Crit death */
constexpr char KILL_TYPE_POWERPLANT[] =
    "POWERPLANT"; /* Vehicle powerplant crit death */
constexpr char KILL_TYPE_SCHARGE[] = "SCHARGE"; /* Super Charger overload */
#define KILL_TYPE_TRANSPORT                                                    \
  "TRANSPORT" /* The Transport containing the unit died */
constexpr char KILL_TYPE_ENGINE[] =
    "ENGINE"; /* Unit engined, standard fusion death */
#define KILL_TYPE_HEAD_TARGET                                                  \
  "HEAD-TARGET" /* Head was taken off, using TARGET */

void ChannelEmitKill(Mech *mech, Mech *attacker, const char *reason);

BattleMap *ValidMap(BtechContext *context, DbRef player, DbRef map);
DbRef FindMechOnMap(BattleMap *map, char *mechid);
Mech *find_mech_in_hex(Mech *mech, BattleMap *mech_map, int x, int y,
                       int needlos);
DbRef FindTargetDBREFFromMapNumber(Mech *mech, char *mapnum);

/* Map Math */
int AcceptableDegree(int d);
void FindXY(float x0, float y0, int bearing, float range, float *x1, float *y1);
float FindRange(float x0, float y0, float z0, float x1, float y1, float z1);
int MyHexDist(int x1, int y1, int x2, int y2, int tc);
float FindXYRange(float x0, float y0, float x1, float y1);
float FindHexRange(float x0, float y0, float x1, float y1);
void RealCoordToMapCoord(short *hex_x, short *hex_y, float cart_x,
                         float cart_y);
void MapCoordToRealCoord(int hex_x, int hex_y, float *cart_x, float *cart_y);
typedef void (*NeighborHexCallback)(BattleMap *map, int x, int y,
                                    void *context);
void visit_neighbor_hexes(BattleMap *map, int x, int y,
                          NeighborHexCallback callback, void *context);
void FindComponents(float magnitude, int degrees, float *x, float *y);
void CheckEdgeOfMap(Mech *mech);
int FindZBearing(float x0, float y0, float z0, float x1, float y1, float z1);
int FindBearing(float x0, float y0, float x1, float y1);
int InWeaponArc(Mech *mech, float x, float y);
int IsInWeaponArc(Mech *mech, float x, float y, int section, int critical);
void navigate_sketch_mechs(Mech *mech, BattleMap *map, int x, int y,
                           char buff[][MBUF_SIZE]);
int FindTargetXY(Mech *mech, float *x, float *y, float *z);

/* Skill lookups */
char *FindGunnerySkillName(Mech *mech, int weapindx);
char *FindPilotingSkillName(Mech *mech);
int FindPilotPiloting(Mech *mech);
int FindSPilotPiloting(Mech *mech);
int FindPilotSpotting(Mech *mech);
int FindPilotArtyGun(Mech *mech);
int FindPilotGunnery(Mech *mech, int weapindx);
char *FindTechSkillName(Mech *mech);
int FindTechSkill(DbRef player, Mech *mech);

/* Skill rolls */
long btech_random_range(BtechContext *context, long low, long high);
int MadePilotSkillRoll(Mech *mech, int mods);
int mech_pilot_skill_roll_target(Mech *mech, int mods);
int MadePilotSkillRoll_Advanced(Mech *mech, int mods, int succeedWhenFallen);
int MadePilotSkillRoll_NoXP(Mech *mech, int mods, int succeedWhenFallen);
int btech_random_roll(BtechContext *context);

/* Section/Crit Functions */
int CritsInLoc(Mech *mech, int index);
int SectHasBusyWeap(Mech *mech, int sect);
int FindWeapons_Advanced(Mech *mech, int index, unsigned char *weaparray,
                         unsigned char *weapdataarray, int *critical,
                         int whine);
int FindAmmunition(Mech *mech, unsigned char *weaparray,
                   unsigned short *ammoarray, unsigned short *ammomaxarray,
                   unsigned int *modearray, int returnall);
int FindLegHeatSinks(Mech *mech);
int FindWeaponNumberOnMech_Advanced(Mech *mech, int number, int *section,
                                    int *crit, int sight);
int FindWeaponNumberOnMech(Mech *mech, int number, int *section, int *crit);
int FindWeaponFromIndex(Mech *mech, int weapindx, int *section, int *crit);
int FindWeaponIndex(Mech *mech, int number);
int findAmmoInSection(Mech *mech, int section, int type, int nogof, int gof);
int FullAmmo(Mech *mech, int loc, int pos);
int FindAmmoForWeapon_sub(Mech *mech, int weapSection, int weapCritical,
                          int weapindx, int start, int *section, int *critical,
                          int nogof, int gof);
int FindAmmoForWeapon(Mech *mech, int weapindx, int start, int *section,
                      int *critical);
int CountAmmoForWeapon(Mech *mech, int weapindx);
int FindArtemisForWeapon(Mech *mech, int section, int critical);
int ReverseSplitCritLoc(Mech *mech, int sect, int crit);
int FindSplitCrits(Mech *mech, int sect, int type, int crit);
int GetSplitData(Mech *mech, int sect, int data, int *ssect, int *scrit,
                 int *stype);
int FindDestructiveAmmo(Mech *mech, int *section, int *critical);
int FindInfernoAmmo(Mech *mech, int *section, int *critical);
int FindRoundsForWeapon(Mech *mech, int weapindx);
int HeatFactor(Mech *mech);
int WeaponIsNonfunctional(Mech *mech, int section, int crit, int numcrits);
char **ProperSectionStringFromType(int type, int mtype);
void ArmorStringFromIndex(int index, char *buffer, char type, char mtype);
int GetWeaponCrits(Mech *mech, int weapindx);
int listmatch(char *const *foo, char *mat);
typedef int (*MultiWeaponSelectionCallback)(Mech *mech, DbRef player, int low,
                                            int high, void *context);
void multi_weap_sel(Mech *mech, DbRef player, char *buffer, int bitbybit,
                    MultiWeaponSelectionCallback callback, void *context);

/* Tech/Repair functions */
void do_sub_magic(Mech *mech, int loud);
void do_magic(Mech *mech);
void do_fixextra(Mech *mech);
void mech_RepairPart(Mech *mech, int loc, int pos);
int no_locations_destroyed(Mech *mech);
void mech_ReAttach(Mech *mech, int loc);
void mech_ReplaceSuit(Mech *mech, int loc);
void mech_ReSeal(Mech *mech, int loc);
void mech_Detach(Mech *mech, int loc);
void mech_FillPartAmmo(Mech *mech, int loc, int pos);

int CountDestroyedLegs(Mech *objMech);
int IsLegDestroyed(Mech *objMech, int wLoc);
int IsMechLegLess(Mech *objMech);
int FindFirstWeaponCrit(Mech *objMech, int wLoc, int wSlot, int wStartSlot,
                        int wCritType, int wMaxCrits);
int checkAllSections(Mech *mech, int specialToFind);
int checkSectionForSpecial(Mech *mech, int specialToFind, int wSec);
int getRemainingInternalPercent(Mech *mech);
int getRemainingArmorPercent(Mech *mech);
int FindObj(Mech *mech, int loc, int type);
int FindObjWithDest(Mech *mech, int loc, int type);
int FindAndCheckAmmo(Mech *mech, int weapindx, int section, int critical,
                     int *ammoLoc, int *ammoCrit, int *ammoLoc1, int *ammoCrit1,
                     int *wGattlingShots);

#ifdef BT_ADVANCED_ECON
void Calc_AddOffBV(const Mech *mech, float *offbv, char *desc, float value);
void Calc_AddDefBV(const Mech *mech, float *defbv, char *desc, float value);
void Calc_SubDefBV(const Mech *mech, float *defbv, char *desc, float value);
int mech_armorpoints(Mech *mech);
int mech_intpoints(Mech *mech);
#endif
#ifdef BT_CALCULATE_BV
int FindAverageGunnery(Mech *mech);
int CalculateBV(Mech *mech, int gunstat, int pilstat);
float Calculate_Defensive_BV(Mech *mech);
float Calculate_Offensive_BV(Mech *mech);
#endif
void unit_parts_list(Mech *mech, char buffer[static LBUF_SIZE]);
int mech_recycling_state(Mech *mech, int num);
#ifdef BT_COMPLEXREPAIRS
int GetPartMod(const Mech *mech, int t);
int ProperArmor(const Mech *mech);
int ProperInternal(const Mech *mech);
int alias_part(Mech *mech, int t, int loc);
int ProperMyomer(Mech *mech);
#endif
