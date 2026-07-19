
/*
 * $Id: macros.h,v 1.1 2005/06/13 20:50:52 murrayma Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Wed Oct  9 10:49:55 1996 fingon
 * Last modified: Wed Aug 12 14:36:33 1998 fingon
 *
 */

#pragma once

#include "mux/server/server_api.h"
#include "mux/world/world_context.h"

#ifdef notify
#undef notify
#endif
#ifdef notify_except
#undef notify_except
#endif
#define notify(evaluation, player, message)                                    \
  mecha_notify(evaluation, player, message)
#define notify_except(evaluation, location, player, exception, message)        \
  mecha_notify_except(evaluation, location, player, exception, message)

/* DOCHECK: Macros for the lazy
   DOCHEK(a,b) basically replaces if (a) { <somewaytopassmessage b>, return }

   Macros ending with 0 return 0, and N NULL. Default behaviour is to
   return nothing.

   Default behavior to pass message is to use notify to notify the
   player who's executing the function. MA = MECHALL, sends to
   everyone within the mech executing the command, MP = MECHPILOT,
   sends to pilot of the mech
   */
#ifndef DOCHECK_CONTEXT
#define DOCHECK_CONTEXT(context, a, b)                                         \
  if (a) {                                                                     \
    notify(btech_context_evaluation(context), player, b);                      \
    return;                                                                    \
  }
#define DOCHECK0_CONTEXT(context, a, b)                                        \
  if (a) {                                                                     \
    notify(btech_context_evaluation(context), player, b);                      \
    return 0;                                                                  \
  }
#define DOCHECK1_CONTEXT(context, a, b)                                        \
  if (a) {                                                                     \
    notify(btech_context_evaluation(context), player, b);                      \
    return -1;                                                                 \
  }
#define DOCHECKN_CONTEXT(context, a, b)                                        \
  if (a) {                                                                     \
    notify(btech_context_evaluation(context), player, b);                      \
    return nullptr;                                                            \
  }
#define DOCHECKMA(a, b)                                                        \
  if (a) {                                                                     \
    mech_notify(mech, MECHALL, b);                                             \
    return;                                                                    \
  }
#define DOCHECKMA0(a, b)                                                       \
  if (a) {                                                                     \
    mech_notify(mech, MECHALL, b);                                             \
    return 0;                                                                  \
  }
#define DOCHECKMP(a, b)                                                        \
  if (a) {                                                                     \
    mech_notify(mech, MECHPILOT, b);                                           \
    return;                                                                    \
  }
#define DOCHECKMP0(a, b)                                                       \
  if (a) {                                                                     \
    mech_notify(mech, MECHPILOT, b);                                           \
    return 0;                                                                  \
  }
#define DOCHECKMP1(a, b)                                                       \
  if (a) {                                                                     \
    mech_notify(mech, MECHPILOT, b);                                           \
    return 1;                                                                  \
  }
#define FUNCHECK(a, b)                                                         \
  if (a) {                                                                     \
    safe_tprintf_str(buff, bufc, b);                                           \
    return;                                                                    \
  }
#endif

#define skipws(name)                                                           \
  while (name && *name && isspace(*name))                                      \
  name++
#define readint(to, from)                                                      \
  (!from || !*from || (!(to = atoi(from)) && strcmp(from, "0")))

#define Readnum(tovar, fromvar)                                                \
  (!(tovar = atoi(fromvar)) && strcmp(fromvar, "0"))

#define SetBit(val, bit) (val |= bit)
#define UnSetBit(val, bit) (val &= ~(bit))
#define EvalBit(val, bit, state)                                               \
  do {                                                                         \
    if (state)                                                                 \
      SetBit(val, bit);                                                        \
    else                                                                       \
      UnSetBit(val, bit);                                                      \
  } while (0)
#define ToggleBit(val, bit)                                                    \
  do {                                                                         \
    if (!(val & bit))                                                          \
      SetBit(val, bit);                                                        \
    else                                                                       \
      UnSetBit(val, bit);                                                      \
  } while (0)

#define WizPo(database, p, fun)                                                \
  (fun(database, game_object_owner(database, p)) && is_inherits(database, p))

#define Wiz(database, p) WizPo(database, p, is_wizard)
#define WizR(database, p) Wiz(database, p)
#define WizP(database, p) WizPo(database, p, is_security)
