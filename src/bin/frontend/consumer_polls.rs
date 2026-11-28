//! Read-only original48b030 apartment consumer poll over saved linked lists.
use openmt2001::save::SaveFile;
#[derive(Clone, Debug, Default)]
pub struct Poll {
    pub counts: [u16; 32],
    pub residents: u16,
}
impl Poll {
    /// Read the same apartment/resident links as 48b030 from the retained
    /// world. Saved +20c/+ec indices remain indices in the live pools.
    pub fn from_runtime(world: &openmt2001::level_startup::World) -> Result<[Self; 52], String> {
        let mut polls = std::array::from_fn(|_| Self::default());
        let commodities = world.commodities.resident_inputs();
        let active = std::array::from_fn(|id| commodities[id].active);
        // The UI is a fresh, read-only view of the same native-backed query
        // used by AI/valuation. Keep its linked-list and filtering semantics
        // authoritative in consumer_poll::start rather than walking it again.
        let mut player = openmt2001::player_manager::Player {
            reference: 0,
            bytes: vec![0; 0x1e50],
            auxiliaries: vec![],
        };
        for (category, poll) in polls.iter_mut().enumerate() {
            openmt2001::consumer_poll::clear(&mut player);
            openmt2001::consumer_poll::start(
                &mut player,
                category as i32,
                &world.categories,
                &world.buildings,
                &world.ledgers.businesses,
                &world.ledgers.residents,
                &active,
            )?;
            poll.residents = u16::from_le_bytes(player.bytes[0x208..0x20a].try_into().unwrap());
            poll.counts = std::array::from_fn(|id| {
                let at = 0x1c8 + id * 2;
                u16::from_le_bytes(player.bytes[at..at + 2].try_into().unwrap())
            });
        }
        Ok(polls)
    }
    pub fn from_save(save: &SaveFile<'_>) -> Result<[Self; 52], String> {
        let mut polls = std::array::from_fn(|_| Self::default());
        let residents = save.residents()?;
        let economy = save.economy()?;
        let businesses: Vec<_> = economy.businesses().collect();
        let commodities = economy.resident_commodities();
        for building in save.buildings()? {
            let business = i32::from_le_bytes(building.bytes[0x1c..0x20].try_into().unwrap());
            if business == -1 {
                continue;
            }
            let business = businesses
                .get(usize::try_from(business).map_err(|_| "negative business index")?)
                .ok_or("poll business exceeds saved pool")?;
            if business.business_type() != 42 {
                continue;
            }
            let poll = &mut polls[building.category];
            let mut next = i32::from_le_bytes(business.bytes[0x20c..0x210].try_into().unwrap());
            let mut visited = vec![false; residents.nodes.len()];
            while next != -1 {
                let index = usize::try_from(next).map_err(|_| "negative poll resident index")?;
                let node = residents
                    .nodes
                    .get(index)
                    .ok_or("poll resident exceeds saved pool")?;
                if visited[index] {
                    return Err("cyclic saved apartment resident list".into());
                }
                visited[index] = true;
                let account = i16::from_le_bytes(node[0x68..0x6a].try_into().unwrap());
                let account = usize::try_from(account)
                    .map_err(|_| "poll resident has no spending account")?;
                let at = account
                    .checked_mul(0x90)
                    .ok_or("poll account offset overflow")?
                    + 0x84;
                let bytes = residents
                    .accounts
                    .get(at..at + 4)
                    .ok_or("poll account exceeds saved pool")?;
                let mask = u32::from_le_bytes(bytes.try_into().unwrap());
                poll.residents = poll.residents.wrapping_add(1);
                for (id, commodity) in commodities.iter().enumerate() {
                    if commodity.active && mask & (1 << id) != 0 {
                        poll.counts[id] = poll.counts[id].wrapping_add(1);
                    }
                }
                next = i32::from_le_bytes(node[0xec..0xf0].try_into().unwrap());
            }
        }
        Ok(polls)
    }
    /// Original4123df nested swaps preserve its tie order; at most17 rows.
    pub fn ranked(&self) -> Vec<(usize, u16)> {
        let mut rows: Vec<_> = self.counts.iter().copied().enumerate().collect();
        for i in 0..31 {
            for j in (i + 1..32).rev() {
                if rows[i].1 < rows[j].1 {
                    rows.swap(i, j);
                }
            }
        }
        rows.into_iter()
            .take(17)
            .take_while(|row| row.1 != 0)
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn submit_legal_business(
        session: &mut openmt2001::session::Session<'_>,
        assets: &openmt2001::session::CommandAssets<'_, '_>,
        business: u32,
        excluded: Option<i32>,
    ) -> i32 {
        let info = session
            .world
            .catalog
            .info
            .row(business as usize)
            .unwrap()
            .to_vec();
        let word = |at| i32::from_le_bytes(info[at..at + 4].try_into().unwrap());
        let kind = word(0xc) as u32;
        let height = word(if business == 42 { 0x44 } else { 0x48 });
        let dimensions = if business == 42 {
            (1..=6).rev().collect::<Vec<_>>()
        } else {
            (1..=6).collect()
        };
        for category in 0..52 {
            if excluded == Some(category)
                || !matches!(
                    crate::build_panel::Context::from_session(session, category as usize)
                        .unwrap()
                        .entry(),
                    crate::build_panel::Entry::Open
                )
            {
                continue;
            }
            for &width in &dimensions {
                for &depth in &dimensions {
                    if width * depth < word(0x38)
                        || width * depth > word(0x3c)
                        || width.max(depth) > word(0x40)
                    {
                        continue;
                    }
                    for x in 0..=7 - width {
                        for y in 0..=7 - depth {
                            let request = crate::build_panel::Request {
                                category,
                                origin: [x, y],
                                size: [width, depth],
                                height,
                                packed_model: 0,
                                kind,
                                quality: 0,
                                displayed_quote: 0.,
                                business,
                            };
                            let inspected = session.inspect_build(&request, assets).unwrap();
                            if !inspected.allowed() {
                                continue;
                            }
                            let form = crate::build_panel::Form {
                                category,
                                rectangle: [x, y, x + width - 1, y + depth - 1],
                                height,
                                quality: 0,
                                choices: [0; 3],
                                variants: vec![0],
                                kind,
                                business,
                                displayed_quote: inspected.total,
                            };
                            let request = form
                                .request(
                                    inspected.category_locked,
                                    inspected.affordable,
                                    inspected.preview.is_some(),
                                )
                                .unwrap()
                                .unwrap();
                            assert!(
                                session
                                    .submit_build(&request, assets, |_| Err(
                                        "build applied before native deadline".into()
                                    ))
                                    .unwrap()
                                    .submitted
                            );
                            println!(
                                "legal human business{business} build: category{category} size{width}x{depth} height{height} quote{}",
                                inspected.total
                            );
                            return category;
                        }
                    }
                }
            }
        }
        panic!("no legal business{business} build available");
    }
    #[test]
    #[ignore = "requires original assets; naturally housed live respondents and retained daily continuation"]
    fn naturally_housed_live_polls() {
        std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
            use openmt2001::session::{Session, Inputs, Policy, CommandAssets, character_names, load_era_styles};
            use std::{fs, path::Path};
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable = read("OPENMT2001_EXE");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let build = read("OPENMT2001_BUILD_FILE");
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let exe = openmt2001::fixtures::path("EXE");
            let scripts = exe.parent().unwrap().join("scripts");
            let names = character_names(&image, &exe.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
            let mut session = Session::initialize(Inputs {
                tree_locators: None,
                image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO3", difficulty: 0,
                selected_character: 4, profile_name: b"Live poll validation", character_names: &names,
            }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
            let initial_cash = session.world.ledgers.players[0].cash;
            assert!(Poll::from_runtime(&session.world).unwrap().iter().all(|p| p.residents == 0));
            let assets = CommandAssets { image: &image, board: &board, file: &file };
            // Original map apartments charge60, above120*0.48. Build a
            // human apartment through the actual form and lower its rent using
            // the original arrows/Apply; subsequent housing is untouched.
            let apartment_category = submit_legal_business(&mut session, &assets, 42, None);
            // A real shop ties the same retained accounts to sales and the
            // overnight restock/history/report path, without injected stock.
            submit_legal_business(&mut session, &assets, 0, Some(apartment_category));
            let mut host = crate::live_host::Host::new([0., 20., 0.]);
            host.set_running(true);
            let mut snapshots = 0;
            let mut rent_target = None;
            for _ in 0..16_000 {
                host.frame(&mut session, &assets, 40);
                assert!(host.failure.is_none(), "natural housing stopped: {:?}", host.failure);
                for prompt in host.take_prompts(&session) {
                    if let Some(dialog) = prompt.dialog {
                        assert_ne!(dialog.callback, 0x48b490, "unexpected early terminal result");
                        host.human_dialog_reply(&mut session, &assets, crate::human_dialog::Reply {
                            callback: dialog.callback, context: dialog.context,
                            button: if dialog.button_count == 2 { 1 } else { 0 },
                        }).unwrap();
                    } else if prompt.dismissal_id != 0 {
                        session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32, &assets, false).unwrap();
                    }
                }
                if rent_target.is_none() && let Some(business) = session.world.ledgers.businesses.iter().position(|b| b[0x21c] != 0 && b[10] == 1 && b[4..8] == 42u32.to_le_bytes()) {
                    let building = usize::from(u16::from_le_bytes(session.world.ledgers.businesses[business][8..10].try_into().unwrap()));
                    let target = crate::business_edit::Target { business, building };
                    for _ in 0..501 { crate::business_edit::edit(&mut session, &assets, target, crate::business_edit::Action::Rent { increase: false }).unwrap(); }
                    crate::business_edit::edit(&mut session, &assets, target, crate::business_edit::Action::Apply).unwrap();
                    rent_target = Some(target);
                }
                if session.world.clock.calendar[4] >= 1 && session.world.clock.hour() >= 7 { break; }
            }
            assert!(session.world.clock.calendar[4] >= 1 && session.world.clock.hour() >= 7);
            crate::session_validation::assert_live_links(&session);
            let before_players = session.roster.manager.players.iter().map(|p| p.bytes.clone()).collect::<Vec<_>>();
            let polls = Poll::from_runtime(&session.world).unwrap();
            let respondents: u32 = polls.iter().map(|p| u32::from(p.residents)).sum();
            assert!(rent_target.is_some(), "legal apartment never opened");
            let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
            println!("housing diagnostic: available{} apartments{:?} resident states{:?}", session.world.available.len(),
                session.world.ledgers.businesses.iter().enumerate().filter(|(_,b)|word(*b,4)==42 && b[0x21c]!=0).map(|(slot,b)|(slot,word(b,0x20c) as i32,u16::from_le_bytes(b[0xe..0x10].try_into().unwrap()),u16::from_le_bytes(b[0x10..0x12].try_into().unwrap()),f32::from_bits(word(b,0x1dc)))).collect::<Vec<_>>(),
                session.world.ledgers.residents.nodes.iter().take(10).map(|r|{let b=r.as_bytes();(r.housing_building(),word(b,0xcc),word(b,0xd0),word(b,0xb8))}).collect::<Vec<_>>());
            assert!(respondents > 0, "natural housing did not populate apartment chains: {respondents}");
            assert!(polls.iter().any(|p| p.counts.iter().any(|&n| n != 0)), "real accounts produced no demand");
            let financial = crate::player_info::State::from_runtime(&session.roster.manager, &session.world.ledgers.players, session.roster.status.current_player).unwrap();
            assert_eq!(u32::from_le_bytes(financial.records[0][..4].try_into().unwrap()), session.world.ledgers.players[0].cash.to_bits());
            assert_ne!(session.world.ledgers.players[0].cash.to_bits(), initial_cash.to_bits(), "overnight retained rent/income did not reach player ledger");
            let rental_income = session.world.ledgers.players[0].ledger(0x21).unwrap();
            assert!(rental_income > 0., "natural morning did not pay housed tenants' rent");
            let report_rows = crate::player_info::report(&financial.records[0], true).unwrap();
            assert_eq!(report_rows.iter().find(|r|r.label == 0x5bb).unwrap().amount, rental_income.to_bits(), "UI financial rent row diverged from retained ledger");
            let target = rent_target.unwrap();
            let apartment = &session.world.ledgers.businesses[target.business];
            assert_eq!(u16::from_le_bytes(apartment[0x10..0x12].try_into().unwrap()), polls.iter().map(|p|p.residents).sum::<u16>(), "UI respondents differ from naturally occupied human apartment");
            assert_eq!(f32::from_le_bytes(apartment[0x50..0x54].try_into().unwrap()).to_bits(), rental_income.to_bits(), "business and player rental postings diverged");
            let bakery = session.world.ledgers.businesses.iter().find(|b|b[0x21c] != 0 && b[10] == 1 && b[4..8] == 0u32.to_le_bytes()).expect("legal bakery never opened");
            let sales = session.world.ledgers.players[0].retail_counters[1];
            let history_sales = (0..4).map(|i| i32::from(i16::from_le_bytes(bakery[0x82+i*2..0x84+i*2].try_into().unwrap()))).sum::<i32>();
            assert!(sales > 0, "legal bakery made no natural sales");
            assert_eq!(sales, history_sales, "retained player sales units diverged from original business history ring");
            assert_eq!(u16::from_le_bytes(bakery[0x80..0x82].try_into().unwrap()), u16::from_le_bytes(bakery[0x1b8..0x1ba].try_into().unwrap()), "next real morning did not restock to original target");
            assert!(f32::from_le_bytes(bakery[0xa8..0xac].try_into().unwrap()) > 0., "daily rollover lost yesterday's retail proceeds");
            let comparison = crate::comparison::Data::from_runtime(&session.world, &financial.records, &image, 0x591318).unwrap();
            assert_eq!(comparison.rows[0].1[8], sales as f32, "UI sales objective report diverged from retained sales counter");
            println!("natural bakery sales{sales} equals history{history_sales}, previous revenue{}, morning stock{} target{}", f32::from_le_bytes(bakery[0xa8..0xac].try_into().unwrap()), u16::from_le_bytes(bakery[0x80..0x82].try_into().unwrap()), u16::from_le_bytes(bakery[0x1b8..0x1ba].try_into().unwrap()));
            let people = crate::city_people::Report::from_world(&session.world, &image,
                &session.roster.manager).unwrap();
            assert_eq!(people.rows[0].value, session.world.ledgers.residents.active_count as i32);
            assert_eq!(before_players, session.roster.manager.players.iter().map(|p| p.bytes.clone()).collect::<Vec<_>>(), "read-only polls changed retained player state");
            if let Ok(path) = std::env::var("OPENMT2001_NATURAL_POLL_EXPORT") {
                let mut bytes = b"MTNPOL01".to_vec();
                for n in [session.world.buildings.len(), session.world.ledgers.businesses.len(), session.world.ledgers.residents.nodes.len(), session.world.ledgers.residents.accounts.len()] { bytes.extend_from_slice(&(n as u32).to_le_bytes()); }
                for row in &session.world.categories { bytes.extend_from_slice(row); }
                for row in &session.world.buildings { bytes.extend_from_slice(row); }
                for row in &session.world.ledgers.businesses { bytes.extend_from_slice(row); }
                for row in &session.world.ledgers.residents.nodes { bytes.extend_from_slice(row.as_bytes()); }
                for row in &session.world.ledgers.residents.accounts { bytes.extend_from_slice(row.as_bytes()); }
                for row in session.world.commodities.resident_inputs() { bytes.push(u8::from(row.active)); }
                for poll in &polls { for n in poll.counts.iter().chain([&poll.residents, &poll.residents]) { bytes.extend_from_slice(&n.to_le_bytes()); } }
                fs::write(path, bytes).unwrap(); snapshots += 1;
            }
            println!("natural Scenario3 housing day{} hour{} respondents{} occupied-blocks{} population{} cash{} -> {} native snapshots{}", session.world.clock.calendar[4], session.world.clock.hour(), respondents, polls.iter().filter(|p|p.residents != 0).count(), session.world.ledgers.residents.active_count, initial_cash, session.world.ledgers.players[0].cash, snapshots);
        }).unwrap().join().unwrap();
    }
    #[test]
    #[ignore = "requires private original poll fixture and supplied saves"]
    fn matches_native_consumer_polls() {
        let fixture = std::fs::read(openmt2001::fixtures::path("NATIVE_CONSUMER_POLLS")).unwrap();
        assert_eq!(&fixture[..8], b"MTCPOLL2");
        assert_eq!(u32::from_le_bytes(fixture[8..12].try_into().unwrap()), 2);
        let mut at = 12;
        for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let bytes = std::fs::read(openmt2001::fixtures::path(key)).unwrap();
            at += 32;
            for (category, poll) in Poll::from_save(&SaveFile::parse(&bytes).unwrap())
                .unwrap()
                .iter()
                .enumerate()
            {
                let expected: Vec<_> = fixture[at..at + 68]
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                    .collect();
                assert_eq!(
                    poll.counts.as_slice(),
                    &expected[..32],
                    "{key} block{category}"
                );
                assert_eq!(
                    poll.residents, expected[32],
                    "{key} block{category} queried"
                );
                assert_eq!(
                    poll.residents, expected[33],
                    "{key} block{category} responded"
                );
                at += 68;
                let row_count =
                    u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap()) as usize;
                at += 4;
                let expected: Vec<_> = fixture[at..at + row_count * 8]
                    .chunks_exact(8)
                    .map(|b| {
                        (
                            u32::from_le_bytes(b[..4].try_into().unwrap()),
                            u32::from_le_bytes(b[4..].try_into().unwrap()),
                        )
                    })
                    .collect();
                at += row_count * 8;
                let save = SaveFile::parse(&bytes).unwrap();
                let catalog = save
                    .sections
                    .iter()
                    .find(|s| s.tag == 0x200000)
                    .unwrap()
                    .payload;
                let actual: Vec<_> = if poll.residents == 0 {
                    vec![(1138, 0)]
                } else {
                    poll.ranked()
                        .iter()
                        .map(|(id, n)| {
                            (
                                u32::from_le_bytes(
                                    catalog[id * 0x1c + 8..id * 0x1c + 12].try_into().unwrap(),
                                ),
                                u32::from(*n),
                            )
                        })
                        .collect()
                };
                assert_eq!(actual, expected, "{key} block{category} ranked rows");
            }
        }
        assert_eq!(at, fixture.len());
    }
}
