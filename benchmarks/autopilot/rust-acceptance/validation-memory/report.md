# Autopilot acceptance

Passed: false. Performance evaluated: true.

| Gate | Passed |
| --- | --- |
| behavior-existing | true |
| replay-existing | true |
| replay-summary-existing | true |
| reference-existing | true |
| behavior-adversarial | true |
| replay-adversarial | true |
| replay-summary-adversarial | true |
| reference-adversarial | false |
| behavior-late_clearance | true |
| replay-late_clearance | true |
| replay-summary-late_clearance | true |
| reference-late_clearance | true |
| behavior-alternating_clearance | true |
| replay-alternating_clearance | true |
| replay-summary-alternating_clearance | true |
| reference-alternating_clearance | true |
| behavior-waiting_controllers | true |
| replay-waiting_controllers | true |
| replay-summary-waiting_controllers | true |
| reference-waiting_controllers | true |
| replay-lateral | true |
| replay-summary-lateral | true |
| reference-lateral | false |
| replay-distant | true |
| replay-summary-distant | true |
| reference-distant | false |
| replay-retreat | true |
| replay-summary-retreat | true |
| reference-retreat | false |
| replay-reversals | true |
| replay-summary-reversals | true |
| reference-reversals | false |
| replay-circuit | true |
| replay-summary-circuit | true |
| reference-circuit | false |
| replay-short_occlusions | true |
| replay-summary-short_occlusions | true |
| reference-short_occlusions | false |
| replay-expiry | true |
| replay-summary-expiry | true |
| reference-expiry | false |
| replay-intercept_move | true |
| replay-summary-intercept_move | true |
| reference-intercept_move | false |
| replay-slow_crossing | true |
| replay-summary-slow_crossing | true |
| reference-slow_crossing | false |
| replay-fast_crossing | true |
| replay-summary-fast_crossing | true |
| reference-fast_crossing | false |
| replay-opposite_crossing | true |
| replay-summary-opposite_crossing | true |
| reference-opposite_crossing | false |
| replay-stop_start | true |
| replay-summary-stop_start | true |
| reference-stop_start | false |
| replay-gradual_turns | true |
| replay-summary-gradual_turns | true |
| reference-gradual_turns | false |
| replay-damaged_pursuit | true |
| replay-summary-damaged_pursuit | true |
| reference-damaged_pursuit | false |
| pursuit | false |
| pursuit_extended | true |
| cpu | true |
| moving | true |

| Workload | Fire | Autopilot p50 before / after | Autopilot p95 before / after | Heartbeat p95 before / after |
| --- | --- | --- | --- | --- |
| moving_congestion | false | 25.494 / 24.966 | 27.09 / 26.752 | 153.194 / 151.701 |
| moving_congestion | true | 31.936 / 31.355 | 47.704 / 47.184 | 202.495 / 199.363 |
| obstacles | false | 29.181 / 28.031 | 32.635 / 31.604 | 165.123 / 164.896 |
| obstacles | true | 37.952 / 36.976 | 54.792 / 53.007 | 220.33 / 200.155 |
| open | false | 25.118 / 24.691 | 27.349 / 26.987 | 152.448 / 152.665 |
| open | true | 31.073 / 30.224 | 53.732 / 51.361 | 202.841 / 195.203 |
| moving_pursuit | false | 23.317 / 23.677 | 26.672 / 26.014 | 181.147 / 168.9 |
| moving_pursuit | true | 27.378 / 27.188 | 48.12 / 46.572 | 193.836 / 186.61 |

Reference trace differences are retained separately and require review for intended behavior changes. Raw artifacts and command logs accompany this report.
