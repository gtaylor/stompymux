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
| reference-adversarial | true |
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
| pursuit | true |
| pursuit_extended | false |
| cpu | true |
| moving | false |

| Workload | Fire | Autopilot p50 before / after | Autopilot p95 before / after | Heartbeat p95 before / after |
| --- | --- | --- | --- | --- |
| moving_congestion | false | 31.132 / 30.919 | 34.836 / 36.213 | 194.329 / 197.821 |
| moving_congestion | true | 40.232 / 39.696 | 63.525 / 59.773 | 245.786 / 253.045 |
| obstacles | false | 35.161 / 34.818 | 40.911 / 40.895 | 201.826 / 207.321 |
| obstacles | true | 46.221 / 46.162 | 70.936 / 73.389 | 263.885 / 269.676 |
| open | false | 31.465 / 29.926 | 37.391 / 35.063 | 200.734 / 195.614 |
| open | true | 37.881 / 37.731 | 73.717 / 73.157 | 255.462 / 257.957 |
| moving_pursuit | false | 29.006 / 30.928 | 33.317 / 36.995 | 216.212 / 213.125 |
| moving_pursuit | true | 33.511 / 36.705 | 64.453 / 68.806 | 236.624 / 242.52 |

Reference trace differences are retained separately and require review for intended behavior changes. Raw artifacts and command logs accompany this report.
