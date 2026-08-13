/* Defines BattleTech equipment types and constants. */

#pragma once

#include <string.h>

#include "btconfig.h"
#include "missile_hit_registry.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
constexpr int NUM_ITEMS = 1024;
constexpr int NUM_ITEMS_M = 512;
constexpr int NUM_BAYS = 4;
constexpr int NUM_TURRETS = 3;
constexpr int C3I_NETWORK_SIZE = 5;
constexpr int C3_NETWORK_SIZE =
    11; /* Constant for the max size of the network */
constexpr int BRANDCOUNT = 5;

constexpr int LEFTSIDE = 1;
constexpr int RIGHTSIDE = 2;
constexpr int FRONT = 3;
constexpr int BACK = 4;

constexpr int STAND = 1;
constexpr int FALL = 0;

constexpr int TURN = 30; /* 30 sec turn */
constexpr float KPH_PER_MP = 10.75F;
constexpr float MP_PER_KPH = 0.0930233F;              /* 1/KPH_PER_MP */
constexpr double MP_PER_UPDATE_PER_KPH = 0.003100777; /* MP_PER_KPH/30 */
constexpr double SCALEMAP = 322.5;                    /* 1/update      */
constexpr int HEXLEVEL = 5;                           /* levels/hex    */
constexpr float ZSCALE = 64.5F;                       /* scalemap/hexlevel */
constexpr double XSCALE = 0.1547;                     /* hex constant  */
constexpr double YSCALE2 = 9.61482e-6;                /* update**2     */
constexpr float MP1 = 10.75F;                         /* 2*MS_PER_MP */
constexpr float MP2 = 21.50F;                         /* 2*MS_PER_MP */
constexpr float MP3 = 32.25F;                         /* 3*MS_PER_MP */
constexpr float MP4 = 43.00F;                         /* 4*MS_PER_MP */
constexpr float MP5 = 53.75F;                         /* 5*MS_PER_MP */
constexpr float MP6 = 64.50F;                         /* 6*MS_PER_MP */
constexpr float MP9 = 96.75F;                         /* 9*MS_PER_MP */
constexpr double DELTAFACING = 1440.0;

constexpr int DEFAULT_FREQS = 5;
constexpr int FREQS = 16;

constexpr int FREQ_DIGITAL = 1;
constexpr int FREQ_MUTE = 2;  /* For digital transmissions */
constexpr int FREQ_RELAY = 4; /* For digital transmissions */
constexpr int FREQ_INFO = 8;  /* For digital transmissions */
constexpr int FREQ_SCAN = 16;
constexpr int FREQ_REST = 32;

constexpr int RADIO_RELAY = 1; /* ability to relay things */
constexpr int RADIO_INFO =
    2; /* ability to see where (digital) message comes from */
constexpr int RADIO_SCAN = 4; /* ability to scan for frequencies */
constexpr int RADIO_NODIGITAL =
    8; /* lacks the ability to hear or set digital freqs */

constexpr int CHTITLELEN = 15;

constexpr int NOT_FOUND = -1;
constexpr int NUM_CRITICALS = 12;

constexpr int ARMOR = 1;
constexpr int INTERNAL = 2;
constexpr int REAR = 3;

constexpr int NOARC = 0;
constexpr int FORWARDARC = 1;
constexpr int LSIDEARC = 2;
constexpr int RSIDEARC = 4;
constexpr int REARARC = 8;
constexpr int TURRETARC = 16;

/*
   Critical Types
   0       Empty
   1-192   Weapons
   193-384 Ammo
   385-394 Bombs (Aero/VTOL droppable)
   395-511 Special startings...
 */

/* Critical Types... */
constexpr int NUM_WEAPONS = 192;
constexpr int NUM_BOMBS = 9;

constexpr int EMPTY = 0;
constexpr int WEAPON_BASE_INDEX = 1;
#define AMMO_BASE_INDEX (WEAPON_BASE_INDEX + NUM_WEAPONS) /* 193 */
#define BOMB_BASE_INDEX (AMMO_BASE_INDEX + NUM_WEAPONS)   /* 385 */
#define SPECIAL_BASE_INDEX (BOMB_BASE_INDEX + NUM_BOMBS)  /* 394 */
constexpr int OSPECIAL_BASE_INDEX = 220;
constexpr int CARGO_BASE_INDEX = 512;

#define SPECIALCOST_SIZE (CARGO_BASE_INDEX - SPECIAL_BASE_INDEX)
#define AMMOCOST_SIZE NUM_WEAPONS
#define WEAPCOST_SIZE NUM_WEAPONS
#define CARGOCOST_SIZE (NUM_ITEMS - NUM_ITEMS_M)
#define BOMBCOST_SIZE NUM_BOMBS

static inline bool equipment_is_ammunition(int equipment) {
  return equipment >= AMMO_BASE_INDEX && equipment < BOMB_BASE_INDEX;
}

static inline bool equipment_is_bomb(int equipment) {
  return equipment >= BOMB_BASE_INDEX && equipment < SPECIAL_BASE_INDEX;
}

static inline bool equipment_is_special(int equipment) {
  return equipment >= SPECIAL_BASE_INDEX && equipment < CARGO_BASE_INDEX;
}

static inline bool equipment_is_cargo(int equipment) {
  return equipment >= CARGO_BASE_INDEX;
}

static inline bool equipment_is_weapon(int equipment) {
  return equipment >= WEAPON_BASE_INDEX && equipment < AMMO_BASE_INDEX;
}

static inline int ammunition_to_weapon_index(int equipment) {
  return equipment - AMMO_BASE_INDEX;
}

static inline int bomb_equipment_index(int bomb) {
  return bomb + BOMB_BASE_INDEX;
}

static inline int bomb_from_equipment_index(int equipment) {
  return equipment - BOMB_BASE_INDEX;
}

static inline int special_equipment_index(int special) {
  return special + SPECIAL_BASE_INDEX;
}

static inline int special_from_equipment_index(int equipment) {
  return equipment - SPECIAL_BASE_INDEX;
}

static inline int cargo_equipment_index(int cargo) {
  return cargo + CARGO_BASE_INDEX;
}

static inline int cargo_from_equipment_index(int equipment) {
  return equipment - CARGO_BASE_INDEX;
}

static inline int weapon_equipment_index(int weapon) {
  return weapon + WEAPON_BASE_INDEX;
}

static inline int weapon_from_equipment_index(int equipment) {
  return equipment - WEAPON_BASE_INDEX;
}

static inline int ammunition_equipment_index(int weapon) {
  return weapon + AMMO_BASE_INDEX;
}

/* To define one of these-> x=SPECIAL_BASE_INDEX+SHOULDER_OR_HIP */
constexpr int SHOULDER_OR_HIP = 0;
constexpr int UPPER_ACTUATOR = 1;
constexpr int LOWER_ACTUATOR = 2;
constexpr int HAND_OR_FOOT_ACTUATOR = 3;
static inline bool equipment_is_actuator(int equipment) {
  return equipment_is_special(equipment) &&
         equipment <= special_equipment_index(HAND_OR_FOOT_ACTUATOR);
}
constexpr int LIFE_SUPPORT = 4;
constexpr int SENSORS = 5;
constexpr int COCKPIT = 6;
constexpr int ENGINE = 7;
constexpr int GYRO = 8;
constexpr int HEAT_SINK = 9;
constexpr int JUMP_JET = 10;
constexpr int CASE = 11;
constexpr int FERRO_FIBROUS = 12;
constexpr int ENDO_STEEL = 13;
constexpr int TRIPLE_STRENGTH_MYOMER = 14;
constexpr int TARGETING_COMPUTER = 15;
constexpr int MASC = 16;
constexpr int C3_MASTER = 17;
constexpr int C3_SLAVE = 18;
constexpr int BEAGLE_PROBE = 19;
constexpr int ARTEMIS_IV = 20;
constexpr int ECM = 21;
constexpr int AXE = 22;
constexpr int SWORD = 23;
constexpr int MACE = 24;
constexpr int CLAW = 25;
constexpr int DS_AERODOOR = 26;
constexpr int DS_MECHDOOR = 27;
constexpr int FUELTANK = 28;
constexpr int TAG = 29;
constexpr int DS_TANKDOOR = 30;
constexpr int DS_CARGODOOR = 31;
constexpr int LAMEQUIP = 32;
constexpr int CASE_II = 33;
constexpr int STEALTH_ARMOR = 34;
constexpr int NULL_SIGNATURE_SYSTEM = 35;
constexpr int C3I = 36;
constexpr int ANGELECM = 37;
constexpr int HVY_FERRO_FIBROUS = 38;
constexpr int LT_FERRO_FIBROUS = 39;
constexpr int BLOODHOUND_PROBE = 40;
constexpr int PURIFIER_ARMOR = 41;
constexpr int KAGE_STEALTH_UNIT = 42;
constexpr int ACHILEUS_STEALTH_UNIT = 43;
constexpr int INFILTRATOR_STEALTH_UNIT = 44;
constexpr int INFILTRATORII_STEALTH_UNIT = 45;
constexpr int SUPERCHARGER = 46;
constexpr int DUAL_SAW = 47;
constexpr int LIGHT_BAP = 48;
constexpr int SPLIT_CRIT_LEFT = 49;
constexpr int SPLIT_CRIT_RIGHT = 50;
constexpr int HARDPOINT = 51;

constexpr int LBX2_AMMO = 0;
constexpr int LBX5_AMMO = 1;
constexpr int LBX10_AMMO = 2;
constexpr int LBX20_AMMO = 3;
constexpr int LRM_AMMO = 4;
constexpr int SRM_AMMO = 5;
constexpr int SSRM_AMMO = 6;
constexpr int NARC_LRM_AMMO = 7;
constexpr int NARC_SRM_AMMO = 8;
constexpr int NARC_SSRM_AMMO = 9;
constexpr int ARTEMIS_LRM_AMMO = 10;
constexpr int ARTEMIS_SRM_AMMO = 11;
constexpr int ARTEMIS_SSRM_AMMO = 12;

constexpr int PETROLEUM = 13;
constexpr int PHOSPHORUS = 14;
constexpr int HYDROGEN = 15;
constexpr int GOLD = 16;
constexpr int NATURAL_EXTRACTS = 17;
constexpr int MARIJUANA = 18;
constexpr int SULFUR = 19;
constexpr int SODIUM = 20;
constexpr int PLUTONIUM = 21;
constexpr int ORE = 22;
constexpr int METAL = 23;
constexpr int PLASTICS = 24;
constexpr int MEDICAL_SUPPLIES = 25;
constexpr int COMPUTERS = 26;
constexpr int EXPLOSIVES = 27;

constexpr int ES_INTERNAL = 28;
constexpr int FF_ARMOR = 29;
constexpr int XL_ENGINE = 30;
constexpr int DOUBLE_HEAT_SINK = 31;
constexpr int IC_ENGINE = 32;

constexpr int S_ELECTRONIC = 33;
constexpr int S_INTERNAL = 34;
constexpr int S_ARMOR = 35;
constexpr int S_ACTUATOR = 36;
constexpr int S_AERO_FUEL = 37;
constexpr int S_DS_FUEL = 38;
constexpr int S_VTOL_FUEL = 39;

constexpr int SWARM_LRM_AMMO = 40;
constexpr int SWARM1_LRM_AMMO = 41;
constexpr int INFERNO_SRM_AMMO = 42;

constexpr int XXL_ENGINE = 43;
constexpr int COMP_ENGINE = 44;

constexpr int HD_ARMOR = 45;
constexpr int RE_INTERNAL = 46;
constexpr int CO_INTERNAL = 47;
constexpr int MRM_AMMO = 48;
constexpr int LIGHT_ENGINE = 49;
constexpr int CASEII = 50;
constexpr int STH_ARMOR = 51;
constexpr int NULLSIGSYS = 52;
constexpr int SILICON = 53;
constexpr int HVY_FF_ARMOR = 54;
constexpr int LT_FF_ARMOR = 55;

constexpr int INARC_EXPLO_AMMO = 56;
constexpr int INARC_HAYWIRE_AMMO = 57;
constexpr int INARC_ECM_AMMO = 58;
constexpr int INARC_NEMESIS_AMMO = 59;

constexpr int AC2_AP_AMMO = 60;
constexpr int AC5_AP_AMMO = 61;
constexpr int AC10_AP_AMMO = 62;
constexpr int AC20_AP_AMMO = 63;
constexpr int LAC2_AP_AMMO = 64;
constexpr int LAC5_AP_AMMO = 65;
constexpr int AC2_FLECHETTE_AMMO = 66;
constexpr int AC5_FLECHETTE_AMMO = 67;
constexpr int AC10_FLECHETTE_AMMO = 68;
constexpr int AC20_FLECHETTE_AMMO = 69;
constexpr int LAC2_FLECHETTE_AMMO = 70;
constexpr int LAC5_FLECHETTE_AMMO = 71;
constexpr int AC2_INCENDIARY_AMMO = 72;
constexpr int AC5_INCENDIARY_AMMO = 73;
constexpr int AC10_INCENDIARY_AMMO = 74;
constexpr int AC20_INCENDIARY_AMMO = 75;
constexpr int LAC2_INCENDIARY_AMMO = 76;
constexpr int LAC5_INCENDIARY_AMMO = 77;
constexpr int AC2_PRECISION_AMMO = 78;
constexpr int AC5_PRECISION_AMMO = 79;
constexpr int AC10_PRECISION_AMMO = 80;
constexpr int AC20_PRECISION_AMMO = 81;
constexpr int LAC2_PRECISION_AMMO = 82;
constexpr int LAC5_PRECISION_AMMO = 83;
constexpr int LR_DFM_AMMO = 84;
constexpr int SR_DFM_AMMO = 85;
constexpr int SLRM_AMMO = 86;
constexpr int ELRM_AMMO = 87;
constexpr int BSUIT_SENSOR = 88;
constexpr int BSUIT_LIFESUPPORT = 89;
constexpr int BSUIT_ELECTRONIC = 90;
constexpr int CARGO_OIL = 91;
constexpr int CARGO_WATER = 92;
constexpr int CARGO_EARTH = 93;
constexpr int CARGO_OXYGEN = 94;
constexpr int CARGO_NITROGEN = 95;
constexpr int CARGO_NICKEL = 96;
constexpr int CARGO_STEEL = 97;
constexpr int CARGO_IRON = 98;
constexpr int CARGO_BRASS = 99;
constexpr int CARGO_PLATINUM = 100;
constexpr int CARGO_COPPER = 101;
constexpr int CARGO_ALUMINUM = 102;
constexpr int CARGO_CONSUMER_GOOD = 103;
constexpr int CARGO_MACHINERY = 104;
constexpr int CARGO_SLAVES = 105;
constexpr int CARGO_TIMBIQUI_DARK = 106;
constexpr int CARGO_COCAINE = 107;
constexpr int CARGO_HEROINE = 108;
constexpr int CARGO_MARBLE = 109;
constexpr int CARGO_GLASS = 110;
constexpr int CARGO_DIAMOND = 111;
constexpr int CARGO_COAL = 112;
constexpr int CARGO_FOOD = 113;
constexpr int CARGO_ZINC = 114;
constexpr int CARGO_FABRIC = 115;
constexpr int CARGO_CLOTHING = 116;
constexpr int CARGO_WOOD = 117;
constexpr int CARGO_PULP = 118;
constexpr int CARGO_LUMBER = 119;
constexpr int CARGO_RUBBER = 120;
constexpr int CARGO_SEEDS = 121;
constexpr int CARGO_FERTILIZER = 122;
constexpr int CARGO_SALT = 123;
constexpr int CARGO_LITHIUM = 124;
constexpr int CARGO_HELIUM = 125;
constexpr int CARGO_LARIUM = 126;
constexpr int CARGO_URANIUM = 127;
constexpr int CARGO_IRIDIUM = 128;
constexpr int CARGO_TITANIUM = 129;
constexpr int CARGO_CONCRETE = 130;
constexpr int CARGO_FERROCRETE = 131;
constexpr int CARGO_BUILDING_SUPPLIES = 132;
constexpr int CARGO_KEVLAR = 133;
constexpr int CARGO_WASTE = 134;
constexpr int CARGO_LIVESTOCK = 135;
constexpr int CARGO_PAPER = 136;
constexpr int XL_GYRO = 137;
constexpr int HD_GYRO = 138;
constexpr int COMP_GYRO = 139;
constexpr int COMPACT_HEAT_SINK = 140;
constexpr int AMMO_LRM_STINGER = 141;
constexpr int AC2_CASELESS_AMMO = 142;
constexpr int AC5_CASELESS_AMMO = 143;
constexpr int AC10_CASELESS_AMMO = 144;
constexpr int AC20_CASELESS_AMMO = 145;
constexpr int LAC2_CASELESS_AMMO = 146;
constexpr int LAC5_CASELESS_AMMO = 147;
constexpr int AMMO_LRM_SGUIDED = 148;
constexpr int AMMO_ATM3_ER = 149;
constexpr int AMMO_ATM3_HE = 150;
constexpr int AMMO_ATM6_ER = 151;
constexpr int AMMO_ATM6_HE = 152;
constexpr int AMMO_ATM9_ER = 153;
constexpr int AMMO_ATM9_HE = 154;
constexpr int AMMO_ATM12_ER = 155;
constexpr int AMMO_ATM12_HE = 156;

/* Weapons structure and array... */
constexpr int TBEAM = 0;
constexpr int TMISSILE = 1;
constexpr int TARTILLERY = 2;
constexpr int TAMMO = 3;
constexpr int THAND = 4;

/* Tic status */

constexpr int TIC_NUM_DESTROYED = -2;
constexpr int TIC_NUM_RELOADING = -3;
constexpr int TIC_NUM_RECYCLING = -4;
constexpr int TIC_NUM_PHYSICAL = -5;

/* This is the max weapons per area- assuming 12 critical location and */
/* the smallest weapon requires 1 */
constexpr int MAX_WEAPS_SECTION = 12;

struct WeaponDefinition {
  const char *name;
  char vrt;
  char type;
  char heat;
  char damage;
  char min;
  int shortrange;
  int medrange;
  int longrange;
  char min_water;
  int shortrange_water;
  int medrange_water;
  int longrange_water;
  char criticals;
  unsigned char ammoperton;
  unsigned short weight; /* in 1/100ths tons */
  short explosiondamage; /* Damage done when exploding (GR/LGR/HGR) */
  long special;
  int battlevalue;
  int cost;
  int ammo_bv;
  int ammo_cost;
};

/* special weapon effects */
constexpr int NONE = 0x00000000;
constexpr int PULSE = 0x00000001;  /* Pulse laser */
constexpr int LBX = 0x00000002;    /* LBX AC */
constexpr int ULTRA = 0x00000004;  /* Ultra AC */
constexpr int STREAK = 0x00000008; /* Streak missile */
constexpr int GAUSS = 0x00000010;  /* Gauss weapon */
constexpr int NARC = 0x00000020;   /* NARC launcher */
constexpr int IDF = 0x00000040;    /* Can be used w/ IDF */
constexpr int DAR =
    0x00000080; /* Has artillery-level delay on hit (1sec/2hex) */
constexpr int HYPER = 0x00000100; /* Hyper AC */
constexpr int A_POD = 0x00000200; /* Anti-infantry Pod */
constexpr int CLAT = 0x00000400;  /* Clan-tech */
constexpr int NOSPA = 0x00000800; /* Does not allow special ammo (swarm, etc) */
constexpr int PC_HEAT =
    0x00001000; /* Heat-based PC weapon (laser/inferno/..) */
constexpr int PC_IMPA = 0x00002000; /* Impact (weapons) */
constexpr int PC_SHAR =
    0x00004000; /* Shrapnel / slash (various kinds of weapons) */
constexpr int AMS = 0x00008000;    /* AntiMissileSystem */
constexpr int NOBOOM = 0x00010000; /* No ammo boom */
constexpr int ATM = 0x00020000;    /* Was Caseless. Now ATM Missile */
constexpr int DFM = 0x00040000; /* DFM - 2 worst rolls outta 3 for missiles */
constexpr int ELRM =
    0x00080000; /* ELRM - 2 worst rolls outta 3 for missiles under */
constexpr int MRM = 0x00100000;   /* MRM - +1 BTH */
constexpr int CHEAT = 0x00200000; /* Can cause heat or damage */
#define HVYW                                                                   \
  0x00400000 /* Clam HeavyWeapons (call 'm so cuz FA$A will undoubtly bring    \
                more variants to the lasers) */
constexpr int RFAC = 0x00800000;     /* Rapid fire ACs */
constexpr int GMG = 0x01000000;      /* Gattling MGs */
constexpr int INARC = 0x02000000;    /* iNARC launcher */
constexpr int RAC = 0x04000000;      /* Rotary AC */
constexpr int HVYGAUSS = 0x08000000; /* Heavy Gauss */
constexpr int ROCKET =
    0x10000000; /* Rocket launchers. +1 to hit, one shot wonders */
#define SPLIT_CRITS                                                            \
  0x20000000 /* Certain weapons can split crits. Mark them appropriately */
constexpr int SNUBPPC = 0x40000000; /* Snub-nosed PPC */

#define PCOMBAT (PC_HEAT | PC_IMPA | PC_SHAR)

/* Section #defs... */

/* The unusual order is related to the locations of weapons of high */

/* magnitude versus weapons of low mag */
constexpr int LARM = 0;
constexpr int RARM = 1;
constexpr int LTORSO = 2;
constexpr int RTORSO = 3;
constexpr int CTORSO = 4;
constexpr int LLEG = 5;
constexpr int RLEG = 6;
constexpr int HEAD = 7;
constexpr int NUM_SECTIONS = 8;

/*  These defs are for Vehicles */
constexpr int LSIDE = 0;
constexpr int RSIDE = 1;
constexpr int FSIDE = 2;
constexpr int BSIDE = 3;
constexpr int TURRET = 4;
constexpr int ROTOR = 5;
constexpr int NUM_VEH_SECTIONS = 6;

/* Aerofighter */
constexpr int AERO_NOSE = 0;
constexpr int AERO_LWING = 1;
constexpr int AERO_RWING = 2;
constexpr int AERO_AFT = 3;
constexpr int NUM_AERO_SECTIONS = 4;

constexpr int NUM_BSUIT_MEMBERS = 8;

constexpr int DS_RWING = 0;  /* Right Front Side / Right Wing */
constexpr int DS_LWING = 1;  /* Left Front Side / Left Wing */
constexpr int DS_LRWING = 2; /* Left Rear Side */
constexpr int DS_RRWING = 3; /* Right Rear Side / Right Wing */
constexpr int DS_AFT = 4;
constexpr int DS_NOSE = 5;

constexpr int NUM_DS_SECTIONS = 6;

constexpr int NUM_TICS = 4;
constexpr int MAX_WEAPONS_PER_MECH = 96; /* Thanks to crit limits */
constexpr int SINGLE_TICLONG_SIZE = 32;
#define TICLONGS (MAX_WEAPONS_PER_MECH / SINGLE_TICLONG_SIZE)

/* structure for each critical hit section */
