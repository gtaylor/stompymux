
/*
   p.mech.tech.damages.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:59 CET 1999 from mech.tech.damages.c */

#pragma once

/* mech.tech.damages.c */
size_t mech_repair_job_count(MECH *mech);
void mech_repair_jobs_format(MECH *mech, char *buffer, size_t buffer_size);
void show_mechs_damage(DbRef player, void *data, char *buffer);
void tech_fix(DbRef player, void *data, char *buffer);
