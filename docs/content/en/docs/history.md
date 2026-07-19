---
title: History
type: docs
weight: 100
---

This codebase is the result of over 30 years of development in one form or
another. It started hosted on a TinyMUSE server, around 1992 or 1993. Its primary
instance powered the most popular Battletech MU* to date, 3056 MUSE. 

There after, the code was ported to/reimplemented for TinyMUSH by the Animudiacs crew,
who ran, amongst other servers, 3034-II (which later evolved into 3035) and
Varxsis (a non-battletech universe with similar theme and technology). Somewhat
simultaneously, Markus Stenberg (aka Fingon) ported the original MUSE code
to TinyMUX, implementing a lot of ideas from the Animudiacs sites and
implementing a lot more from scratch, both official Battletech rules and
brand new ideas. The main MUX site has always been 3030 MUX.

Fingon continued development until about 1998, after which he released the
source to the public under a restricted license and retired. Development for
3030 was continued by several of the 3030 MUX Wizards, most notably 
Thomas Wouters (Focus) and Cord Awtry (Spectre).

In 1999, a new BattletechMUX using the restricted license source opened up
under the banner of 'Exile: Exodus', with development efforts lead by Null
(Kevin Stevens). Exile took an aggressive approach to development and many
modifications, rewrites, and additions were made to the now ancient
source. While some parallel development was done in coordination with
the staff of 3030, the two source trees remained largely seperate due to 
different licensing.

After some requests from people running their own spinoffs from the original
'public' release by Fingon, Fingon, Spectre, and Focus wrote a new licence:
the 3030 MUX Artistic License, and released the actual 3030 MUX source tree
under it. The MUX source then moved to SourceForge for its main development.

Limited by the licensing on the old release, the Exile source tree was
unable to release its source. Attempts were made to contact Fingon to get the
old tree re-licensed, but over the span of several years nothing was heard
from him.

In late 2004 and early 2005, Null began to port his changes to the Exile
base to the open-sourced 3030 branch.

The torch was then picked up and carried by a development team
headed up by Kelvin McCorvin, composed of many of the people in the
credits below. With Hag as lead programmer overseeing a capable team of
developers, the codebase grew in both scope and quality.

## Credits

Much credit is due to many. Unfortunately due to the long and storied history of 
the source code as well as many of the included ideas borrowed from other MUSEs, 
MUSHes and MUXes that inspired this MUX code, it presents an impossible task.

Anyone who worked on TinyMUSE, TinyMUSH and TinyMUX, both with and without 
Battletech extensions, as well as those who built and ran the Battletech MUSE,
MUSH and MUX worlds out there is deserving of credit. If that is you, and you 
want to be listed in this credits file, let us know.

## BattletechMUX Credits file

Presented from the last BattletechMUX archive's `CREDITS` file here for preservation:

```
BattletechMUX Credits
---------------------
BattletechMUX is maintained by a group of volunteers, all working towards the
continued improvement of the codebase. Without any of the individuals listed
below, the BTMux scene might be very different.

The following individuals have made contributions of some sort towards the
betterment of BattletechMUX. If we have omitted anyone, please let us know.

Major Contributors
------------------
The following people have invested a substantial amount of time working on
BattletechMUX in some way. Most of these have spent years with the project
as it went through its many incarnations, and without any one of the
below listed, a lot would be missing.

* Guzzer and Crew  - Very early development, started one of the first big
                     BT MU*s.
* Fingon           - Lots of code work, much of it is still in the base
                     today.
* Kip              - Development lead while the codebase was being
                     maintained by Battletech: 3030.
* Focus            - Code contributions. 
* Null/DJ          - Creator of the Exile branch, responsible for a lot
                     of the merged functionality and was a driving force
		     behind innovation in BTMux.
* Kelvin McCorvin  - Current project lead.
* Hagbard          - Current lead programmer. Lots of internal changes
                     and optimizations.
* Daniel MacGregor - Lots of assorted hardcode. New AI system.
* Power_Shaper     - Lots of assorted hardcode.
* Scotty           - Responsible for the writing of a vast amount of
                     documentation.

Contributors
------------
* NeverWhere         - Help files and documentation. 
* Stringfellow Hawke - Template files.
* Fitz               - Misc. code contributions, stagger code.
```

## TinyMUX Credits

Presented from the last BattletechMUX archive's `CREDITS.TinyMUX` file here for preservation:

```
TinyMUX 1.0 is derived from TinyMUSH 2.0.10 patchlevel 6. It is maintained
by David Passmore (Lauren@From the Ashes) and would not exist without
the generous contributions of many individuals. Ideas for features (and
occasionally code) came from many places, including TinyMUSE, and PennMUSH.

We would like to thank the following people:
 
- Dave Peterson (Evinar) who maintained TinyMUSH 2.0 in its later
  incarnations.
 
- TinyMUSH 2.2, and PennMUSH 1.50 for many functions carried over for
  compatibility, and parts of the help text. These are documented as they
  appear in the source code. This code would not be possible without these
  two servers.

- James Callahan (Darkenelf), who contributed many patches, and ideas
  (@teleport/quiet, @readcache fixes, side effect functions,
   many bug fixes)
 
- Chris (Children of the Atom) who had many, many, many ideas and found
  many, many, many bugs, in his own gleeful manner of crashing our site.
 
- Ethaniel and Kayan Telva (BTech3056) for the basic comsystem and macro code.
 
- Dreamline(Horizons) who helped update the help text.

- Stephen J. Kiernan for the (very alpha) port concentrator code.

- Kalkin(DarkZone) and Harlock(StarWarsII) who extended the comsystem and
  added tons of new commands, which is basically unchanged here.
 
- Kalkin, again, for the idea of restarting on a fatal signal.
  
- Airam(Generations) for ideas on the stack and @program code.

- Mike(StarWars), idea for not saving GOING objects.

- Dean Gaudet, for his user and hostname slave process code.

- The GNU project, for their wonderful database manager, and malloc package.

- Andrew Molitor, for the radix compression library, and some wonderful
  utilities.

- Robby Griffin, whose skill in uncovering obscure bugs has saved everyone
  a lot of time and effort.

- Many other people who may go unnamed. Dozens of people have contributed to
  development of the TinyMU* family of servers, and we would like to thank
  them for their hard work.
 
Following is the original credits for TinyMUSH 2.0:
 
TinyMUSH 2.0 is derived from Larry Foard's TinyMUSH (which was itself derived
from TinyMUD, written by Jim Aspnes).  Ideas for features (and occasionally
code) came from many places, including TinyMUSE, PernMUSH, and TinyTIM.
 
We would like to thank the following people:
 
- Jim Aspnes, for the original TinyMUD (from which TinyMUSH was derived)
 
- Larry Foard, for the original implementation of TinyMUSH.
 
- Marcus Ranum for the original Untermud database layer code, and  Andrew
  Molitor for getting it to work with TinyMUSH.
 
- Andrew Molitor (again) for the VMS port.
 
- Russ(Random) and Jennifer(Moira) Smith, for ideas, comments, and coding
  help.
 
- R'nice(TinyTIM) for more good ideas than we could shake a wand of coding
  at.  (@doing, @edit enhancements, a REAL use command, lotsa minor fixes and
  tweaks)
 
- Coyote(TinyTIM, DungeonMUSH, NarniaMUSH), for finding some nasty bugs and
  NOT using them for evil purposes.
 
- Ambar, Amberyl, Sh'dow, Jellan, and Miritha (all from PernMUSH) for numerous
  bug fixes, enhancements, and ideas.
 
- Sketch(TinyTIM) for rewriting some of the more confusing help file entries.
 
- Hcobb(TinyTIM) and Furie(DungeonMUSH) for inspiring the parser rewrite and
  other security-related fixes and enhancements.
 
- The many other people who have contributed ideas, comments, or complaints
  about bugs.
```

## TinyMUX, BattletechMUX, 3030MUX Copyrights

As per the `COPYRIGHT` file in BattletechMUX:

```
# Copyright

TinyMUX 1.0, 1.1 and 1.2 Source code is maintained by David Passmore
(Lauren@Children of the Atom). Comments should be sent to
lauren@ranger.range.orst.edu. TinyMUX is Copyright (c) 1995 by David
Passmore.

Many parts of the source code, and the help text were derived from TinyMUSH
2.2:

TinyMUSH 2.2 Source was written in part by Jean Marie Diaz, Lydia Leong, and
Devin Hooker. 2.2 initial alpha release 9/21/94. Final beta snapshot
(hopefully) 3/4/95.

Many parts of the source code, and the help text were derived from PennMUSH
1.50.

Patchlevel 12 is based on PennMUSH pl10, the last version of PennMUSH released
by Amberyl. All copyright notices above continue to apply. Versions of
PennMUSH prior to pl10 are written in part by Alan Schwartz.

Thanks and praise are due to:

* Ralph Melton (Rhyanna@Castle D'Image) for bugs reports, patches, ideas, and
  extensions.
* T. Alexander Popiel for bug reports, patches, ideas, and extensions, too.
* Al Brown (Kalkin@DarkZone) for many clever ideas.
* The many other people who've sent in bug reports.
* The admin and players of Dune II, who put up with a lot of broken code.
* Special thanks to Amberyl for all her help.

- Paul/Javelin (Alan Schwartz, alansz@mellers1.psych.berkeley.edu)

Based on TinyMUSH 2.0 Source code Copyright (c) 1991 Joseph Traub and Glenn
Crocker.

Based on TinyMUD code Copyright (c) 1989, 1990 by David Applegate, James
Aspnes, Timothy Freeman, and Bennet Yee.

This material was developed by the above-mentioned authors. Permission to copy
this software, to redistribute it, and to use it for any purpose is granted,
subject to the following restrictions and understandings.

1. Any copy made of this software must include this copyright notice in full.

2. Users of this software agree to make their best efforts:

   (a) to return to the above-mentioned authors any improvements or extensions
   that they make, so that these may be included in future releases; and

   (b) to inform the authors of noteworthy uses of this software.

3. All materials developed as a consequence of the use of this software shall
   duly acknowledge such use, in accordance with the usual standards of
   acknowledging credit in academic research.

4. The authors have made no warrantee or representation that the operation of
   this software will be error-free, and the authors are under no obligation to
   provide any services, by way of maintenance, update, or otherwise.

5. In conjunction with products arising from the use of this material, there
   shall be no use of the names of the authors, of Carnegie-Mellon University,
   nor of any adaptation thereof in any advertising, promotional, or sales
   literature without prior written consent from the authors and both
   Carnegie-Mellon University Case Wester Reserve University in each case.

## Credits

Lawrence Foard:
Wrote the original TinyMUSH 1.0 code from which this later derived.

Jin (and MicroMUSH):
Made many, many changes to the code that improved it immensely.

Lachesis:
Introduced the idea of property lists to TinyMUCK.

Many others:
Many features borrowed from other muds.

## Additional Feature Copyrights

All of the following defined features are Copyright (c) 2004 by Kevin Stevens
(AKA Null, kevin@thinairit.com) and released under the same license agreement
as the rest of code in the `/src` directory:

* ARBITRARY_LOGFILES_MODE changes to use `2` as settings.
* BT_ADVANCED_ECON
* BT_FREETECHTIME
* BT_COMPLEXREPAIRS
* BT_MECHDEST_TRIGGER

Under the `/hcode`, `/hcode/btech`, and `hcode/btech/include` tree all code
pertaining to Kevin Stevens copyright in any headers is related to the
following definitions and under the same license as the rest of the code in
those trees:

* BT_ADVANCED_ECON
* BT_ADVANCED_ECON_INIT
* EXILE_FUNCS_SUPPORT
* BT_CALCULATE_BV
* BT_MECHDEST_TRIGGER
* BT_EXILE_SKILLS
* BT_EXILE_MW3STATS
* BT_FREETECHTIME
* BT_MOVEMENT_MODES
* BT_CARRIERS
* BT_COMPLEXREPAIRS
```