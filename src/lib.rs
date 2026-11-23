//! Original file decoding and game logic, independent of host I/O.
pub mod archive;
pub mod auction_restore;
pub mod board;
pub mod board_style;
pub mod build_file;
pub mod clock;
pub mod era;
pub mod category_reset;
pub mod economy;
pub mod business_pool;
pub mod business_report;
pub mod geometry;
pub mod hourly;
pub mod housing;
pub mod lighting;
pub mod pulse_lights;
pub mod material;
pub mod market;
pub mod mesh;
pub mod panels;
pub mod pe;
pub mod picking;
pub mod people;
pub mod polygon;
#[cfg(feature = "graphics")]
pub mod render;
pub mod resident;
pub mod roof;
pub mod routing;
pub mod save;
pub mod scene;
pub mod scheduler;
pub mod pause;
pub mod timers;
pub mod traffic;
pub mod x87;
pub mod script_events;
pub mod sky;
pub mod texture;
pub mod trees;

pub mod ai_ownership;
pub mod ai_business;
pub mod ai_candidate_fill;
pub mod ai_business_choice;
pub mod ai_business_ranking;
pub mod ai_developed_targets;
pub mod ai_neighbors;
pub mod ai_maintenance;
pub mod ai_candidate_queries;
pub mod ai_candidate_scores;
pub mod ai_dispatch;
pub mod ai_tick;
pub mod ai_events;
pub mod ai_parameters;
pub mod ai_profiles;
pub mod consumer_poll;
pub mod ai_valuation_queries;
pub mod player_manager;
pub mod player_metrics;
pub mod player_history;
pub mod player_tick;
pub mod startup_rng;
pub mod scenario_objective;
pub mod objective_tick;
pub mod player_constructor;
#[cfg(feature = "lua4")]
pub mod lua4;
#[cfg(feature = "lua4")]
pub mod ai_script;
#[cfg(feature = "lua4")]
pub mod player_script;
#[cfg(feature = "lua4")]
pub mod population_script;
#[cfg(feature = "lua4")]
pub mod initial_conditions;
#[cfg(feature = "lua4")]
pub mod commodity_script;
#[cfg(feature = "lua4")]
pub mod business_script;
pub mod ai_rent;
pub mod atlas;
pub mod camera;
pub mod category;
pub mod category_lease;
pub mod property_offer;
pub mod property_rights;
pub mod outcome_notice;
pub mod water;

pub mod shopping;
pub mod flyby;
pub mod scene_assets;

#[cfg(feature = "graphics")]
pub mod render_scene;
pub mod ui_mesh;
pub mod font;
pub mod ui_button;
pub mod building_binding;
pub mod business_binding;
pub mod business_initialize;
pub mod building_pool;

#[cfg(feature = "lua4")]
pub mod map_script;

pub mod building_geometry;

#[cfg(feature = "lua4")]
pub mod level_script;

#[cfg(feature = "lua4")]
pub mod resident_settings;

#[cfg(feature = "lua4")]
pub mod startup_settings;

#[cfg(feature = "lua4")]
pub mod level_startup;

#[cfg(feature = "lua4")]
pub mod level_runtime;
pub mod pending_world;
pub mod category_ownership;

pub mod business_detach;
pub mod business_open;
pub mod business_sale;
pub mod ai_takeover;
pub mod ai_daily;
pub mod construction;
pub mod demolition;
pub mod demolition_effect;
pub mod local_commands;
#[cfg(feature = "lua4")]
pub mod live_commands;
#[cfg(test)]
mod demolition_tests;

pub mod player_share;
pub mod ai_hourly;

pub mod player_elimination;
#[cfg(test)]
mod player_elimination_live_test;

pub mod player_messages;
pub mod event_cards;
pub mod ai_messages;
pub mod ai_finance;
pub mod ai_runtime;
pub mod ai_space;
pub mod ai_guards;
pub mod ai_strategy;
pub mod ai_strategy_queries;
pub mod ai_scoring;
pub mod ai_selection;
pub mod ai_targets;
pub mod ai_actions;

pub mod native_sort;
pub mod ai_demand;

pub mod building_styles;

pub mod ai_acquisition;
pub mod business_takeover;
pub mod business_conversion;
pub mod human_reply;
pub mod human_messages;
pub mod business_controls;
pub mod business_permissions;
pub mod level_catalog;
pub mod skirmish;
