/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#pragma once

#include <string.h>

#include "btconfig.h"
#include "missile_hit_registry.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mymath.h"
#define NUM_ITEMS 1024
#define NUM_ITEMS_M 512
#define NUM_BAYS 4
#define NUM_TURRETS 3
#define C3I_NETWORK_SIZE 5
#define C3_NETWORK_SIZE 11 /* Constant for the max size of the network */
#define BRANDCOUNT 5

#define LEFTSIDE 1
#define RIGHTSIDE 2
#define FRONT 3
#define BACK 4

#define STAND 1
#define FALL 0

#define TURN 30 /* 30 sec turn */
#define KPH_PER_MP 10.75
#define MP_PER_KPH 0.0930233              /* 1/KPH_PER_MP  */
#define MP_PER_UPDATE_PER_KPH 0.003100777 /* MP_PER_KPH/30 */
#define SCALEMAP 322.5                    /* 1/update      */
#define HEXLEVEL 5                        /* levels/hex    */
#define ZSCALE 64.5                       /* scalemap/hexlevel */
#define XSCALE 0.1547                     /* hex constant  */
#define YSCALE2 9.61482e-6                /* update**2     */
#define MP1 10.75                         /* 2*MS_PER_MP   */
#define MP2 21.50                         /* 2*MS_PER_MP   */
#define MP3 32.25                         /* 3*MS_PER_MP   */
#define MP4 43.00                         /* 4*MS_PER_MP   */
#define MP5 53.75                         /* 5*MS_PER_MP   */
#define MP6 64.50                         /* 6*MS_PER_MP   */
#define MP9 96.75                         /* 9*MS_PER_MP   */
#define DELTAFACING 1440.0

#define DEFAULT_FREQS 5
#define FREQS 16

#define FREQ_DIGITAL 1
#define FREQ_MUTE 2  /* For digital transmissions */
#define FREQ_RELAY 4 /* For digital transmissions */
#define FREQ_INFO 8  /* For digital transmissions */
#define FREQ_SCAN 16
#define FREQ_REST 32

#define RADIO_RELAY 1 /* ability to relay things */
#define RADIO_INFO 2  /* ability to see where (digital) message comes from */
#define RADIO_SCAN 4  /* ability to scan for frequencies */
#define RADIO_NODIGITAL 8 /* lacks the ability to hear or set digital freqs */

#define CHTITLELEN 15

#define NOT_FOUND -1
#define NUM_CRITICALS 12

#define ARMOR 1
#define INTERNAL 2
#define REAR 3

#define NOARC 0
#define FORWARDARC 1
#define LSIDEARC 2
#define RSIDEARC 4
#define REARARC 8
#define TURRETARC 16

/*
   Critical Types
   0       Empty
   1-192   Weapons
   193-384 Ammo
   385-394 Bombs (Aero/VTOL droppable)
   395-511 Special startings...
 */

/* Critical Types... */
#define NUM_WEAPONS 192
#define NUM_BOMBS 9

#define EMPTY 0
#define WEAPON_BASE_INDEX 1
#define AMMO_BASE_INDEX (WEAPON_BASE_INDEX + NUM_WEAPONS) /* 193 */
#define BOMB_BASE_INDEX (AMMO_BASE_INDEX + NUM_WEAPONS)   /* 385 */
#define SPECIAL_BASE_INDEX (BOMB_BASE_INDEX + NUM_BOMBS)  /* 394 */
#define OSPECIAL_BASE_INDEX 220
#define CARGO_BASE_INDEX 512

#ifdef BT_ADVANCED_ECON
#define SPECIALCOST_SIZE (CARGO_BASE_INDEX - SPECIAL_BASE_INDEX)
#define AMMOCOST_SIZE NUM_WEAPONS
#define WEAPCOST_SIZE NUM_WEAPONS
#define CARGOCOST_SIZE (NUM_ITEMS - NUM_ITEMS_M)
#define BOMBCOST_SIZE NUM_BOMBS
#endif

#define IsAmmo(a) ((a) >= AMMO_BASE_INDEX && (a) < BOMB_BASE_INDEX)
#define IsBomb(a) ((a) >= BOMB_BASE_INDEX && (a) < SPECIAL_BASE_INDEX)
#define IsSpecial(a) ((a) >= SPECIAL_BASE_INDEX && (a) < CARGO_BASE_INDEX)
#define IsCargo(a) ((a) >= CARGO_BASE_INDEX)
#define IsActuator(a) (IsSpecial(a) && a <= I2Special(HAND_OR_FOOT_ACTUATOR))
#define IsWeapon(a) ((a) >= WEAPON_BASE_INDEX && (a) < AMMO_BASE_INDEX)
#define IsArtillery(a) (MechWeapons[a].type == TARTILLERY)
#define IsMissile(a) (MechWeapons[a].type == TMISSILE)
#define IsBallistic(a) (MechWeapons[a].type == TAMMO)
#define IsEnergy(a) (MechWeapons[a].type == TBEAM)

/* Fun Weapons that do affects */
#define IsFlamer(a) (strstr(MechWeapons[a].name, "Flamer"))
#define IsCoolant(a) (strstr(MechWeapons[a].name, "Coolant"))
#define IsAcid(a) (strstr(MechWeapons[a].name, "Acid"))

#define GunRangeWithCheck(mech, sec, a)                                        \
  (SectionUnderwater(mech, sec) > 0 ? GunWaterRange(a)                         \
   : IsArtillery(a) ? (ARTILLERY_MAPSHEET_SIZE * MechWeapons[a].longrange)     \
                    : (MechWeapons[a].longrange))
#define EGunRangeWithCheck(mech, sec, a)                                       \
  ((SectionUnderwater(mech, sec) > 0)                                          \
       ? EGunWaterRange((mech)->xcode.context->configuration, a)               \
   : ((mech)->xcode.context->configuration->btech_erange &&                    \
      (MechWeapons[a].medrange * 2) > GunRange(a))                             \
       ? (MechWeapons[a].medrange * 2)                                         \
       : GunRange(a))
#define GunRange(a)                                                            \
  (IsArtillery(a) ? (ARTILLERY_MAPSHEET_SIZE * MechWeapons[a].longrange)       \
                  : (MechWeapons[a].longrange))
#define EGunRange(configuration, a)                                            \
  (((configuration)->btech_erange &&                                           \
    (MechWeapons[a].medrange * 2) > GunRange(a))                               \
       ? (MechWeapons[a].medrange * 2)                                         \
       : GunRange(a))
#define GunWaterRange(a)                                                       \
  (MechWeapons[a].longrange_water > 0    ? MechWeapons[a].longrange_water      \
   : MechWeapons[a].medrange_water > 0   ? MechWeapons[a].medrange_water       \
   : MechWeapons[a].shortrange_water > 0 ? MechWeapons[a].shortrange_water     \
                                         : 0)
#define EGunWaterRange(configuration, a)                                       \
  (((configuration)->btech_erange &&                                           \
    ((MechWeapons[a].medrange_water * 2) > GunWaterRange(a)) &&                \
    (MechWeapons[a].longrange_water > 0))                                      \
       ? (MechWeapons[a].medrange_water * 2)                                   \
       : GunWaterRange(a))
#define SectionUnderwater(mech, sec)                                           \
  (MechZ(mech) >= 0                       ? 0                                  \
   : (MechZ(mech) < -1) || (Fallen(mech)) ? 1                                  \
   : ((sec == LLEG) || (sec == RLEG)) ||                                       \
           (MechIsQuad(mech) && ((sec == LARM) || (sec == RARM)))              \
       ? 1                                                                     \
       : 0)

#define Ammo2WeaponI(a) ((a) - AMMO_BASE_INDEX)
#define Ammo2Weapon(a) Ammo2WeaponI(a)
#define Ammo2I(a) Ammo2Weapon(a)
#define Bomb2I(a) ((a) - BOMB_BASE_INDEX)
#define Special2I(a) ((a) - SPECIAL_BASE_INDEX)
#define Cargo2I(a) ((a) - CARGO_BASE_INDEX)
#define Weapon2I(a) ((a) - WEAPON_BASE_INDEX)
#define I2Bomb(a) ((a) + BOMB_BASE_INDEX)
#define I2Weapon(a) ((a) + WEAPON_BASE_INDEX)
#define I2Ammo(a) ((a) + AMMO_BASE_INDEX)
#define I2Special(a) ((a) + SPECIAL_BASE_INDEX)
#define I2Cargo(a) ((a) + CARGO_BASE_INDEX)
#define Special I2Special
#define Cargo I2Cargo

/* To define one of these-> x=SPECIAL_BASE_INDEX+SHOULDER_OR_HIP */
#define SHOULDER_OR_HIP 0
#define UPPER_ACTUATOR 1
#define LOWER_ACTUATOR 2
#define HAND_OR_FOOT_ACTUATOR 3
#define LIFE_SUPPORT 4
#define SENSORS 5
#define COCKPIT 6
#define ENGINE 7
#define GYRO 8
#define HEAT_SINK 9
#define JUMP_JET 10
#define CASE 11
#define FERRO_FIBROUS 12
#define ENDO_STEEL 13
#define TRIPLE_STRENGTH_MYOMER 14
#define TARGETING_COMPUTER 15
#define MASC 16
#define C3_MASTER 17
#define C3_SLAVE 18
#define BEAGLE_PROBE 19
#define ARTEMIS_IV 20
#define ECM 21
#define AXE 22
#define SWORD 23
#define MACE 24
#define CLAW 25
#define DS_AERODOOR 26
#define DS_MECHDOOR 27
#define FUELTANK 28
#define TAG 29
#define DS_TANKDOOR 30
#define DS_CARGODOOR 31
#define LAMEQUIP 32
#define CASE_II 33
#define STEALTH_ARMOR 34
#define NULL_SIGNATURE_SYSTEM 35
#define C3I 36
#define ANGELECM 37
#define HVY_FERRO_FIBROUS 38
#define LT_FERRO_FIBROUS 39
#define BLOODHOUND_PROBE 40
#define PURIFIER_ARMOR 41
#define KAGE_STEALTH_UNIT 42
#define ACHILEUS_STEALTH_UNIT 43
#define INFILTRATOR_STEALTH_UNIT 44
#define INFILTRATORII_STEALTH_UNIT 45
#define SUPERCHARGER 46
#define DUAL_SAW 47
#define LIGHT_BAP 48
#define SPLIT_CRIT_LEFT 49
#define SPLIT_CRIT_RIGHT 50
#define HARDPOINT 51

#define LBX2_AMMO 0
#define LBX5_AMMO 1
#define LBX10_AMMO 2
#define LBX20_AMMO 3
#define LRM_AMMO 4
#define SRM_AMMO 5
#define SSRM_AMMO 6
#define NARC_LRM_AMMO 7
#define NARC_SRM_AMMO 8
#define NARC_SSRM_AMMO 9
#define ARTEMIS_LRM_AMMO 10
#define ARTEMIS_SRM_AMMO 11
#define ARTEMIS_SSRM_AMMO 12

#define PETROLEUM 13
#define PHOSPHORUS 14
#define HYDROGEN 15
#define GOLD 16
#define NATURAL_EXTRACTS 17
#define MARIJUANA 18
#define SULFUR 19
#define SODIUM 20
#define PLUTONIUM 21
#define ORE 22
#define METAL 23
#define PLASTICS 24
#define MEDICAL_SUPPLIES 25
#define COMPUTERS 26
#define EXPLOSIVES 27

#define ES_INTERNAL 28
#define FF_ARMOR 29
#define XL_ENGINE 30
#define DOUBLE_HEAT_SINK 31
#define IC_ENGINE 32

#define S_ELECTRONIC 33
#define S_INTERNAL 34
#define S_ARMOR 35
#define S_ACTUATOR 36
#define S_AERO_FUEL 37
#define S_DS_FUEL 38
#define S_VTOL_FUEL 39

#define SWARM_LRM_AMMO 40
#define SWARM1_LRM_AMMO 41
#define INFERNO_SRM_AMMO 42

#define XXL_ENGINE 43
#define COMP_ENGINE 44

#define HD_ARMOR 45
#define RE_INTERNAL 46
#define CO_INTERNAL 47
#define MRM_AMMO 48
#define LIGHT_ENGINE 49
#define CASEII 50
#define STH_ARMOR 51
#define NULLSIGSYS 52
#define SILICON 53
#define HVY_FF_ARMOR 54
#define LT_FF_ARMOR 55

#define INARC_EXPLO_AMMO 56
#define INARC_HAYWIRE_AMMO 57
#define INARC_ECM_AMMO 58
#define INARC_NEMESIS_AMMO 59

#define AC2_AP_AMMO 60
#define AC5_AP_AMMO 61
#define AC10_AP_AMMO 62
#define AC20_AP_AMMO 63
#define LAC2_AP_AMMO 64
#define LAC5_AP_AMMO 65
#define AC2_FLECHETTE_AMMO 66
#define AC5_FLECHETTE_AMMO 67
#define AC10_FLECHETTE_AMMO 68
#define AC20_FLECHETTE_AMMO 69
#define LAC2_FLECHETTE_AMMO 70
#define LAC5_FLECHETTE_AMMO 71
#define AC2_INCENDIARY_AMMO 72
#define AC5_INCENDIARY_AMMO 73
#define AC10_INCENDIARY_AMMO 74
#define AC20_INCENDIARY_AMMO 75
#define LAC2_INCENDIARY_AMMO 76
#define LAC5_INCENDIARY_AMMO 77
#define AC2_PRECISION_AMMO 78
#define AC5_PRECISION_AMMO 79
#define AC10_PRECISION_AMMO 80
#define AC20_PRECISION_AMMO 81
#define LAC2_PRECISION_AMMO 82
#define LAC5_PRECISION_AMMO 83
#define LR_DFM_AMMO 84
#define SR_DFM_AMMO 85
#define SLRM_AMMO 86
#define ELRM_AMMO 87
#define BSUIT_SENSOR 88
#define BSUIT_LIFESUPPORT 89
#define BSUIT_ELECTRONIC 90
#define CARGO_OIL 91
#define CARGO_WATER 92
#define CARGO_EARTH 93
#define CARGO_OXYGEN 94
#define CARGO_NITROGEN 95
#define CARGO_NICKEL 96
#define CARGO_STEEL 97
#define CARGO_IRON 98
#define CARGO_BRASS 99
#define CARGO_PLATINUM 100
#define CARGO_COPPER 101
#define CARGO_ALUMINUM 102
#define CARGO_CONSUMER_GOOD 103
#define CARGO_MACHINERY 104
#define CARGO_SLAVES 105
#define CARGO_TIMBIQUI_DARK 106
#define CARGO_COCAINE 107
#define CARGO_HEROINE 108
#define CARGO_MARBLE 109
#define CARGO_GLASS 110
#define CARGO_DIAMOND 111
#define CARGO_COAL 112
#define CARGO_FOOD 113
#define CARGO_ZINC 114
#define CARGO_FABRIC 115
#define CARGO_CLOTHING 116
#define CARGO_WOOD 117
#define CARGO_PULP 118
#define CARGO_LUMBER 119
#define CARGO_RUBBER 120
#define CARGO_SEEDS 121
#define CARGO_FERTILIZER 122
#define CARGO_SALT 123
#define CARGO_LITHIUM 124
#define CARGO_HELIUM 125
#define CARGO_LARIUM 126
#define CARGO_URANIUM 127
#define CARGO_IRIDIUM 128
#define CARGO_TITANIUM 129
#define CARGO_CONCRETE 130
#define CARGO_FERROCRETE 131
#define CARGO_BUILDING_SUPPLIES 132
#define CARGO_KEVLAR 133
#define CARGO_WASTE 134
#define CARGO_LIVESTOCK 135
#define CARGO_PAPER 136
#define XL_GYRO 137
#define HD_GYRO 138
#define COMP_GYRO 139
#define COMPACT_HEAT_SINK 140
#define AMMO_LRM_STINGER 141
#define AC2_CASELESS_AMMO 142
#define AC5_CASELESS_AMMO 143
#define AC10_CASELESS_AMMO 144
#define AC20_CASELESS_AMMO 145
#define LAC2_CASELESS_AMMO 146
#define LAC5_CASELESS_AMMO 147
#define AMMO_LRM_SGUIDED 148
#define AMMO_ATM3_ER 149
#define AMMO_ATM3_HE 150
#define AMMO_ATM6_ER 151
#define AMMO_ATM6_HE 152
#define AMMO_ATM9_ER 153
#define AMMO_ATM9_HE 154
#define AMMO_ATM12_ER 155
#define AMMO_ATM12_HE 156

#ifdef BT_COMPLEXREPAIRS
#define TON_SENSORS_FIRST 157
#define TON_SENSORS_LAST (TON_SENSORS_FIRST + 9)

#define TON_MYOMER_FIRST (TON_SENSORS_LAST + 1)
#define TON_MYOMER_LAST (TON_MYOMER_FIRST + 9)

#define TON_TRIPLEMYOMER_FIRST (TON_MYOMER_LAST + 1)
#define TON_TRIPLEMYOMER_LAST (TON_TRIPLEMYOMER_FIRST + 9)

#define TON_INTERNAL_FIRST (TON_TRIPLEMYOMER_LAST + 1)
#define TON_INTERNAL_LAST (TON_INTERNAL_FIRST + 9)

#define TON_ESINTERNAL_FIRST (TON_INTERNAL_LAST + 1)
#define TON_ESINTERNAL_LAST (TON_ESINTERNAL_FIRST + 9)

#define TON_JUMPJET_FIRST (TON_ESINTERNAL_LAST + 1)
#define TON_JUMPJET_LAST (TON_JUMPJET_FIRST + 9)

#define TON_ARMUPPER_FIRST (TON_JUMPJET_LAST + 1)
#define TON_ARMUPPER_LAST (TON_ARMUPPER_FIRST + 9)

#define TON_ARMLOWER_FIRST (TON_ARMUPPER_LAST + 1)
#define TON_ARMLOWER_LAST (TON_ARMLOWER_FIRST + 9)

#define TON_ARMHAND_FIRST (TON_ARMLOWER_LAST + 1)
#define TON_ARMHAND_LAST (TON_ARMHAND_FIRST + 9)

#define TON_LEGUPPER_FIRST (TON_ARMHAND_LAST + 1)
#define TON_LEGUPPER_LAST (TON_LEGUPPER_FIRST + 9)

#define TON_LEGLOWER_FIRST (TON_LEGUPPER_LAST + 1)
#define TON_LEGLOWER_LAST (TON_LEGLOWER_FIRST + 9)

#define TON_LEGFOOT_FIRST (TON_LEGLOWER_LAST + 1)
#define TON_LEGFOOT_LAST (TON_LEGFOOT_FIRST + 9)

#define TON_ENGINE_FIRST (TON_LEGFOOT_LAST + 1)
#define TON_ENGINE_LAST (TON_ENGINE_FIRST + 19)

#define TON_ENGINE_XL_FIRST (TON_ENGINE_LAST + 1)
#define TON_ENGINE_XL_LAST (TON_ENGINE_XL_FIRST + 19)

#define TON_ENGINE_ICE_FIRST (TON_ENGINE_XL_LAST + 1)
#define TON_ENGINE_ICE_LAST (TON_ENGINE_ICE_FIRST + 19)

#define TON_ENGINE_LIGHT_FIRST (TON_ENGINE_ICE_LAST + 1)
#define TON_ENGINE_LIGHT_LAST (TON_ENGINE_LIGHT_FIRST + 19)

#define TON_COINTERNAL_FIRST (TON_ENGINE_LIGHT_LAST + 1)
#define TON_COINTERNAL_LAST (TON_COINTERNAL_FIRST + 9)

#define TON_REINTERNAL_FIRST (TON_COINTERNAL_LAST + 1)
#define TON_REINTERNAL_LAST (TON_REINTERNAL_FIRST + 9)

#define TON_GYRO_FIRST (TON_REINTERNAL_LAST + 1)
#define TON_GYRO_LAST (TON_GYRO_FIRST + 3)

#define TON_XLGYRO_FIRST (TON_GYRO_LAST + 1)
#define TON_XLGYRO_LAST (TON_XLGYRO_FIRST + 3)

#define TON_HDGYRO_FIRST (TON_XLGYRO_LAST + 1)
#define TON_HDGYRO_LAST (TON_HDGYRO_FIRST + 3)

#define TON_CGYRO_FIRST (TON_HDGYRO_LAST + 1)
#define TON_CGYRO_LAST (TON_CGYRO_FIRST + 3)

#define TON_ENGINE_XXL_FIRST (TON_CGYRO_LAST + 1)
#define TON_ENGINE_XXL_LAST (TON_ENGINE_XXL_FIRST + 19)

#define TON_ENGINE_COMP_FIRST (TON_ENGINE_XXL_LAST + 1)
#define TON_ENGINE_COMP_LAST (TON_ENGINE_COMP_FIRST + 19)
#endif

/* Weapons structure and array... */
#define TBEAM 0
#define TMISSILE 1
#define TARTILLERY 2
#define TAMMO 3
#define THAND 4

/* Tic status */

#define TIC_NUM_DESTROYED -2
#define TIC_NUM_RELOADING -3
#define TIC_NUM_RECYCLING -4
#define TIC_NUM_PHYSICAL -5

/* This is the max weapons per area- assuming 12 critical location and */
/* the smallest weapon requires 1 */
#define MAX_WEAPS_SECTION 12

struct WeaponDefinition {
  char *name;
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
#define NONE 0x00000000
#define PULSE 0x00000001   /* Pulse laser */
#define LBX 0x00000002     /* LBX AC */
#define ULTRA 0x00000004   /* Ultra AC */
#define STREAK 0x00000008  /* Streak missile */
#define GAUSS 0x00000010   /* Gauss weapon */
#define NARC 0x00000020    /* NARC launcher */
#define IDF 0x00000040     /* Can be used w/ IDF */
#define DAR 0x00000080     /* Has artillery-level delay on hit (1sec/2hex) */
#define HYPER 0x00000100   /* Hyper AC */
#define A_POD 0x00000200   /* Anti-infantry Pod */
#define CLAT 0x00000400    /* Clan-tech */
#define NOSPA 0x00000800   /* Does not allow special ammo (swarm, etc) */
#define PC_HEAT 0x00001000 /* Heat-based PC weapon (laser/inferno/..) */
#define PC_IMPA 0x00002000 /* Impact (weapons) */
#define PC_SHAR 0x00004000 /* Shrapnel / slash (various kinds of weapons) */
#define AMS 0x00008000     /* AntiMissileSystem */
#define NOBOOM 0x00010000  /* No ammo boom */
#define ATM 0x00020000     /* Was Caseless. Now ATM Missile */
#define DFM 0x00040000     /* DFM - 2 worst rolls outta 3 for missiles */
#define ELRM 0x00080000    /* ELRM - 2 worst rolls outta 3 for missiles under */
#define MRM 0x00100000     /* MRM - +1 BTH */
#define CHEAT 0x00200000   /* Can cause heat or damage */
#define HVYW                                                                   \
  0x00400000 /* Clam HeavyWeapons (call 'm so cuz FA$A will undoubtly bring    \
                more variants to the lasers) */
#define RFAC 0x00800000     /* Rapid fire ACs */
#define GMG 0x01000000      /* Gattling MGs */
#define INARC 0x02000000    /* iNARC launcher */
#define RAC 0x04000000      /* Rotary AC */
#define HVYGAUSS 0x08000000 /* Heavy Gauss */
#define ROCKET 0x10000000   /* Rocket launchers. +1 to hit, one shot wonders */
#define SPLIT_CRITS                                                            \
  0x20000000 /* Certain weapons can split crits. Mark them appropriately */
#define SNUBPPC 0x40000000 /* Snub-nosed PPC */

#define PCOMBAT (PC_HEAT | PC_IMPA | PC_SHAR)

/* Section #defs... */

/* The unusual order is related to the locations of weapons of high */

/* magnitude versus weapons of low mag */
#define LARM 0
#define RARM 1
#define LTORSO 2
#define RTORSO 3
#define CTORSO 4
#define LLEG 5
#define RLEG 6
#define HEAD 7
#define NUM_SECTIONS 8

/*  These defs are for Vehicles */
#define LSIDE 0
#define RSIDE 1
#define FSIDE 2
#define BSIDE 3
#define TURRET 4
#define ROTOR 5
#define NUM_VEH_SECTIONS 6

/* Aerofighter */
#define AERO_NOSE 0
#define AERO_LWING 1
#define AERO_RWING 2
#define AERO_AFT 3
#define NUM_AERO_SECTIONS 4

#define NUM_BSUIT_MEMBERS 8

#define DS_RWING 0  /* Right Front Side / Right Wing */
#define DS_LWING 1  /* Left Front Side / Left Wing */
#define DS_LRWING 2 /* Left Rear Side */
#define DS_RRWING 3 /* Right Rear Side / Right Wing */
#define DS_AFT 4
#define DS_NOSE 5

#define NUM_DS_SECTIONS 6
#define SpheroidDS(a) (MechType(a) == CLASS_SPHEROID_DS)
#define SpheroidToRear(mech, a)                                                \
  if (MechType(mech) == CLASS_SPHEROID_DS)                                     \
  (a) = ((a) == DS_LWING ? DS_LRWING : DS_RRWING)

#define NUM_TICS 4
#define MAX_WEAPONS_PER_MECH 96 /* Thanks to crit limits */
#define SINGLE_TICLONG_SIZE 32
#define TICLONGS (MAX_WEAPONS_PER_MECH / SINGLE_TICLONG_SIZE)

/* structure for each critical hit section */
