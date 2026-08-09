
/*
 * $Id: btechstats_global.h,v 1.1.1.1 2005/01/11 21:18:03 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Thu Sep 19 22:40:49 1996 fingon
 * Last modified: Sat Jun  6 20:20:38 1998 fingon
 *
 */

#pragma once

constexpr int VALUES_HEALTH = 1; /* Bruise and lethal damage */
constexpr int VALUES_SKILLS = 2; /* Skill values and experience */
constexpr int VALUES_ATTRS = 4;  /* Core character attributes */
constexpr int VALUES_ADVS = 8;   /* Advantages and lives */
constexpr int VALUES_ALL = 15;
constexpr int VALUES_CO = 6; /* Attr + Skill */

/* hmm. */

constexpr int CHAR_VALUE = 0;
constexpr int CHAR_SKILL = 1;
constexpr int CHAR_ADVANTAGE = 2;
constexpr int CHAR_ATTRIBUTE = 3;

/* 4 diff. skill types */

constexpr int CHAR_ATHLETIC = 0x0001;
constexpr int CHAR_MENTAL = 0x0002;
constexpr int CHAR_PHYSICAL = 0x0004;
constexpr int CHAR_SOCIAL = 0x0008;

/* Career-types */
constexpr int CAREER_CAVALRY = 0x0010;   /* Drive + Gun-Conv */
constexpr int CAREER_BMECH = 0x0020;     /* Bmech Pilot/Gun */
constexpr int CAREER_AERO = 0x0040;      /* Aero Pilot/Gun */
constexpr int CAREER_ARTILLERY = 0x0080; /* Artillery-Gun */
constexpr int CAREER_DROPSHIP = 0x0100;  /* Dropship Pilot/Gun */
constexpr int CAREER_TECHMECH = 0x0200;
constexpr int CAREER_TECHVEH = 0x0400;
constexpr int CAREER_TECH = CAREER_TECHMECH | CAREER_TECHVEH;
constexpr int CAREER_MISC = 0x0800;
constexpr int CAREER_ACADMISC = 0x1000;
constexpr int CAREER_RECON = 0x2000;
constexpr int SK_XP = 0x4000;           /* Always raise xp (not spammable) */
constexpr int XP_MAX = 256 * 256 * 256; /* Then we wrap ; tough beans */

/* 3 diff. adv types */

constexpr int CHAR_ADV_VALUE = 0;
constexpr int CHAR_ADV_BOOL = 1;
constexpr int CHAR_ADV_EXCEPT = 2;

constexpr int CHAR_BLD = 1;
constexpr int CHAR_REF = 2;
constexpr int CHAR_INT = 4;
constexpr int CHAR_LRN = 8;
constexpr int CHAR_CHA = 16;

constexpr int GREEN = 0;
constexpr int REGULAR = 1;
constexpr int VETEREN = 2;
constexpr int ELITE = 3;
constexpr int HISTORICAL = 4;

#include "btech_api.h"
#include "btechstats_api.h"

constexpr int EE_NUMBER = 11;
constexpr int LIVES_NUMBER = 5;
