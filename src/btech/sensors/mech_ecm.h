
/* Declares electronic countermeasure interfaces. */

#pragma once

#include "mech_lifecycle.h"
/* mech.ecm.c */
void cause_ecm(Mech *from, Mech *to);
void end_ecm_check(Mech *mech);
