use super::*;
#[test]
#[ignore = "requires original game assets; owned Options save and cold Clock continuation"]
fn options_save_cold_load_retires_saved_pause_owners() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use crate::fresh_city::{CommandAssets, Inputs, Policy};
            let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
            let exe = read("OPENMT2001_EXE");
            let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let build = read("OPENMT2001_BUILD_FILE");
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let index =
                openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let default = openmt2001::fixtures::path("DEFAULT_SETTINGS");
            let scripts = default.parent().unwrap().parent().unwrap();
            let styles = crate::fresh_city::load_era_styles(
                std::path::Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                    .parent()
                    .unwrap(),
            )
            .unwrap();
            let names = std::array::from_fn(|_| Vec::new());
            let inputs = || Inputs {
                tree_locators: None,
                image: &image,
                board: &board,
                file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                    &image, &index, &archive,
                )
                .unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(
                    &image, &index, &archive,
                )
                .unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
                scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: styles.clone(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                scenario: "SCENARIO3",
                difficulty: 0,
                selected_character: 4,
                profile_name: b"Save fixture",
                character_names: &names,
            };
            let policy = || Policy {
                seed: 0x12345678,
                auxiliary_prior_bytes: [0; 284],
            };
            let mut session = Session::initialize(inputs(), policy()).unwrap();

            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let camera = openmt2001::flyby::CameraSample {
                eye: [100., 120., 100.],
                target: [0., 0., 0.],
                up: [0., 1., 0.],
                vertical_fov: 0.7,
            };
            let cameras = crate::save_menu::cameras(None, camera).unwrap();
            // These real Session saves exercise serialization/restore and a fresh
            // Host's actual registered logic/Clock. Camera/UI admission stays a
            // separate native containing regression, not a synthetic capture.
            for other in [None, Some(0usize), Some(1usize), Some(3usize)] {
                if let Some(reason) = other {
                    session
                        .world
                        .pause
                        .pause(reason, |c, _| {
                            c.apply(&mut session.hooks.timers);
                            Ok(())
                        })
                        .unwrap();
                }
                let expected_pause = session.world.pause.clone();
                let expected_timers = session.hooks.timers.clone();
                let mut host = crate::live_host::Host::new(camera.eye);
                host.set_running(true);
                host.options(&mut session.world.pause, &mut session.hooks.timers, true)
                    .unwrap();
                assert!(host.options_save_allowed(&session.world.pause, &session.hooks.timers));
                let live_pause = session.world.pause.clone();
                let live_timers = session.hooks.timers.clone();
                let bytes = session
                    .save_v2_with_options_view(&image, cameras, 1, &host, 0)
                    .unwrap();
                assert_eq!(session.world.pause, live_pause);
                assert_eq!(session.hooks.timers, live_timers);
                assert!(!host.running);
                assert!(host.options_save_allowed(&session.world.pause, &session.hooks.timers));
                let save = SaveFile::parse(&bytes).unwrap();
                let mut cold = Session::load_original(inputs(), policy(), &bytes).unwrap();
                crate::process_dispatch::native_load_completed(&mut cold).unwrap();
                let load_pause = cold.world.pause.clone();
                let load_timers = cold.hooks.timers.clone();
                cold.restore_v2_extension_with_image(&save, &image).unwrap();
                assert_eq!(cold.saved_cameras, Some(cameras));
                assert_eq!(cold.world.pause, load_pause);
                assert_eq!(cold.hooks.timers, load_timers);
                assert_eq!(cold.world.clock, session.world.clock);
                let mut fresh = crate::live_host::Host::new(camera.eye);
                fresh.set_running(true);
                assert!(!fresh.options_save_allowed(&cold.world.pause, &cold.hooks.timers));
                let tick = cold.world.clock.timing[0];
                fresh.frame(&mut cold, &assets, 40);
                assert!(fresh.failure.is_none(), "{:?}", fresh.failure);
                assert!(
                    cold.world.clock.timing[0] > tick,
                    "cold logic must advance without opening/closing a new Options owner"
                );
                host.options(&mut session.world.pause, &mut session.hooks.timers, false)
                    .unwrap();
                assert_eq!(session.world.pause, expected_pause);
                assert_eq!(session.hooks.timers, expected_timers);
                if let Some(reason) = other {
                    session
                        .world
                        .pause
                        .unpause(reason, |c, _| {
                            c.apply(&mut session.hooks.timers);
                            Ok(())
                        })
                        .unwrap();
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
#[ignore = "requires original v1.4 game assets and audited scripts"]
fn retained_save_host_roundtrip_and_delayed_terms() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(||{
            use crate::fresh_city::{Inputs,Policy,CommandAssets};
            let read=|key|std::fs::read(openmt2001::fixtures::path(key)).unwrap();
            let exe=read("OPENMT2001_EXE");let image=openmt2001::pe::PeImage::parse(&exe).unwrap();
            let board=openmt2001::board::Board::from_v14(&image).unwrap();let build=read("OPENMT2001_BUILD_FILE");let file=openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let directory=read("OPENMT2001_ARCHIVE_DIR");let archive=read("OPENMT2001_ARCHIVE_BIN");let index=openmt2001::archive::ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
            let parameters=read("OPENMT2001_PARAMETERS");let timetable=read("OPENMT2001_TIMETABLE");
            let default=openmt2001::fixtures::path("DEFAULT_SETTINGS");let scripts=default.parent().unwrap().parent().unwrap();
            let styles=crate::fresh_city::load_era_styles(std::path::Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap();
            let names=std::array::from_fn(|_|Vec::new());
            let inputs=||Inputs{tree_locators: None, image:&image,board:&board,file:&file,people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts,parameters:std::str::from_utf8(&parameters).unwrap(),era_styles:styles.clone(),timetable:std::str::from_utf8(&timetable).unwrap(),scenario:"SCENARIO3",difficulty:0,selected_character:4,profile_name:b"Save fixture",character_names:&names};
            let policy=||Policy{seed:0x12345678,auxiliary_prior_bytes:[0;284]};
            let mut session=Session::initialize(inputs(),policy()).unwrap();
            // The supplied bank must reach this actual Session before map
            // production. Preserve an old save-owned rate10 across cold load.
            assert_eq!([session.world.building_parameters.row(0).unwrap(),session.world.building_parameters.row(1).unwrap(),session.world.building_parameters.row(2).unwrap()],[[3,15,4],[3,15,4],[0,15,4]]);
            let old_rate_slot=session.world.buildings.iter().position(|row|row[0x3f]!=0 && word(row,0x24)<2).expect("Scenario3 materialized retail/residential building");
            assert_eq!(word(&session.world.buildings[old_rate_slot],0x28),3,"map/scenario producer must receive configured rent");
            session.world.buildings[old_rate_slot][0x28..0x2c].copy_from_slice(&10u32.to_le_bytes());
            // Reach the human quote adapter with exact, independent operands:
            // volume2, prestige0, quality0, landscape before06 => base30.
            let prior_prestige: [u8;4]=session.world.categories[0][0x84..0x88].try_into().unwrap();
            let prior_components=session.world.construction_components;
            session.world.categories[0][0x84..0x88].copy_from_slice(&0f32.to_le_bytes());
            let mut quote_command=openmt2001::construction::Command { category:0,rectangle:[0,0,1,0],height:1,kind:2,business:0,variation:0,quality:0,owner:1,components:[0.;4] };
            assert_eq!(openmt2001::live_commands::quote(&mut quote_command,&mut session.world.city,&session.roster,&image,5).unwrap(),30);
            assert_eq!(quote_command.components.map(f32::to_bits),[30f32,0.,0.,0.].map(f32::to_bits));
            session.world.categories[0][0x84..0x88].copy_from_slice(&prior_prestige);
            session.world.construction_components=prior_components;
            let cold_build_state=session.build_random_state;
            let cold_dust_state=session.demolition_effects.random;
            let style_counts=session.era_styles[session.world.clock.calendar[2] as usize].qualities.each_ref().map(Vec::len);
            crate::build_panel::placement_choices(&mut session.build_random_state,style_counts).unwrap();
            assert_ne!(session.build_random_state,cold_build_state);
            session.campaign_profile=Some(std::path::PathBuf::from("work/save/campaign/536176652066697874757265.lua"));
            let camera=openmt2001::flyby::CameraSample{eye:[100.,120.,100.],target:[0.,0.,0.],up:[0.,1.,0.],vertical_fov:0.7};
            let cameras=crate::save_menu::cameras(None,camera).unwrap();
            let assets=CommandAssets{image:&image,board:&board,file:&file};
            let business=session.world.ledgers.businesses.iter().position(|b|b[0x21c]!=0).unwrap();
            let terms=openmt2001::economy::PendingBusinessTerms{business_index:word(&session.world.ledgers.businesses[business],0x210),terms:openmt2001::economy::BusinessTerms{stock_targets:[7;6],prices:[2.;6],rent:123.}};
            session.live_commands.submit(Command::Terms(terms),session.world.clock.timing[0],true,|_|Err("terms dispatched early".into())).unwrap();
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let root=std::path::PathBuf::from("work/save/v2-roundtrip-profile");let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let loaded=std::fs::read(&path).unwrap();let save=SaveFile::parse(&loaded).unwrap();
            let mut resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
            let native_allocation_next = resumed.allocation_next;
            let metadata=resumed.restore_v2_extension(&save).unwrap().unwrap();
            continuation_test::load_boundary(&mut session, &resumed, &assets, &save, native_allocation_next);
            assert_eq!(metadata.profile_filename.as_deref(),Some("536176652066697874757265.lua"));
            assert_eq!(resumed.world.clock,session.world.clock);
            assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses);
            assert_eq!(resumed.world.pending_world.to_bytes(),session.world.pending_world.to_bytes());
            assert_eq!(resumed.saved_cameras,Some(cameras));
            assert_eq!(resumed.world.building_parameters.row(0).unwrap(),[3,15,4]);
            assert_eq!(word(&resumed.world.buildings[old_rate_slot],0x28),10,"cold resource reinit must not rewrite saved rent");

            assert_eq!(resumed.world.creation.auxiliary.slots(),session.world.creation.auxiliary.slots());
            assert_eq!(resumed.roster.profiles,session.roster.profiles);
            assert_eq!(resumed.script_random_state,session.script_random_state);
            assert_eq!(resumed.build_random_state,session.build_random_state);
            let ext=extension::Extension::parse(&save).unwrap().unwrap();
            assert_eq!(ext.version,4);
            // Rebuild actual legacy record sets, rather than claiming the new
            // placement stream can be derived from their other RNG records.
            for legacy_version in [1,2,3] {
                let mut legacy=bytes[..save.sections.iter().find(|s|s.tag==extension::TAG).unwrap().offset].to_vec();
                legacy.extend_from_slice(&u32::MAX.to_le_bytes());legacy.extend_from_slice(&save.terminator_padding);
                let mut records:Vec<_>=ext.records.iter().filter(|(tag,_)|**tag<=13).map(|(tag,row)|(*tag,row.to_vec())).collect();
                if legacy_version==2 { records.push((15,words([3]))); }
                if legacy_version==3 { records.push((16,words([session.build_random_state]))); }
                extension::append_version(&mut legacy,&records,legacy_version).unwrap();
                let parsed=SaveFile::parse(&legacy).unwrap();
                assert_eq!(verified_modern_presentation_queue(&parsed).unwrap(), [legacy_version != 1],
                    "VERSION1 alone keeps synchronous native admission");
                if legacy_version == 1 {
                    let queue=extension::Extension::parse(&parsed).unwrap().unwrap().required(9).unwrap();
                    let at=queue.as_ptr() as usize-legacy.as_ptr() as usize;
                    let mut disagree=legacy.clone();
                    put(&mut disagree,at+8,word(&legacy,at+8).wrapping_add(1));
                    let disagree=SaveFile::parse(&disagree).unwrap();
                    assert!(verified_modern_presentation_queue(&disagree).unwrap_err()
                        .contains("queue identity"),
                        "VERSION1 must not replace a different native queue");
                    let mut direct=Session::load_original(inputs(),policy(),&legacy).unwrap();
                    assert!(direct.restore_v2_extension(&disagree).err().expect("tampered VERSION1 queue accepted")
                        .contains("queue identity"),
                        "direct VERSION1 restoration must reject a different native queue");
                }
                let mut old=Session::load_original(inputs(),policy(),&legacy).unwrap();
                assert_eq!(old.build_random_state,cold_build_state);
                old.restore_v2_extension(&parsed).unwrap();
                assert_eq!(old.build_random_state,if legacy_version==3 {session.build_random_state} else {cold_build_state});
                assert_eq!(old.demolition_effects.count,0);
                assert_eq!(old.demolition_effects.random,cold_dust_state);
            }
            let mut host=crate::live_host::Host::new(camera.eye);let mut resumed_host=crate::live_host::Host::new(camera.eye);
            host.running=true;resumed_host.running=true;
            for _ in 0..30 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
            }
            assert_eq!(resumed.world.clock,session.world.clock);
            assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses);
            assert_eq!(resumed.world.clock.timing[0],30);
            assert_eq!(word(&resumed.world.ledgers.businesses[business],0x1dc),123f32.to_bits());
            println!("V2 host saved {}: complete city/accounting/camera/profile/RNG state and delayed terms resume through 30 ticks",path.display());
            // Find a genuinely admitted original form, then save while native
            // pending-world growth is active rather than manufacturing a pool.
            let mut selected=None;
            'search: for category in 0..52 {
                if !matches!(crate::build_panel::Context::from_session(&session,category).unwrap().entry(),crate::build_panel::Entry::Open){continue;}
                for x in 0..=5 {for y in 0..=5 {
                    let request=crate::build_panel::Request{category:category as i32,origin:[x,y],size:[2,2],height:2,packed_model:0,quality:0,kind:0,displayed_quote:0.,business:0};
                    if session.inspect_build(&request,&assets).is_ok_and(|quote|quote.allowed()){selected=Some(request);break 'search;}
                }}
            }
            let request=selected.expect("Scenario3 has a legally quoted bakery lot");
            let slot=openmt2001::building_pool::first_free(&session.world.buildings).unwrap().unwrap();
            assert!(session.submit_build(&request,&assets,|event|Err(format!("build prematurely dispatched:{event:?}"))).unwrap().submitted);
            // A normal Save Game request can arrive during the 25-tick delay
            // after a legal Build submission. The original queue packet is
            // future-due here; compare its uninterrupted and cold V4 paths.
            let build_due=session.world.clock.timing[0]+25;
            let queue_image=|s:&Session<'_>|s.live_commands.snapshot().unwrap().into_iter()
                .map(|(domain,outer,inner,direct,command)|{
                    let domain=match domain {openmt2001::scheduler::ClockDomain::Total=>0u8,
                        openmt2001::scheduler::ClockDomain::Normal=>1u8};
                    let bytes=match command {Command::CategoryReady{category}=>category.to_le_bytes().to_vec(),
                        _=>packet(command,inner).unwrap()};
                    (domain,outer,inner,direct,bytes)
                }).collect::<Vec<_>>();
            let accounts=|s:&Session<'_>|s.world.ledgers.players.iter().map(|account|{
                let mut row=[0u8;0x1bc];account.write_runtime(&mut row).unwrap();row
            }).collect::<Vec<_>>();
            assert!(session.live_commands.snapshot().unwrap().iter().any(|(_,outer,inner,_,command)|
                *outer==build_due && *inner==build_due && matches!(command,Command::Build(_))));
            let pending_bytes=session.save_v2(&image,cameras,1).unwrap();
            let pending_save=SaveFile::parse(&pending_bytes).unwrap();
            assert!(verified_modern_presentation_queue(&pending_save).unwrap().iter().all(|deferred| !deferred));
            resumed=Session::load_original(inputs(),policy(),&pending_bytes).unwrap();
            let native_allocation_next = resumed.allocation_next;
            resumed.restore_v2_extension(&pending_save).unwrap();
            continuation_test::load_boundary(&mut session, &resumed, &assets, &pending_save, native_allocation_next);
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            assert_eq!(resumed.world.clock,session.world.clock);
            assert_eq!(queue_image(&resumed),queue_image(&session));
            assert_eq!(accounts(&resumed),accounts(&session));
            while session.world.clock.timing[0]<80 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(resumed.world.clock,session.world.clock);
                assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses);
                assert_eq!(resumed.world.pending_world.to_bytes(),session.world.pending_world.to_bytes());
                assert_eq!(queue_image(&resumed),queue_image(&session));
                assert_eq!(accounts(&resumed),accounts(&session));
                assert_eq!(&resumed.world.buildings[slot][..64],&session.world.buildings[slot][..64]);
            }
            assert!(session.world.clock.timing[0]>build_due);
            assert_eq!(word(&session.world.buildings[slot],0x28),3,"due human Build initializer uses configured rent");
            assert_eq!(word(&resumed.world.buildings[slot],0x28),3,"cold queued Build uses configured rent");
            assert_eq!(word(&resumed.world.buildings[old_rate_slot],0x28),10,"new construction must preserve old saved rate");

            assert_ne!(session.world.pending_world.count,0);
            assert_ne!(word(&session.world.buildings[slot],0x70),1f32.to_bits());
            assert!(!crate::intro::detail_model_at_unit_scale(&session.world.buildings[slot]),
                "native pass3 must not draw a legally growing bakery");
            let bytes=session.save_v2(&image,cameras,1).unwrap();let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let bytes=std::fs::read(path).unwrap();let save=SaveFile::parse(&bytes).unwrap();
            resumed=Session::load_original(inputs(),policy(),&bytes).unwrap();
            let native_allocation_next = resumed.allocation_next;
            resumed.restore_v2_extension(&save).unwrap();
            continuation_test::load_boundary(&mut session, &resumed, &assets, &save, native_allocation_next);
            assert!(!crate::intro::detail_model_at_unit_scale(&resumed.world.buildings[slot]),
                "restored growth must retain the native pass3 admission gate");
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            let mut prompt_deadlines=Vec::new();
            let mut open_due=None;
            while session.world.clock.timing[0]<700 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(resumed.world.clock,session.world.clock);
                let left_prompts=host.take_prompts(&session);let right_prompts=resumed_host.take_prompts(&resumed);
                assert_eq!(left_prompts,right_prompts,"named original scenario callback deadline changed");
                for prompt in left_prompts{prompt_deadlines.push(session.world.clock.timing[0]);session.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,true).unwrap();resumed.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,true).unwrap();}
                let differing=session.world.ledgers.businesses.iter().zip(&resumed.world.ledgers.businesses).position(|(a,b)|a!=b);
                assert!(differing.is_none(),"business state diverged at tick{} row{:?}",session.world.clock.timing[0],differing);
                assert_eq!(resumed.world.pending_world.to_bytes(),session.world.pending_world.to_bytes());
                // The load step draws no traffic RNG. Equal pedestrian lane
                // reservations must still yield identical later vehicle draws.
                assert_eq!(resumed.road_traffic.admission.random,session.road_traffic.admission.random);
                assert_eq!(resumed.world.creation.initialization_rng.constructor,session.world.creation.initialization_rng.constructor);
                if open_due.is_none() {
                    let queued=session.live_commands.snapshot().unwrap().into_iter()
                        .find_map(|(_,outer,inner,_,command)|match command {
                            Command::Open(open) if open.building==slot as u32=>Some((outer,inner)),
                            _=>None,
                        });
                    if let Some((outer,inner))=queued {
                        assert_eq!(outer,inner,"completion's Open packet keeps its inner deadline");
                        assert!(outer>session.world.clock.timing[0],"Open must be future-due at Save Game");
                        let open_bytes=session.save_v2(&image,cameras,1).unwrap();
                        let open_save=SaveFile::parse(&open_bytes).unwrap();
                        assert!(verified_modern_presentation_queue(&open_save).unwrap().iter().all(|deferred| !deferred));
                        resumed=Session::load_original(inputs(),policy(),&open_bytes).unwrap();
                        let native_allocation_next = resumed.allocation_next;
                        resumed.restore_v2_extension(&open_save).unwrap();
                        continuation_test::load_boundary(&mut session, &resumed, &assets, &open_save, native_allocation_next);
                        resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
                        assert_eq!(queue_image(&resumed),queue_image(&session));
                        assert_eq!(accounts(&resumed),accounts(&session));
                        assert_eq!(resumed.world.pending_world.to_bytes(),session.world.pending_world.to_bytes());
                        open_due=Some(outer);
                    }
                }
                if open_due.is_some_and(|due|session.world.clock.timing[0]<=due+2) {
                    assert_eq!(queue_image(&resumed),queue_image(&session),"Open queue continuation at tick{}",session.world.clock.timing[0]);
                    assert_eq!(accounts(&resumed),accounts(&session),"Open accounting at tick{}",session.world.clock.timing[0]);
                    assert_eq!(&resumed.world.buildings[slot][..64],&session.world.buildings[slot][..64]);
                }
            }
            assert!(open_due.is_some(),"legal construction never queued its delayed Open");
            assert!(!prompt_deadlines.is_empty(),"original CreatePrompt callback did not fire");
            assert_eq!(word(&session.world.buildings[slot],0x14),4);
            assert_eq!(session.world.pending_world.count,0);
            for completed in [&session, &resumed] {
                assert!(crate::intro::detail_model_at_unit_scale(&completed.world.buildings[slot]),
                    "completed bakery must admit its native descriptor rows");
            }
            let opened=word(&session.world.buildings[slot],0x1c) as usize;
            assert_ne!(session.world.ledgers.businesses[opened][0x21c],0);
            for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players){assert_eq!(left.bytes,right.bytes);}
            for (left,right) in session.world.ledgers.residents.nodes.iter().zip(&resumed.world.ledgers.residents.nodes){assert!(left.as_bytes()==right.as_bytes(),"resident state diverged after active-construction save");}
            // Controlled queue boundary, using its public restoration API:
            // preserve the legally built bakery and clocks without mutation.
            // The ordinary UI producer delays25 ticks; this represents a
            // pending due-now record that the modern serializer can retain.
            let seller=session.roster.manager.players.iter().position(|p|
                p.reference==session.roster.status.current_player).unwrap();
            assert_eq!(session.world.ledgers.businesses[opened][10],seller as u8+1);
            assert!(openmt2001::ai_actions::liquidation_score::business_value(
                &session.world.ledgers.businesses[opened],&session.world.buildings,&image,
            ).unwrap().to_f32()>0.,"legal bakery must exercise actual cash presentation");
            // A real ordinary completed model still needs synchronous485520.
            // Reject its due-now takeover before returning publishable bytes.
            let retained_queue=std::mem::take(&mut session.live_commands);
            let due=session.world.clock.timing[0];
            session.live_commands.restore_queued(Command::Takeover(openmt2001::business_takeover::Command {
                building:slot as u32,
            }),openmt2001::scheduler::ClockDomain::Total,due,due,false);
            assert!(session.save_v2(&image,cameras,1).unwrap_err()
                .contains("unverified synchronous appearance"));
            session.live_commands=retained_queue;
            let sale=Command::SellBusiness(openmt2001::business_sale::Command {
                business:opened as u32,player:seller as u8,
            });
            let due=session.world.clock.timing[0];
            session.live_commands.restore_queued(sale,
                openmt2001::scheduler::ClockDomain::Total,due,due,false);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let save=SaveFile::parse(&bytes).unwrap();
            assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
            // Native and extension identities must agree before dispatch is
            // deferred; changing only the redundant extension deadline fails.
            let queue=extension::Extension::parse(&save).unwrap().unwrap().required(9).unwrap();
            let queue_offset=queue.as_ptr() as usize-bytes.as_ptr() as usize;
            let mut disagree=bytes.clone();
            put(&mut disagree,queue_offset+4+8,due.wrapping_add(1));
            assert!(verified_modern_presentation_queue(&SaveFile::parse(&disagree).unwrap()).is_err());
            let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let loaded=std::fs::read(path).unwrap();
            let save=SaveFile::parse(&loaded).unwrap();
            resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
            let native_allocation_next = resumed.allocation_next;
            resumed.restore_v2_extension(&save).unwrap();
            continuation_test::load_boundary(&mut session, &resumed, &assets, &save, native_allocation_next);
            assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses,
                "loading must not execute the modern sale early");
            assert_eq!(resumed.live_commands.snapshot().unwrap().len(),1);
            let age=session.population.business_counts.age;
            let (left_sound,left_requests)=std::sync::mpsc::channel();
            let (right_sound,right_requests)=std::sync::mpsc::channel();
            host=crate::live_host::Host::new(camera.eye);host.running=true;host.gameplay_sound=Some(left_sound);
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;resumed_host.gameplay_sound=Some(right_sound);
            for turn in 0..2 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(resumed.world.clock,session.world.clock);
                assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses);
                for (left,right) in session.world.buildings.iter().zip(&resumed.world.buildings) {
                    assert_eq!(&left[..64],&right[..64]);
                    assert_eq!(word(left,0x4c),word(right,0x4c));
                    assert_eq!(&left[0x58..0x80],&right[0x58..0x80]);
                }
                for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players){assert_eq!(left.bytes,right.bytes);}
                assert_eq!(session.population.business_counts.age,age.wrapping_add(100),"sale ran more than once at turn{turn}");
                assert_eq!(resumed.population.business_counts.age,session.population.business_counts.age);
            }
            assert_eq!(session.world.ledgers.businesses[opened][10],0);
            assert_eq!(resumed.live_commands.snapshot().unwrap().len(),0);
            let ledger=|receiver:std::sync::mpsc::Receiver<crate::gameplay_sound::Request>|receiver.try_iter()
                .filter_map(|r|match r.operation {crate::gameplay_sound::Operation::Ledger{sale}=>Some(sale),_=>None}).collect::<Vec<_>>();
            assert_eq!(ledger(left_requests),[true]);assert_eq!(ledger(right_requests),[true]);
            assert!(host.cash_arrow_state().active);assert!(resumed_host.cash_arrow_state().active);
            let buyer=session.roster.manager.players[seller].bytes[0x1bc]+1;
            let mut acquired=None;
            for (amount,same_player) in [(50000i32,false),(-50000,false),(50000,true)] {
                let chains=openmt2001::building_pool::category_chains(
                    &session.world.categories,&session.world.buildings,0x591318).unwrap();
                let category=if same_player { acquired.unwrap() } else {
                    chains.iter().enumerate().find(|(index,chain)|
                        chain.is_empty() && session.world.categories[*index][5]==0
                            && ![2,35,39,42].contains(index)).unwrap().0
                };
                if acquired.is_none() { acquired=Some(category); }
                let due=session.world.clock.timing[0];
                session.live_commands.restore_queued(Command::PropertyTransfer {
                    category:category as i16,owner:buyer,amount,
                },openmt2001::scheduler::ClockDomain::Total,due,due,false);
                let bytes=session.save_v2(&image,cameras,1).unwrap();
                let save=SaveFile::parse(&bytes).unwrap();
                assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
                let path=crate::save_menu::write_new(&root,&bytes).unwrap();
                let loaded=std::fs::read(path).unwrap();
                let save=SaveFile::parse(&loaded).unwrap();
                resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
                let native_allocation_next = resumed.allocation_next;
                resumed.restore_v2_extension(&save).unwrap();
                continuation_test::load_boundary(&mut session, &resumed, &assets, &save, native_allocation_next);
                for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                    assert_eq!(left.bytes,right.bytes,"load must not post cash early");
                }
                assert_eq!(session.world.categories[category][5],resumed.world.categories[category][5]);
                assert_eq!(resumed.live_commands.snapshot().unwrap().len(),1);
                let (left_sound,left_requests)=std::sync::mpsc::channel();
                let (right_sound,right_requests)=std::sync::mpsc::channel();
                host=crate::live_host::Host::new(camera.eye);host.running=true;host.gameplay_sound=Some(left_sound);
                resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;resumed_host.gameplay_sound=Some(right_sound);
                for _ in 0..2 {
                    host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                    assert!(host.failure.is_none(),"{:?}",host.failure);
                    assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                    assert_eq!(session.world.clock,resumed.world.clock);
                    assert_eq!(session.world.ledgers.businesses,resumed.world.ledgers.businesses);
                    for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                        assert_eq!(left.bytes,right.bytes);
                    }
                    assert_eq!(session.world.city.ownership_refresh_counter,resumed.world.city.ownership_refresh_counter);
                    for (left,right) in session.world.categories.iter().zip(&resumed.world.categories) {
                        assert_eq!(left[5],right[5]);
                        assert_eq!(&left[0x6c..0x84],&right[0x6c..0x84]);
                    }
                    assert_eq!(openmt2001::building_pool::category_chains(&session.world.categories,&session.world.buildings,0x591318).unwrap(),
                        openmt2001::building_pool::category_chains(&resumed.world.categories,&resumed.world.buildings,0x591318).unwrap());
                    assert_eq!(host.cash_arrow_state(),resumed_host.cash_arrow_state());
                }
                assert_eq!(session.world.categories[category][5],buyer);
                assert!(session.live_commands.snapshot().unwrap().is_empty());
                assert!(resumed.live_commands.snapshot().unwrap().is_empty());
                let expected=if same_player {vec![true,false]} else {vec![amount<0]};
                assert_eq!(ledger(left_requests),expected);assert_eq!(ledger(right_requests),expected);
                assert!(host.cash_arrow_state().active);assert!(resumed_host.cash_arrow_state().active);
            }
            // The already-due zero-price release and direct category expiry
            // each pass through original4810e0 on load. A V4 save must leave
            // their effects pending until its authoritative worker resumes;
            // otherwise record9 reinstalls a command already consumed once.
            let released_category=acquired.unwrap();
            assert_eq!(session.world.categories[released_category][5],buyer);
            let due=session.world.clock.timing[0];
            session.live_commands.restore_queued(Command::PropertyTransfer {
                category:released_category as i16,owner:0,amount:0,
            },openmt2001::scheduler::ClockDomain::Total,due,due,false);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let save=SaveFile::parse(&bytes).unwrap();
            assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
            resumed=Session::load_original(inputs(),policy(),&bytes).unwrap();
            let native_allocation_next = resumed.allocation_next;
            resumed.restore_v2_extension(&save).unwrap();
            continuation_test::load_boundary(&mut session, &resumed, &assets, &save, native_allocation_next);
            assert_eq!(resumed.world.categories[released_category][5],buyer,
                "zero-price release must remain pending at load");
            host=crate::live_host::Host::new(camera.eye);host.running=true;
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
            assert!(host.failure.is_none(),"{:?}",host.failure);
            assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
            assert_eq!(session.world.categories[released_category][5],0);
            assert_eq!(resumed.world.categories[released_category][5],0);
            assert_eq!(resumed.world.city.ownership_refresh_counter,
                session.world.city.ownership_refresh_counter);
            assert!(session.live_commands.snapshot().unwrap().is_empty());
            assert!(resumed.live_commands.snapshot().unwrap().is_empty());

            let ready_category=released_category;
            let normal=session.world.clock.timing[1];
            openmt2001::category_ownership::begin_wait(
                &mut session.world.categories[ready_category],normal);
            session.live_commands.restore_queued(Command::CategoryReady {
                category:ready_category as u32,
            },openmt2001::scheduler::ClockDomain::Normal,normal,normal,true);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let save=SaveFile::parse(&bytes).unwrap();
            assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
            resumed=Session::load_original(inputs(),policy(),&bytes).unwrap();
            let native_allocation_next = resumed.allocation_next;
            resumed.restore_v2_extension(&save).unwrap();
            continuation_test::load_boundary(&mut session, &resumed, &assets, &save, native_allocation_next);
            assert_eq!(word(&resumed.world.categories[ready_category],0x78),1,
                "direct category expiry must remain pending at load");
            host=crate::live_host::Host::new(camera.eye);host.running=true;
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
            assert!(host.failure.is_none(),"{:?}",host.failure);
            assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
            assert_eq!(word(&session.world.categories[ready_category],0x78),2);
            assert_eq!(session.world.categories[ready_category],resumed.world.categories[ready_category]);
            assert!(session.live_commands.snapshot().unwrap().is_empty());
            assert!(resumed.live_commands.snapshot().unwrap().is_empty());
            // Continue the legally built bakery through the real demolition
            // producer, dust allocator, scheduler and geometry retirement.
            let category = ((word(&session.world.buildings[slot],0x40)-0x617628)/380) as usize;
            let amount = openmt2001::demolition::quote_with_parameters(&session.world.buildings[slot],
                &session.world.categories,&session.world.city.building_parameters,&image).unwrap().to_f32();
            session.live_commands.submit(Command::Demolish(openmt2001::demolition::Command {
                category:category as i8,building:slot as u16,amount,
            }),session.world.clock.timing[0],true,|_|Err("demolition dispatched early".into())).unwrap();
            let demolition=session.live_commands.snapshot().unwrap().into_iter()
                .find_map(|(_,_,_,_,command)| match command {
                    Command::Demolish(command)=>Some(command.clone()),_=>None,
                }).expect("legal demolition producer queued its command");
            let retained=std::mem::take(&mut session.live_commands);
            let due=session.world.clock.timing[0];
            session.live_commands.restore_queued(Command::Demolish(demolition),
                openmt2001::scheduler::ClockDomain::Total,due,due,false);
            assert!(session.save_v2(&image,cameras,1).unwrap_err()
                .contains("Cannot save due-now command 10"));
            session.live_commands=retained;
            for _ in 0..150 {
                host.tick(&mut session,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                if session.demolition_points.len()>=8 {break;}
            }
            assert!(session.demolition_points.len()>=8,"legal demolition must emit live dust");
            let effect = word(&session.world.buildings[slot],0x7c) as usize;
            assert!(effect<16);
            assert_eq!(session.demolition_effects.slots[effect].model,1);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let loaded=std::fs::read(path).unwrap();let save=SaveFile::parse(&loaded).unwrap();
            resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
            let native_allocation_next = resumed.allocation_next;
            resumed.restore_v2_extension_with_image(&save,&image).unwrap();
            continuation_test::load_boundary(&mut session, &resumed, &assets, &save, native_allocation_next);
            assert_eq!(word(&resumed.world.buildings[slot],0x7c),effect as u32);
            assert_eq!(session.demolition_points,resumed.demolition_points);
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            for _ in 0..350 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(openmt2001::save::demolition::encode(&session.demolition_effects).unwrap(),
                    openmt2001::save::demolition::encode(&resumed.demolition_effects).unwrap());
                assert_eq!(session.demolition_points,resumed.demolition_points);
                assert_eq!(session.world.pending_world.to_bytes(),resumed.world.pending_world.to_bytes());
                assert_eq!(session.world.clock,resumed.world.clock);
                assert_eq!(session.world.ledgers.businesses,resumed.world.ledgers.businesses);
                for offset in [0x14,0x5c,0x70,0x74,0x78,0x7c] {
                    assert_eq!(word(&session.world.buildings[slot],offset),
                        word(&resumed.world.buildings[slot],offset),"demolition field{offset:x}");
                }
                assert_eq!(openmt2001::building_pool::category_chains(
                    &session.world.categories,&session.world.buildings,0x591318).unwrap(),
                    openmt2001::building_pool::category_chains(
                    &resumed.world.categories,&resumed.world.buildings,0x591318).unwrap());
                for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                    assert_eq!(left.bytes,right.bytes);
                }
                if !session.demolition_effects.allocated() && session.world.pending_world.count==0 {break;}
            }
            assert!(!session.demolition_effects.allocated(),"dust must naturally retire");
            assert_eq!(session.world.pending_world.count,0);
            assert!(!session.demolition_effects.slots[effect].positions.is_empty());
            let retired= openmt2001::save::demolition::encode(&session.demolition_effects).unwrap();
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let save=SaveFile::parse(&bytes).unwrap();
            let mut after=Session::load_original(inputs(),policy(),&bytes).unwrap();
            after.restore_v2_extension_with_image(&save,&image).unwrap();
            assert_eq!(openmt2001::save::demolition::encode(&after.demolition_effects).unwrap(),retired);
            assert!(after.demolition_points.is_empty());
            // Original planning type2/business44 reaches the genuine linked
            // structure branch that does not refresh485520. No flags/status
            // are manufactured to admit the saved takeover.
            let (park_sound, _park_requests) = std::sync::mpsc::channel();
            host.gameplay_sound = Some(park_sound);
            let mut park = None;
            'park_search: for category in 0..52 {
                if !matches!(crate::build_panel::Context::from_session(&session,category).unwrap().entry(),
                    crate::build_panel::Entry::Open) {continue;}
                for x in 0..=5 {for y in 0..=5 {
                    let request=crate::build_panel::Request {category:category as i32,origin:[x,y],
                        size:[2,2],height:1,packed_model:0,quality:0,kind:2,displayed_quote:0.,business:44};
                    if session.inspect_build(&request,&assets).is_ok_and(|q|q.allowed()) {
                        park=Some(request);break 'park_search;
                    }
                }}
            }
            let park=park.expect("actual Scenario3 catalog must admit a legal type2 structure");
            let park_slot=openmt2001::building_pool::first_free(&session.world.buildings).unwrap().unwrap();
            assert!(session.submit_build(&park,&assets,|_|Ok(())).unwrap().submitted);
            // Submit queues the build command; the scene row appears only
            // after the Host consumes it on a simulation tick.
            for _ in 0..350 {
                host.tick(&mut session,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                if word(&session.world.buildings[park_slot],0x24)==2 {break;}
            }
            assert_eq!(word(&session.world.buildings[park_slot],0x24),2);
            assert_ne!(word(&session.world.buildings[park_slot],0x14),4);
            let suncols=read("OPENMT2001_SUNCOLS");
            let before_open=session.scene(&build,openmt2001::render_scene::OriginalAssets {
                executable:&exe,directory:&directory,archive:&archive,
            },&suncols,Some(camera),false).unwrap();
            let pending_park=before_open.inspection_buildings.iter()
                .find(|building|building.runtime_slot==Some(park_slot)).unwrap();
            assert_eq!(pending_park.kind,2);
            assert!(!crate::intro::building_has_management_link(pending_park),
                "original 4c39ed must present Block Information before the Landscape Open link");
            for _ in 0..350 {
                host.tick(&mut session,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                for prompt in host.take_prompts(&session) {
                    session.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,true).unwrap();
                }
                if word(&session.world.buildings[park_slot],0x14)==4
                    && word(&session.world.buildings[park_slot],0x1c)!=u32::MAX {break;}
            }
            assert_eq!(word(&session.world.buildings[park_slot],0x14),4);
            assert_eq!(word(&session.world.buildings[park_slot],0x24),2);
            assert_ne!(word(&session.world.buildings[park_slot],0x1c),u32::MAX,
                "legal landscape Open must finish: tick{}, pause{:?}, queued{}",
                session.world.clock.timing[0],session.world.pause.bytes,session.live_commands.snapshot().unwrap().len());
            let park_business=word(&session.world.buildings[park_slot],0x1c) as usize;
            assert_eq!(word(&session.world.ledgers.businesses[park_business],4),44);
            // The original 407970 type2 building-panel path receives the
            // linked landscape business after Open, with its action tabs
            // gated by class2. Retain that link in the frontend snapshot.
            let scene=session.scene(&build,openmt2001::render_scene::OriginalAssets {
                executable:&exe,directory:&directory,archive:&archive,
            },&suncols,Some(camera),false).unwrap();
            let park_view=scene.inspection_buildings.iter()
                .find(|building|building.runtime_slot==Some(park_slot)).unwrap();
            assert!(crate::intro::building_has_management_link(park_view),
                "original 4c39f3 must present Building Information after Landscape Open");
            let park_link=park_view.business().expect("opened Landscape must reach the original building panel");
            assert_eq!(park_link.slot,park_business);
            assert_eq!(word(&park_link.record,0),2);
            let park_panel=session.inspect_business_operations(park_slot,[30.,29.,190.,300.],&image).unwrap();
            assert_eq!(park_panel.report.summary.len(),3);
            assert!(park_panel.permissions.modes[0]);
            assert!(!park_panel.permissions.modes[1]);
            assert!(park_panel.trade_price.is_none());
            let owner=session.roster.manager.players[seller].bytes[0x1bc]+1;
            session.live_commands.submit(Command::PropertyTransfer {category:park.category as i16,
                owner,amount:50000},session.world.clock.timing[0],true,
                |_|Err("land acquisition dispatched early".into())).unwrap();
            for _ in 0..30 {host.tick(&mut session,&assets);assert!(host.failure.is_none(),"{:?}",host.failure);}
            assert_eq!(session.world.categories[park.category as usize][5],owner);
            assert_eq!(session.world.ledgers.businesses[park_business][10],owner);
            let references:Vec<_>=session.roster.manager.players.iter().map(|p|p.reference).collect();
            assert!(openmt2001::business_takeover::price(park_slot,&openmt2001::business_takeover::PriceWorld {
                categories:&session.world.categories,buildings:&session.world.buildings,
                businesses:&session.world.ledgers.businesses,player_references:&references,
            },session.world.clock.timing[1],&image).unwrap().to_f32()>0.,
                "actual park must exercise both current-player cash postings");
            assert!(session.world.clock.timing[0] > 0);
            session.live_commands.restore_queued(Command::Takeover(openmt2001::business_takeover::Command {
                building:park_slot as u32,
            }),openmt2001::scheduler::ClockDomain::Total,session.world.clock.timing[0],
                session.world.clock.timing[0] - 1,false);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let loaded=std::fs::read(path).unwrap();let save=SaveFile::parse(&loaded).unwrap();
            assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
            resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
            let native_allocation_next = resumed.allocation_next;
            resumed.restore_v2_extension_with_image(&save,&image).unwrap();
            continuation_test::load_boundary(&mut session, &resumed, &assets, &save, native_allocation_next);
            assert_eq!(session.world.ledgers.businesses,resumed.world.ledgers.businesses);
            for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                assert_eq!(left.bytes,right.bytes,"takeover must not post during loading");
            }
            let (left_sound,left_requests)=std::sync::mpsc::channel();
            let (right_sound,right_requests)=std::sync::mpsc::channel();
            host=crate::live_host::Host::new(camera.eye);host.running=true;host.gameplay_sound=Some(left_sound);
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;resumed_host.gameplay_sound=Some(right_sound);
            for _ in 0..30 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(session.world.clock,resumed.world.clock);
                assert_eq!(session.world.ledgers.businesses,resumed.world.ledgers.businesses);
                assert_eq!(session.population.business_counts.age,resumed.population.business_counts.age);
                assert_eq!(session.population.business_counts.values,resumed.population.business_counts.values);
                assert_eq!(session.world.pending_world.to_bytes(),resumed.world.pending_world.to_bytes());
                for (left,right) in session.world.buildings.iter().zip(&resumed.world.buildings) {
                    assert_eq!(&left[..64],&right[..64]);
                    assert_eq!(word(left,0x4c),word(right,0x4c));
                }
                for (left,right) in session.world.categories.iter().zip(&resumed.world.categories) {
                    assert_eq!(left[5],right[5]);assert_eq!(&left[0x6c..0x8c],&right[0x6c..0x8c]);
                }
                assert_eq!(openmt2001::building_pool::category_chains(
                    &session.world.categories,&session.world.buildings,0x591318).unwrap(),
                    openmt2001::building_pool::category_chains(
                    &resumed.world.categories,&resumed.world.buildings,0x591318).unwrap());
                assert_eq!(session.roster.profiles,resumed.roster.profiles);
                for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                    assert_eq!(left.bytes,right.bytes);
                    assert_eq!(left.auxiliaries.len(),right.auxiliaries.len());
                    for (left,right) in left.auxiliaries.iter().zip(&right.auxiliaries) {
                        assert_eq!(left.reference,right.reference);assert_eq!(left.bytes,right.bytes);
                    }
                }
                assert_eq!(host.cash_arrow_state(),resumed_host.cash_arrow_state());
            }
            assert!(session.live_commands.snapshot().unwrap().is_empty());
            assert!(resumed.live_commands.snapshot().unwrap().is_empty());
            assert_eq!(ledger(left_requests),[true,false]);assert_eq!(ledger(right_requests),[true,false]);
            println!("V4 host save acceptance: legal bakery construction/open/hour plus live demolition dust and retired pool/RNG continuation");
        }).unwrap().join().unwrap();
}
