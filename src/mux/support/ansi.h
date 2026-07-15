/* ansi.h */

/* ANSI control codes for various neat-o terminal effects
 *
 * Some older versions of Ultrix don't appear to be able to
 * handle these escape sequences. If lowercase 'a's are being
 * stripped from output using the ANSI flag
 * is screwed up, you have the Ultrix problem.
 *
 * To fix the ANSI problem, try replacing the '\x1B' with '\033'.
 * To fix the problem with 'a's, replace all occurrences of '\a'
 * in the code with '\07'.
 *
 */

#pragma once
constexpr char BEEP_CHAR = '\07';
constexpr char ESC_CHAR = '\033';

constexpr char ANSI_NORMAL[] = "\033[0m";

constexpr char ANSI_HILITE[] = "\033[1m";
constexpr char ANSI_INVERSE[] = "\033[7m";
constexpr char ANSI_BLINK[] = "\033[5m";
constexpr char ANSI_UNDER[] = "\033[4m";

constexpr char ANSI_INV_BLINK[] = "\033[7;5m";
constexpr char ANSI_INV_HILITE[] = "\033[1;7m";
constexpr char ANSI_BLINK_HILITE[] = "\033[1;5m";
constexpr char ANSI_INV_BLINK_HILITE[] = "\033[1;5;7m";

/* Foreground colors */
constexpr char ANSI_BLACK[] = "\033[30m";
constexpr char ANSI_RED[] = "\033[31m";
constexpr char ANSI_GREEN[] = "\033[32m";
constexpr char ANSI_YELLOW[] = "\033[33m";
constexpr char ANSI_BLUE[] = "\033[34m";
constexpr char ANSI_MAGENTA[] = "\033[35m";
constexpr char ANSI_CYAN[] = "\033[36m";
constexpr char ANSI_WHITE[] = "\033[37m";

/* Background colors */
constexpr char ANSI_BBLACK[] = "\033[40m";
constexpr char ANSI_BRED[] = "\033[41m";
constexpr char ANSI_BGREEN[] = "\033[42m";
constexpr char ANSI_BYELLOW[] = "\033[43m";
constexpr char ANSI_BBLUE[] = "\033[44m";
constexpr char ANSI_BMAGENTA[] = "\033[45m";
constexpr char ANSI_BCYAN[] = "\033[46m";
constexpr char ANSI_BWHITE[] = "\033[47m";
