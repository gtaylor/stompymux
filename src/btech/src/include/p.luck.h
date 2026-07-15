
/*
   p.luck.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Feb 22 14:59:38 CET 1999 from luck.c */

#pragma once

/* luck.c */
int player_luck(DbRef player);
int luck_die_mod_base(int mod, int l);
int luck_die_mod(DbRef player, int mod);
