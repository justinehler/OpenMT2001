//! Test-only, read-only witnesses. No second encoder or state edits.
//! Hooks observe actual production boundaries; no synthetic callback outcomes.
//! Record11 comparison deliberately reuses its existing transport codec; it
//! checks which live owner was lent, not independent original codec fidelity.
use super::*;
use std::cell::RefCell;

#[derive(Clone, Debug)]
pub(crate) struct Cut {
    pub clock: openmt2001::clock::Clock,
    pub rng: Vec<u32>,
    pub build_rng: u32,
    pub utilities: Vec<u32>,
    pub queue: Vec<(u32,u32,u32,bool,Vec<u8>)>,
    pub scripts: Vec<(String,Vec<u8>,u32,i32)>,
    pub players: Vec<(u32,Vec<u8>,Vec<(u32,[u8;48])>)>,
    pub roads: Vec<u8>,
    pub runtime_prefix: Vec<u8>,
    pub category_ids: Vec<Vec<u32>>,
    pub category_before: Vec<Vec<u32>>,
    pub category_after: Option<Vec<Vec<u32>>>,
    pub auction_before: [u8;344],
    pub auction_after: Option<[u8;344]>,
    pub auction_countdown: u32,
    pub auction_active: bool,
}
thread_local! {
    static CUTS: RefCell<Option<Vec<Cut>>> = const {RefCell::new(None)};
    static RESTORED: RefCell<Option<Vec<Vec<u8>>>> = const {RefCell::new(None)};
}
pub(crate) struct Observe;
impl Observe {
    pub fn start()->Self {
        CUTS.with(|v|assert!(v.borrow_mut().replace(vec![]).is_none()));
        RESTORED.with(|v|assert!(v.borrow_mut().replace(vec![]).is_none()));
        Self
    }
    pub fn take(&self)->Vec<Cut>{CUTS.with(|v|std::mem::take(v.borrow_mut().as_mut().unwrap()))}
    pub fn discard_initial_restore(&self) {
        RESTORED.with(|v|{
            let mut observed=v.borrow_mut();let rows=observed.as_mut().unwrap();
            assert_eq!(rows.len(),1,"exactly one initial fixture load before natural progression");
            rows.clear();
        });
    }
    pub fn assert_no_pending_restores(&self) {
        RESTORED.with(|v|assert!(v.borrow().as_ref().unwrap().is_empty(),"unconsumed restore witness"));
    }
}
impl Drop for Observe {fn drop(&mut self){
    CUTS.with(|v|*v.borrow_mut()=None);
    RESTORED.with(|v|*v.borrow_mut()=None);
}}
/// Called by saved_preview immediately after its actual extension restoration,
/// before resident repair, profile reconciliation and inspect_fresh. Immutable
/// bytes retain every record11 field, including first-frame transport state.
pub(crate) fn after_restore(s:&Session<'_>) {
    if !RESTORED.with(|v|v.borrow().is_some()){return;}
    let bytes=transport::encode(s).unwrap();
    RESTORED.with(|v|v.borrow_mut().as_mut().unwrap().push(bytes));
}
fn ids(rows:&[[u8;380];52])->Vec<Vec<u32>> {
    rows.iter().map(|row|(0..24).map(|i|word(row,12+4*i)).collect()).collect()
}
pub(crate) fn before(p:&mut SaveProjection<'_>) {
    if !CUTS.with(|v|v.borrow().is_some()){return;}
    let scripts=p.scripts.events.pending().iter().map(|e|{
        let (name,body)=p.scripts.runtime.event_reference_identity(e.reference).unwrap().unwrap();
        (name,body,e.remaining,e.argument)
    }).collect();
    let queue=p.live_commands.snapshot().unwrap().into_iter().map(|(domain,outer,inner,direct,c)| {
        let domain=match domain {openmt2001::scheduler::ClockDomain::Total=>0,
            openmt2001::scheduler::ClockDomain::Normal=>1};
        let bytes=match c {Command::CategoryReady{category}=>words([*category]),
            other=>packet(other,inner).unwrap()};
        (domain,outer,inner,direct,bytes)
    }).collect();
    let chains=openmt2001::building_pool::category_chains(
        &p.city.categories,&p.city.buildings,0x591318).unwrap();
    let category_ids=chains.iter().map(|chain|chain.iter().map(|index|
        word(&p.city.buildings[*index],0)).collect()).collect();
    let mut runtime_prefix=p.city.ai_neighbors.rows.as_flattened().to_vec();
    runtime_prefix.extend(words(p.city.ai_neighbors.owned_properties.map(|v|v as u32)));
    runtime_prefix.extend(words(p.city.ai_neighbors.completed_groups.map(|v|v as u32)));
    runtime_prefix.extend(words([u32::from(p.city.ownership_refresh_counter),
        u32::from(p.population.business_counts.age)]));
    runtime_prefix.extend(words(p.population.business_counts.values.map(|v|v as u32)));
    runtime_prefix.extend(words([u32::from(p.population.market.dirty)]));
    runtime_prefix.extend(words(p.population.market.prices.map(f32::to_bits)));
    runtime_prefix.extend(words([p.population.market.rent.to_bits()]));
    runtime_prefix.extend(&p.roster.scripts.scores.bytes);
    let cut=Cut {
        clock:*p.clock,
        rng:p.random_registry.process.into_iter().chain([
            p.random_registry.head,p.random_registry.tail,p.script_random_state,
            p.credit_random_state,p.particle_random_state,p.allocation_next,
            p.static_rng_tail_next,p.city.creation.initialization_rng.constructor,
            p.city.creation.initialization_rng.movement,p.city.creation.initialization_rng.objective,
            p.city.creation.identity_rng,p.city.creation.preference_rng,p.policy.seed]).collect(),
        build_rng:p.build_random_state,
        utilities:p.utilities.owners.map(u32::from).into_iter()
            .chain(p.utilities.enabled.map(u32::from)).chain(p.utilities.totals.map(f32::to_bits)).collect(),
        queue,scripts,
        // Typed ledgers own accounting at this boundary. Form expected bytes
        // in a LOCAL witness copy; do not publish or edit the actual roster.
        players:p.roster.manager.players.iter().zip(&p.city.ledgers.players).map(|(p,account)|{
            let mut expected=p.bytes.clone();account.write_runtime(&mut expected).unwrap();
            (p.reference,expected,p.auxiliaries.iter().map(|a|(a.reference,a.bytes)).collect())
        }).collect(),
        roads:transport::encode_parts(&p.city.road_network,p.road_traffic,p.transport).unwrap(),
        runtime_prefix,category_ids,category_before:ids(&p.city.categories),category_after:None,
        auction_before:p.city.auction.controller.record,auction_after:None,
        auction_countdown:p.city.auction.presentation.countdown,
        auction_active:p.city.auction.controller.reference!=0,
    };
    CUTS.with(|v|v.borrow_mut().as_mut().unwrap().push(cut));
}
pub(crate) fn after(categories:&[[u8;380];52],auction:&[u8;344]) {
    CUTS.with(|v|if let Some(cuts)=v.borrow_mut().as_mut() {
        let Some(cut)=cuts.last_mut() else{return;};
        assert!(cut.category_after.replace(ids(categories)).is_none(),"second encode at one callback");
        cut.auction_after=Some(*auction);
    });
}
fn saved_queue(save:&SaveFile<'_>)->Vec<(u32,u32,u32,bool,Vec<u8>)> {
    let ext=extension::Extension::parse(save).unwrap().unwrap();
    let mut r=Input::new(ext.required(9).unwrap());let n=r.word().unwrap();
    let rows=(0..n).map(|_|{
        let d=r.word().unwrap();let outer=r.word().unwrap();let inner=r.word().unwrap();
        let direct=r.word().unwrap()!=0;let header=r.blob().unwrap();let bytes=r.blob().unwrap();
        let bytes=if direct {words([word(header,12)])}else{bytes.to_vec()};
        (d,outer,inner,direct,bytes)
    }).collect();r.end().unwrap();rows
}
pub(crate) fn assert_file(c:&Cut,bytes:&[u8],camera:[[u8;196];2]) {
    let save=SaveFile::parse(bytes).unwrap();assert_eq!(save.clock().unwrap(),c.clock);
    assert_eq!(save.cameras().unwrap().map(|v|*v),camera,"both opaque196-byte allocations");
    let ext=extension::Extension::parse(&save).unwrap().unwrap();
    let r=ext.required(3).unwrap();
    assert_eq!((0..c.rng.len()).map(|i|word(r,4*i)).collect::<Vec<_>>(),c.rng);
    assert_eq!(word(ext.required(16).unwrap(),0),c.build_rng);
    assert_eq!((0..12).map(|i|word(ext.required(12).unwrap(),4*i)).collect::<Vec<_>>(),c.utilities);
    assert_eq!(ext.required(11).unwrap(),c.roads);
    assert!(ext.required(10).unwrap().starts_with(&c.runtime_prefix));
    assert_eq!(saved_queue(&save),c.queue);
    let mut scripts=Input::new(ext.required(13).unwrap());assert_eq!(scripts.word().unwrap() as usize,c.scripts.len());
    for (name,body,remaining,argument) in &c.scripts {
        assert_eq!(scripts.blob().unwrap(),name.as_bytes());assert_eq!(scripts.blob().unwrap(),body);
        assert_eq!(scripts.word().unwrap(),*remaining);assert_eq!(scripts.word().unwrap(),*argument as u32);
    }
    scripts.end().unwrap();
    let mut players=Input::new(ext.required(2).unwrap());assert_eq!(players.word().unwrap() as usize,c.players.len());
    players.take(40).unwrap();players.word().unwrap();
    for (reference,bytes,aux) in &c.players {
        assert_eq!(players.word().unwrap(),*reference);assert_eq!(players.blob().unwrap(),bytes);
        assert_eq!(players.word().unwrap() as usize,aux.len());
        for (reference,bytes) in aux {assert_eq!(players.word().unwrap(),*reference);assert_eq!(players.take(48).unwrap(),bytes);}
    }
    players.end().unwrap();
    let post=c.category_after.as_ref().expect("native producer write witness absent");
    assert!(c.category_ids.iter().any(|ids|!ids.is_empty()),"populated IDs must be exercised");
    let table=save.sections.iter().find(|s|s.tag==4).unwrap().payload;
    for (i,expected) in c.category_ids.iter().enumerate() {
        assert!(expected.len()<=24);
        for (j,id) in expected.iter().enumerate(){assert_eq!(post[i][j],*id);}
        for j in expected.len()..24{assert_eq!(post[i][j],c.category_before[i][j]);}
        for j in 0..24 {assert_eq!(word(table,0x9600+i*0x8c+12+4*j),post[i][j]);}
    }
    let mut expected=c.auction_before;
    // The writer's source flag gate, not controller existence alone.
    let native_auction=c.auction_active && (expected[0x7c]!=0 || expected[0x7d]!=0);
    if native_auction {put(&mut expected,0x80,c.auction_countdown);}
    assert_eq!(c.auction_after,Some(expected));
    if c.auction_active {
        assert_eq!(save.auction().unwrap(),native_auction.then_some(&expected));
        let a=ext.required(14).unwrap();assert_eq!(word(a,20),c.auction_countdown);
        assert_eq!(&a[28..28+344],&expected);
    } else {assert!(save.auction().unwrap().is_none());assert!(!ext.records.contains_key(&14));}
}
pub(crate) fn assert_loaded(c:&Cut,s:&mut Session<'_>) {
    let restored=RESTORED.with(|v|{
        let mut observed=v.borrow_mut();let rows=observed.as_mut().unwrap();
        assert_eq!(rows.len(),1,"exactly one actual post-restore/pre-preview witness required");
        rows.pop().unwrap()
    });
    assert!(restored==c.roads,
        "full record11 at actual post-restore/pre-preview boundary: actual_bytes={} saved_bytes={} first_difference={:?}",
        restored.len(),c.roads.len(),restored.iter().zip(&c.roads).position(|(a,b)|a!=b));
    // Remaining checks intentionally inspect the session returned by preview.
    // inspect_fresh projects road auxiliaries and advances aircraft frame state;
    // record11 retention is therefore witnessed above, before those effects.
    assert_eq!(s.world.clock,c.clock,"load must retain transient absolute hour");
    assert_eq!(s.allocation_next,c.rng[8]);assert_eq!(s.build_random_state,c.build_rng);
    assert_eq!(s.random_registry.process.as_slice(),&c.rng[..3]);
    assert_eq!(s.script_random_state,c.rng[5]);assert_eq!(s.credit_random_state,c.rng[6]);
    assert_eq!(s.particle_random_state,c.rng[7]);
    let utilities=s.hooks.utilities.owners.map(u32::from).into_iter()
        .chain(s.hooks.utilities.enabled.map(u32::from)).chain(s.hooks.utilities.totals.map(f32::to_bits)).collect::<Vec<_>>();
    assert_eq!(utilities,c.utilities);
    let scripts=s.scripts.events.pending().iter().map(|e|{
        let (name,body)=s.scripts.runtime.event_reference_identity(e.reference).unwrap().unwrap();
        (name,body,e.remaining,e.argument)
    }).collect::<Vec<_>>();assert_eq!(scripts,c.scripts,"record13 replaces once, no native era requeue");
    for (p,(reference,bytes,aux)) in s.roster.manager.players.iter().zip(&c.players) {
        assert_eq!(p.reference,*reference);assert_eq!(p.bytes,*bytes);
        assert_eq!(p.auxiliaries.iter().map(|a|(a.reference,a.bytes)).collect::<Vec<_>>(),*aux);
    }
}
pub(crate) fn assert_pair(a:&mut Session<'_>,b:&mut Session<'_>) {
    assert_eq!(a.world.clock,b.world.clock);
    assert_eq!(a.random_registry.process,b.random_registry.process);
    assert_eq!(a.random_registry.head,b.random_registry.head);assert_eq!(a.random_registry.tail,b.random_registry.tail);
    assert_eq!([a.script_random_state,a.credit_random_state,a.particle_random_state,a.build_random_state,
        a.allocation_next,a.static_rng_tail_next],[b.script_random_state,b.credit_random_state,
        b.particle_random_state,b.build_random_state,b.allocation_next,b.static_rng_tail_next]);
    assert_eq!(a.event_cards.cards,b.event_cards.cards);assert_eq!(a.event_cards.used,b.event_cards.used);
    assert_eq!(a.event_cards.rng,b.event_cards.rng);assert_eq!(a.event_cards.remaining,b.event_cards.remaining);
    assert_eq!(a.roster.profiles,b.roster.profiles);
    assert_eq!(a.roster.manager.players.len(),b.roster.manager.players.len());
    for (a,b) in a.roster.manager.players.iter().zip(&b.roster.manager.players) {
        assert_eq!(a.bytes,b.bytes);assert_eq!(a.auxiliaries.len(),b.auxiliaries.len());
        for (a,b) in a.auxiliaries.iter().zip(&b.auxiliaries){assert_eq!(a.reference,b.reference);assert_eq!(a.bytes,b.bytes);}
    }
    assert_eq!(a.world.pending_world.to_bytes(),b.world.pending_world.to_bytes());
    assert_eq!(transport::encode(a).unwrap(),transport::encode(b).unwrap());
    fn queue(s:&Session<'_>)->Vec<(u32,u32,u32,bool,Vec<u8>)> {
        s.live_commands.snapshot().unwrap().into_iter().map(|(domain,outer,inner,direct,c)|{
            let domain=match domain{openmt2001::scheduler::ClockDomain::Total=>0,openmt2001::scheduler::ClockDomain::Normal=>1};
            let p=match c{Command::CategoryReady{category}=>words([*category]),c=>packet(c,inner).unwrap()};
            (domain,outer,inner,direct,p)
        }).collect()
    }
    assert_eq!(queue(a),queue(b));
    fn scripts(s:&mut Session<'_>)->Vec<(String,Vec<u8>,u32,i32)> {
        s.scripts.events.pending().iter().map(|e|{
            let (name,body)=s.scripts.runtime.event_reference_identity(e.reference).unwrap().unwrap();
            (name,body,e.remaining,e.argument)
        }).collect()
    }
    assert_eq!(scripts(a),scripts(b));
}
