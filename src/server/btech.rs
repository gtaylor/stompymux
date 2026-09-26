//! One-second BattleTech updates share the ordinary world commit and notification boundary.
use super::*;

impl Server {
    /// Apply one simulation step; failed saves restore the countdown and discard its notices.
    pub(super) async fn btech_tick(&mut self, now: i64) {
        self.btech_tick_measured(now, None).await;
    }

    /// The production step with optional diagnostic timings; scheduling semantics are identical.
    pub(super) async fn btech_tick_measured(
        &mut self,
        now: i64,
        mut metrics: Option<&mut super::heartbeat_harness::HeartbeatMetrics>,
    ) {
        self.scripts.record_battle_event_tick();
        let mut scanner_observers = crate::optical_scanner_observers(&self.scripts.world.borrow());
        let starting_scanners: std::collections::BTreeSet<_> = {
            let world = self.scripts.world.borrow();
            world
                .btech
                .constructed_units()
                .iter()
                .filter_map(|(&id, unit)| {
                    matches!(unit.power(), crate::BattlePower::Starting { .. }).then_some(id)
                })
                .chain(world.btech.vehicles().iter().filter_map(|(&id, unit)| {
                    matches!(unit.power(), crate::BattlePower::Starting { .. }).then_some(id)
                }))
                .collect()
        };
        let active = crate::btech::simulation_pending::pending(
            &self.scripts.world.borrow(),
            &self.config,
            !scanner_observers.is_empty(),
        );
        let before = self.scripts.world.borrow().clone();
        let simulation_time = {
            let mut world = self.scripts.world.borrow_mut();
            world.btech.turn_clock.advance();
            world.btech.simulation_seconds = world.btech.simulation_seconds.saturating_add(1);
            world.btech.simulation_time()
        };
        if !active {
            if self.commit_heartbeat(before, metrics.as_deref_mut()).await {
                self.flush();
            }
            return;
        }
        if let Err(error) = crate::advance_battle_wrecks_action(&self.scripts, &self.config) {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "ERROR",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        crate::advance_battle_reactor_windows(&mut self.scripts.world.borrow_mut());
        crate::advance_building_repairs(&mut self.scripts.world.borrow_mut());
        crate::advance_map_smoke(&mut self.scripts.world.borrow_mut());
        let fire_result = crate::advance_map_fire(&mut self.scripts.world.borrow_mut());
        if let Err(error) = fire_result {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "ERROR",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        if let Err(error) = crate::advance_artillery_action(
            &self.scripts,
            &self.config,
            crate::BattleFallRules {
                vehicle_impact: crate::BattleVehicleImpactRules::configured(
                    &self.config.battletech,
                    false,
                ),
                stacking: crate::BattleStackingRules {
                    mode: self.config.battletech.stacking,
                    damage_percent: self.config.battletech.stackdamage,
                    hit_arcs: self.config.battletech.hit_arcs,
                },
                stagger: crate::BattleStaggerMode::from_setting(self.config.battletech.newstagger),
                hit: crate::BattleHitRules {
                    inferno_penalty: self.config.battletech.inferno_penalty != 0,
                    exile_stun_mode: self.config.battletech.exile_stun_code.clamp(0, 2) as u8,
                },
                extended_piloting: self.config.battletech.extended_piloting != 0,
                toughness: false,
            },
        ) {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "ARTILLERY",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        let mut building_arrivals =
            match crate::advance_battle_building_entries_action(&self.scripts) {
                Ok(arrivals) => arrivals,
                Err(error) => {
                    self.config.log(
                        &[crate::logging::Category::Problems],
                        "BTECH",
                        "BUILDING",
                        error.to_string(),
                    );
                    *self.scripts.world.borrow_mut() = before;
                    std::sync::Arc::make_mut(
                        &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                    )
                    .clear();
                    self.scripts.effects.rollback();
                    return;
                }
            };
        if !building_arrivals.is_empty() {
            // New map membership changes eligible pairs; startup completion below still waits a tick.
            scanner_observers.extend(crate::optical_scanner_observers(
                &self.scripts.world.borrow(),
            ));
        }
        if let Err(error) = crate::advance_battle_sixth_sense_action(&self.scripts) {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "ERROR",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        if let Err(error) =
            crate::advance_battle_periodic_piloting_action(&self.scripts, &self.config)
        {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "PILOTING",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        crate::clear_battle_recent_fire(&mut self.scripts.world.borrow_mut());
        let mut notices = crate::advance_battle_units(&mut self.scripts.world.borrow_mut(), now);
        notices.extend(crate::advance_battle_standing(
            &mut self.scripts.world.borrow_mut(),
        ));
        let autopilot_result = match metrics.as_deref_mut() {
            Some(metrics) => crate::btech::autopilot::runtime::advance_with_metrics(
                &mut self.scripts.world.borrow_mut(),
                &self.config,
                simulation_time,
                &mut metrics.autopilot,
            ),
            None => crate::btech::autopilot::runtime::advance(
                &mut self.scripts.world.borrow_mut(),
                &self.config,
                simulation_time,
            ),
        };
        match autopilot_result {
            Ok(autopilot_notices) => notices.extend(autopilot_notices),
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "AUTOPILOT",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        match crate::advance_battle_motion_action(
            &self.scripts,
            &self.config,
            crate::BattleMovementRules {
                free_fusion_vtol_fuel: self.config.battletech.nofusionvtolfuel != 0,
                tsm_tow_bonus: self.config.battletech.tsm_tow_bonus != 0,
                physical_pilot_skill: self.config.battletech.phys_use_pskill != 0,
                new_terrain: self.config.battletech.newterrain != 0,
                charge: crate::BattleChargePolicy {
                    new_rules: self.config.battletech.newcharge != 0,
                    technology_level_three: self.config.battletech.tl3_charge != 0,
                    extended_movement: self.config.battletech.extendedmovemod != 0,
                    hit_arc_mode: self.config.battletech.hit_arcs,
                },
                fasa_turning: self.config.battletech.fasaturn != 0,
                slowdown: self.config.battletech.slowdown,
                roll_on_backwalk: self.config.battletech.roll_on_backwalk != 0,
                skid_cliff: self.config.battletech.skidcliff != 0,
                fall: crate::BattleFallRules {
                    vehicle_impact: crate::BattleVehicleImpactRules::configured(
                        &self.config.battletech,
                        false,
                    ),
                    stacking: crate::BattleStackingRules {
                        mode: self.config.battletech.stacking,
                        damage_percent: self.config.battletech.stackdamage,
                        hit_arcs: self.config.battletech.hit_arcs,
                    },
                    stagger: crate::BattleStaggerMode::from_setting(
                        self.config.battletech.newstagger,
                    ),
                    hit: crate::BattleHitRules {
                        inferno_penalty: self.config.battletech.inferno_penalty != 0,
                        exile_stun_mode: self.config.battletech.exile_stun_code.clamp(0, 2) as u8,
                    },
                    extended_piloting: self.config.battletech.extended_piloting != 0,
                    toughness: false,
                },
            },
        ) {
            Ok(arrivals) => {
                if !arrivals.is_empty() {
                    scanner_observers.extend(
                        crate::optical_scanner_observers(&self.scripts.world.borrow())
                            .into_iter()
                            .filter(|id| !starting_scanners.contains(id)),
                    );
                    building_arrivals.extend(arrivals);
                }
            }
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        notices.extend(crate::advance_battle_null_signature(
            &mut self.scripts.world.borrow_mut(),
        ));
        notices.extend(crate::advance_battle_stealth(
            &mut self.scripts.world.borrow_mut(),
        ));
        notices.extend(crate::advance_battle_searchlights(
            &mut self.scripts.world.borrow_mut(),
        ));
        notices.extend(crate::advance_battle_sensor_selection(
            &mut self.scripts.world.borrow_mut(),
        ));
        notices.extend(crate::advance_battle_recycle(
            &mut self.scripts.world.borrow_mut(),
        ));
        if let Err(error) = crate::advance_battle_boosters_action(&self.scripts, &self.config) {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "ERROR",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        let dump_result = crate::advance_battle_dumping(&mut self.scripts.world.borrow_mut());
        match dump_result {
            Ok(dumping) => notices.extend(dumping),
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        let unjam_result = crate::btech::unjam::advance_unjamming_in_action(
            &mut self.scripts.world.borrow_mut(),
            self.config.battletech.extended_piloting != 0,
            self.config.battletech.extended_gunnery != 0,
        );
        let unjam_report = match unjam_result {
            Ok(messages) => messages,
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        };
        notices.extend(crate::advance_battle_heat(
            &mut self.scripts.world.borrow_mut(),
        ));
        let vehicle_fires = crate::advance_battle_vehicle_fires_action(&self.scripts, &self.config);
        match vehicle_fires {
            Ok(_) => (),
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        notices.extend(crate::advance_inferno_burns(
            &mut self.scripts.world.borrow_mut(),
        ));
        match crate::advance_battle_jumps_action(
            &self.scripts,
            &self.config,
            crate::BattleMovementRules {
                free_fusion_vtol_fuel: self.config.battletech.nofusionvtolfuel != 0,
                tsm_tow_bonus: self.config.battletech.tsm_tow_bonus != 0,
                physical_pilot_skill: self.config.battletech.phys_use_pskill != 0,
                new_terrain: self.config.battletech.newterrain != 0,
                fall: crate::BattleFallRules {
                    vehicle_impact: crate::BattleVehicleImpactRules::configured(
                        &self.config.battletech,
                        false,
                    ),
                    stacking: crate::BattleStackingRules {
                        mode: self.config.battletech.stacking,
                        damage_percent: self.config.battletech.stackdamage,
                        hit_arcs: self.config.battletech.hit_arcs,
                    },
                    stagger: crate::BattleStaggerMode::from_setting(
                        self.config.battletech.newstagger,
                    ),
                    hit: crate::BattleHitRules {
                        inferno_penalty: self.config.battletech.inferno_penalty != 0,
                        exile_stun_mode: self.config.battletech.exile_stun_code.clamp(0, 2) as u8,
                    },
                    extended_piloting: self.config.battletech.extended_piloting != 0,
                    toughness: false,
                },
                charge: crate::BattleChargePolicy {
                    new_rules: self.config.battletech.newcharge != 0,
                    technology_level_three: self.config.battletech.tl3_charge != 0,
                    extended_movement: self.config.battletech.extendedmovemod != 0,
                    hit_arc_mode: self.config.battletech.hit_arcs,
                },
                fasa_turning: self.config.battletech.fasaturn != 0,
                ..crate::BattleMovementRules::STANDARD
            },
        ) {
            Ok(arrivals) => {
                if !arrivals.is_empty() {
                    scanner_observers.extend(
                        crate::optical_scanner_observers(&self.scripts.world.borrow())
                            .into_iter()
                            .filter(|id| !starting_scanners.contains(id)),
                    );
                    building_arrivals.extend(arrivals);
                }
            }
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        notices.extend(crate::advance_battle_stun(
            &mut self.scripts.world.borrow_mut(),
        ));
        let recovery_notices = crate::advance_battle_recovery(&mut self.scripts.world.borrow_mut());
        // Stage earlier cockpit and unjam feedback before thermal consequences; output remains uncommitted.
        for (target, text) in notices
            .drain(..)
            .map(|notice| (crate::BattleMessageTarget::Unit(notice.unit), notice.text))
        {
            if let Err(error) = crate::btech::notify_message(&self.scripts, target, &text) {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        if let Err(error) =
            crate::btech::unjam::publish_unjamming(&self.scripts, &self.config, &unjam_report)
        {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "ERROR",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        let settings = &self.config.battletech;
        let thermal = crate::advance_battle_overheat_action(
            &self.scripts,
            &self.config,
            crate::BattleOverheatRules {
                vehicle_impact: crate::BattleVehicleImpactRules::configured(settings, false),
                stacking: crate::BattleStackingRules {
                    mode: settings.stacking,
                    damage_percent: settings.stackdamage,
                    hit_arcs: settings.hit_arcs,
                },
                hit: crate::BattleHitRules {
                    inferno_penalty: settings.inferno_penalty != 0,
                    exile_stun_mode: settings.exile_stun_code.clamp(0, 2) as u8,
                },
                extended_piloting: settings.extended_piloting != 0,
                stagger: crate::BattleStaggerMode::from_setting(settings.newstagger),
            },
        );
        match thermal {
            Ok(_) => {}
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        };
        let stagger = crate::advance_battle_stagger_action(
            &self.scripts,
            &self.config,
            crate::BattleStaggerRules {
                vehicle_impact: crate::BattleVehicleImpactRules::configured(settings, false),
                mode: crate::BattleStaggerMode::from_setting(settings.newstagger),
                interval: settings.newstaggertime.max(1) as u64,
                tonnage: settings.newstaggertons != 0,
                hit: crate::BattleHitRules {
                    inferno_penalty: settings.inferno_penalty != 0,
                    exile_stun_mode: settings.exile_stun_code.clamp(0, 2) as u8,
                },
                extended_piloting: settings.extended_piloting != 0,
            },
        );
        match stagger {
            Ok(_) => {}
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        if let Err(error) =
            crate::advance_battle_computer_failures_action(&self.scripts, &self.config)
        {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "COMPUTER",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        crate::advance_battle_sensor_signals(&mut self.scripts.world.borrow_mut());
        let electronic_changes =
            crate::refresh_battle_electronic_fields(&mut self.scripts.world.borrow_mut());
        match electronic_changes {
            Ok(changes) => notices.extend(changes),
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        let links = crate::advance_battle_spotter_links(&mut self.scripts.world.borrow_mut());
        match links {
            Ok(link_notices) => notices.extend(link_notices),
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        notices.extend(crate::advance_battle_tags(
            &mut self.scripts.world.borrow_mut(),
        ));
        notices.extend(crate::refresh_battle_illumination(
            &mut self.scripts.world.borrow_mut(),
        ));
        notices.extend(crate::advance_battle_sensor_flashes(
            &mut self.scripts.world.borrow_mut(),
        ));
        if let Err(error) = crate::advance_battle_self_destructs_action(&self.scripts, &self.config)
        {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "SELFDESTRUCT",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        crate::advance_battle_automatic_turrets(&mut self.scripts.world.borrow_mut());
        let contact_events = crate::refresh_optical_scanners(
            &mut self.scripts.world.borrow_mut(),
            &scanner_observers,
        );
        let contact_events = match contact_events {
            Ok(events) => events,
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        };
        match crate::advance_battle_hiding(&mut self.scripts.world.borrow_mut()) {
            Ok(updates) => notices.extend(updates),
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "HIDE",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        notices.extend(crate::advance_battle_target_locks(
            &mut self.scripts.world.borrow_mut(),
        ));
        let autopilot_result = match metrics.as_deref_mut() {
            Some(metrics) => crate::btech::autopilot::runtime::advance_combat_with_metrics(
                &mut self.scripts.world.borrow_mut(),
                &self.config,
                simulation_time,
                &mut metrics.autopilot,
            ),
            None => crate::btech::autopilot::runtime::advance_combat(
                &mut self.scripts.world.borrow_mut(),
                &self.config,
                simulation_time,
            ),
        };
        match autopilot_result {
            Ok(autopilot_notices) => notices.extend(autopilot_notices),
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "AUTOPILOT",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        for event in contact_events {
            if let Err(error) = crate::btech::notify_contact(&self.scripts, &self.config, event) {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        if let Err(error) =
            crate::publish_battle_building_arrivals(&self.scripts, building_arrivals)
        {
            self.config.log(
                &[crate::logging::Category::Problems],
                "BTECH",
                "BUILDING",
                error.to_string(),
            );
            *self.scripts.world.borrow_mut() = before;
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
            self.scripts.effects.rollback();
            return;
        }
        for notice in recovery_notices {
            if let Err(error) = crate::btech::notify_character(&self.scripts, notice) {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        let messages = notices
            .into_iter()
            .map(|notice| (crate::BattleMessageTarget::Unit(notice.unit), notice.text));
        for (unit, text) in messages {
            if let Err(error) = crate::btech::notify_message(&self.scripts, unit, &text) {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "BTECH",
                    "ERROR",
                    error.to_string(),
                );
                *self.scripts.world.borrow_mut() = before;
                std::sync::Arc::make_mut(
                    &mut self.scripts.world.borrow_mut().btech.autopilot_plans,
                )
                .clear();
                self.scripts.effects.rollback();
                return;
            }
        }
        if self.commit_heartbeat(before, metrics.as_deref_mut()).await {
            self.flush();
        }
    }
    /// Measure the actual validated database commit, with no benchmark substitute.
    async fn commit_heartbeat(
        &mut self,
        before: World,
        metrics: Option<&mut super::heartbeat_harness::HeartbeatMetrics>,
    ) -> bool {
        let started = metrics.as_ref().map(|_| Instant::now());
        let committed = self.commit(before).await;
        if !committed {
            std::sync::Arc::make_mut(&mut self.scripts.world.borrow_mut().btech.autopilot_plans)
                .clear();
        }
        if let (Some(metrics), Some(started)) = (metrics, started) {
            metrics.persistence = started.elapsed();
            metrics.committed = committed;
        }
        committed
    }
}
