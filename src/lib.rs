//! Original file decoding and game logic, independent of host I/O.
pub mod archive;
pub mod auction_controls;
pub mod auction_restore;
pub mod board;
pub mod board_style;
pub mod build_file;
pub mod building_models;
pub mod business_pool;
pub mod business_report;
#[cfg(feature = "lua4")]
pub mod campaign_store;
pub mod category_reset;
pub mod clock;
pub mod compatibility;
pub mod economy;
pub mod era;
/// Test support: locates private original files for original-backed tests.
#[doc(hidden)]
pub mod fixtures;
pub mod geometry;
pub mod hourly;
pub mod housing;
pub mod lighting;
pub mod market;
pub mod material;
pub mod mesh;
pub mod network;
pub mod opening_sequence;
pub mod panels;
pub mod pause;
pub mod pe;
pub mod people;
pub mod picking;
pub mod player_startup;
pub mod polygon;
pub mod prompt_format;
pub mod pulse_lights;
#[cfg(feature = "graphics")]
pub mod render;
pub mod resident;
pub mod result_controller;
pub mod roof;
pub mod routing;
pub mod save;
#[cfg(feature = "native")]
pub mod save_menu;
pub mod scene;
pub mod scheduler;
pub mod script_events;
pub mod skirmish_maps;
pub mod sky;
pub mod street_furniture;
pub mod texture;
pub mod timers;
pub mod traffic;
pub mod trees;
pub mod x87;

pub mod ai_business;
pub mod ai_business_choice;
pub mod ai_business_ranking;
pub mod ai_candidate_fill;
pub mod ai_candidate_queries;
pub mod ai_candidate_scores;
pub mod ai_developed_targets;
pub mod ai_diagnostic;
pub mod ai_dispatch;
pub mod ai_events;
pub mod ai_maintenance;
pub mod ai_neighbors;
pub mod ai_ownership;
pub mod ai_parameters;
pub mod ai_profiles;
pub mod ai_rent;
#[cfg(feature = "lua4")]
pub mod ai_script;
pub mod ai_tick;
pub mod ai_valuation_queries;
pub mod atlas;
#[cfg(feature = "lua4")]
pub mod business_script;
pub mod camera;
pub mod category;
pub mod category_lease;
#[cfg(feature = "lua4")]
pub mod commodity_script;
pub mod consumer_poll;
#[cfg(feature = "lua4")]
pub mod initial_conditions;
#[cfg(feature = "lua4")]
pub mod lua4;
pub mod objective_tick;
pub mod outcome_notice;
pub mod player_constructor;
pub mod player_history;
pub mod player_manager;
pub mod player_metrics;
#[cfg(feature = "lua4")]
pub mod player_script;
pub mod player_tick;
#[cfg(feature = "lua4")]
pub mod population_script;
pub mod process_dispatch;
pub mod process_image;
pub mod process_runtime;
pub mod property_offer;
pub mod property_rights;
pub mod scenario_objective;
pub mod singleplayer_setup;
pub mod startup_rng;
pub mod water;

pub mod flyby;
#[cfg(feature = "lua4")]
pub mod flyby_script;
pub mod scene_assets;
pub mod shopping;

pub mod building_binding;
pub mod building_parameters;
pub mod building_pool;
pub mod business_binding;
pub mod business_initialize;
pub mod font;
#[cfg(feature = "graphics")]
pub mod render_scene;
pub mod ui_button;
pub mod ui_mesh;

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

pub mod category_ownership;
#[cfg(feature = "lua4")]
pub mod level_runtime;
pub mod pending_world;

pub mod ai_daily;
pub mod ai_takeover;
pub mod business_detach;
pub mod business_open;
pub mod business_sale;
pub mod construction;
pub mod demolition;
pub mod demolition_effect;
#[cfg(test)]
mod demolition_tests;
#[cfg(feature = "lua4")]
pub mod live_commands;
pub mod local_commands;

pub mod ai_hourly;
pub mod player_share;

pub mod player_elimination;
#[cfg(test)]
mod player_elimination_live_test;

pub mod ai_actions;
pub mod ai_finance;
pub mod ai_guards;
pub mod ai_messages;
pub mod ai_runtime;
pub mod ai_scoring;
pub mod ai_selection;
pub mod ai_space;
pub mod ai_strategy;
pub mod ai_strategy_queries;
pub mod ai_targets;
pub mod event_cards;
pub mod player_messages;

pub mod ai_demand;
pub mod native_sort;

pub mod building_styles;

pub mod ai_acquisition;
pub mod business_controls;
pub mod business_conversion;
pub mod business_permissions;
pub mod business_takeover;
pub mod frontend_list;
pub mod human_messages;
pub mod human_reply;
pub mod level_catalog;
pub mod skirmish;

pub mod native_input;

pub mod window_texture;

#[cfg(test)]
mod account_posting_test_support;
