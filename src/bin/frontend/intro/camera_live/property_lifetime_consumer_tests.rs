//! Regressions over real owned property and graph entry points,
//! installed Pool, authored rows/model binding. No live manager/lifecycle proof.
use super::{Pool,PavementModel,ResetTransform};
use super::super::{main_panel::{self,Access,Prospective,regression_tests},main_model,
    property_child,owner::Owner,ui_graph::{self,Panel,PanelId,UiGraphState,SelectionAccess},
    navigation::{self,Group},ui_selection::{Selected,SelectionState}};
use openmt2001::camera::bank::{Bank,Capture};

struct Installed {
    owner:Owner,rows:[[u8;380];52],model:PavementModel,reset:ResetTransform,
    old:Selected,
    _assets:crate::selection::Assets, // sole strong definition owner outlives installed aliases
}
impl Installed {
    fn new()->Self {
        let definition=[0;116]; // Explicit authored manager contents only.
        let assets=crate::selection::Assets::descriptor_test_assets(Vec::new(),definition);
        let model=PavementModel::bind(&assets).unwrap();
        let rows=[[0;380];52];let reset=ResetTransform([0x10203040;12]);
        let pool=Pool::from_rows(&rows,1,&model,&reset).unwrap();
        let old=pool.selected(pool.lookup(0).unwrap().unwrap()).unwrap();
        let mut owner=Owner::new(Bank::from_capture(Capture::from_records([[0;196];2])),0x91);
        owner.install_ui_selection(SelectionState::installed(
            Some(old),Some(old),Some(old),None,Some(0x703000),None,12,13)).unwrap();
        owner.install_property_descriptors(pool).unwrap();
        Self{owner,rows,model,reset,old,_assets:assets}
    }
    fn rebuild(&mut self)->Selected {
        self.owner.property_descriptors_mut().unwrap().rebuild_rows(&self.rows,1,&self.model,&self.reset).unwrap();
        let pool=self.owner.property_descriptors().unwrap();
        let fresh=pool.selected(pool.lookup(0).unwrap().unwrap()).unwrap();
        assert_ne!(self.old.key,fresh.key);
        assert_eq!((self.old.kind,self.old.category),(fresh.kind,fresh.category));
        fresh
    }
}
fn owner_cells(owner:&Owner)->String {
    format!("{:?} dirty={}",owner.ui_selection().unwrap(),owner.kind_dirty())
}
fn assert_stale(error:String){assert!(error.contains("stale property descriptor allocation"),"{error}");}
fn rejected_property(p:&mut Prospective,owner:&mut Owner,
    operation:impl FnOnce(&mut Prospective,&mut Owner)->Result<(),String>) {
    let before=format!("{p:?}");let raw=owner.capture();let cells=owner_cells(owner);
    assert_stale(operation(p,owner).unwrap_err());
    assert_eq!(format!("{p:?}"),before);assert_eq!(owner.capture(),raw);
    assert_eq!(owner_cells(owner),cells);
}

#[test]
fn real_property_direct_entries_reject_stale_current_before_any_owned_write() {
    let mut f=Installed::new();let fresh=f.rebuild();
    // Independently stale current; other cells are fresh (same kind/category).
    {let s=f.owner.ui_selection_mut().unwrap();s.published=Some(fresh);s.transient=Some(fresh);}
    let mut p=regression_tests::fixture();p.main.mode=1;
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::activate(p,o));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::enter(p,o));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::exit(p,o));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::reset_no_selection(p,o));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::select_mode(p,o,1));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::select_report(p,o,0));
    rejected_property(&mut p,&mut f.owner,|p,o|main_model::refresh(p,o));
    rejected_property(&mut p,&mut f.owner,|p,o|main_panel::switch(p,1,o));
}
#[test]
fn raw_prospective_and_transient_copies_cannot_bypass_property_consumer_validation() {
    let mut f=Installed::new();let fresh=f.rebuild();
    // Prospective is intentionally not a lifetime token. Every field-reading
    // owned entry must validate its own source even if actual current is fresh.
    {let s=f.owner.ui_selection_mut().unwrap();s.current=Some(fresh);s.published=Some(fresh);}
    let mut p=regression_tests::fixture();p.current=f.owner.ui_selection().unwrap().transient;
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::activate_owned(p,o));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::enter_owned(p,o));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::exit_owned(p,o));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::select_mode_owned(p,o,1));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::select_report_owned(p,o,0));
    rejected_property(&mut p,&mut f.owner,|p,o|main_model::refresh_owned(p,o));
    let before=format!("{p:?}");let cells=owner_cells(&f.owner);let raw=f.owner.capture();
    assert_stale(main_panel::preflight(&p,1,&f.owner,p.current).unwrap_err());
    assert_stale(main_model::check_for(&p,1,&f.owner,p.current).unwrap_err());
    assert_eq!(format!("{p:?}"),before);assert_eq!(owner_cells(&f.owner),cells);assert_eq!(f.owner.capture(),raw);
}

/// Actual graph/Main/navigation owners, no fabricated Inspection/GPU/audio.
/// Unrepresented family checks PANIC rather than granting a successful route.
/// Stale admission must reject before any such downstream call is attempted.
struct GraphOwner {fonts:super::super::case_zero_text::Fonts,graph:UiGraphState,navigation:navigation::State}
impl GraphOwner {
    fn new()->Self {Self{fonts:regression_tests::fonts(),graph:ui_graph::refresh_fixture([0;600]),
        navigation:navigation::State::new(0,7,2,[0;3],Some(ui_graph::consumers()))}}
    fn snapshot(&self)->String {
        let g=&self.graph;let n=&self.navigation;
        format!("fonts={:?} main={:?} panels={:?} groups={:?} retained={:?} world={:?} report={:?} \
            human={} blocked={} focus={:?} compass={:?} speed={:?} upper={:?} nav={:?}",
            self.fonts,g.main,g.panels,g.groups,g.retained_c,g.world,g.report_inputs,g.human_enabled,g.dispatch_blocked,
            g.focus_child,(g.compass_literal,g.compass_identifier),(g.speed_disabled,g.speed_blink),
            g.upper.as_ref().map(|u|(&u.geometry,u.selected,u.active,u.enabled,u.pointer,std::sync::Arc::as_ptr(&u.input))),
            (n.mode(),n.shadow(),n.previous(),n.saved_visibility(),n.compass_identifier()))
    }
}
impl Access for GraphOwner {
    fn fonts(&self)->Result<&super::super::case_zero_text::Fonts,String>{Ok(&self.fonts)}
    fn text_parts_mut(&mut self)->Result<(&mut main_panel::State,&mut super::super::case_zero_text::Fonts),String>{Ok((&mut self.graph.main,&mut self.fonts))}
    fn main(&self)->Result<&main_panel::State,String>{Ok(&self.graph.main)}
    fn main_mut(&mut self)->Result<&mut main_panel::State,String>{Ok(&mut self.graph.main)}
    fn panel(&self)->Result<&Panel,String>{self.graph.panel(PanelId::A)}
    fn panel_mut(&mut self)->Result<&mut Panel,String>{self.graph.panel_mut(PanelId::A)}
}
impl SelectionAccess for GraphOwner {
    fn selection_graph(&self)->Result<&UiGraphState,String>{Ok(&self.graph)}
    fn navigation_state(&self)->Result<&navigation::State,String>{Ok(&self.navigation)}
    fn tutorial_session(&self)->Result<bool,String>{panic!("stale admission reached tutorial consumer")}
    fn check_group(&self,_:Group)->Result<(),String>{panic!("stale admission reached group consumer")}
    fn check_panel(&self,_:PanelId,_:bool,_:&Owner)->Result<(),String>{panic!("stale admission reached panel consumer")}
    fn check_family(&self,_:PanelId,_:bool,_:&Owner,_:Option<Selected>)->Result<(),String>{panic!("stale admission reached generic family consumer")}
    fn check_upper_tree(&self)->Result<(),String>{panic!("stale admission reached upper tree")}
    fn check_shown_phase11(&self,_:PanelId,_:&Owner,_:bool)->Result<(),String>{panic!("stale admission reached phase11 consumer")}
}
fn rejected_graph(g:&mut GraphOwner,o:&mut Owner,
    operation:impl FnOnce(&mut GraphOwner,&mut Owner)->Result<(),String>) {
    let before=g.snapshot();let cells=owner_cells(o);let raw=o.capture();
    assert_stale(operation(g,o).unwrap_err());
    assert_eq!(g.snapshot(),before);assert_eq!(owner_cells(o),cells);assert_eq!(o.capture(),raw);
}
#[test]
fn real_restore_rejects_stale_published_even_with_fresh_current() {
    let mut f=Installed::new();let fresh=f.rebuild();
    {let s=f.owner.ui_selection_mut().unwrap();s.current=Some(fresh);s.transient=Some(fresh);}
    let mut g=GraphOwner::new();g.graph.retained_c=Some(fresh);
    rejected_graph(&mut g,&mut f.owner,|g,o|ui_graph::restore_picked_panel(g,o));
    assert_eq!(f.owner.ui_selection().unwrap().published,Some(f.old));
    assert_eq!(g.graph.main.mode,0);
}
#[test]
fn real_navigation_preflight_rejects_separately_stale_current_before_downstream_consumers() {
    for camera_kind in [4,12] {
        let mut f=Installed::new();let fresh=f.rebuild();f.owner.set_kind(camera_kind);
        {let s=f.owner.ui_selection_mut().unwrap();s.published=Some(fresh);s.transient=Some(fresh);}
        let mut g=GraphOwner::new();g.graph.retained_c=Some(fresh);
        g.graph.panel_mut(PanelId::C).unwrap().requested=0;
        rejected_graph(&mut g,&mut f.owner,|g,o|ui_graph::preflight(g,o));
    }
}
#[test]
fn real_navigation_preflight_rejects_stale_retained_c_before_main_or_navigation() {
    let mut f=Installed::new();let fresh=f.rebuild();f.owner.set_kind(4);
    {let s=f.owner.ui_selection_mut().unwrap();s.current=Some(fresh);s.published=Some(fresh);s.transient=Some(fresh);}
    let mut g=GraphOwner::new();g.graph.retained_c=Some(f.old);
    g.graph.panel_mut(PanelId::C).unwrap().requested=1;
    g.navigation=navigation::State::new(3,3,0,[1,0,1],Some(ui_graph::consumers()));
    rejected_graph(&mut g,&mut f.owner,|g,o|ui_graph::preflight(g,o));
    assert_eq!(g.graph.retained_c,Some(f.old));assert_eq!(f.owner.ui_selection().unwrap().current,Some(fresh));
}
#[test]
fn successful_property_and_restore_keep_identity_and_ignore_unconsumed_stale_cells() {
    let mut f=Installed::new();let fresh=f.rebuild();
    // Published/transient remain stale. A direct property operation consumes
    // only current; unrelated stale cells must neither block nor be repaired.
    f.owner.ui_selection_mut().unwrap().current=Some(fresh);
    let mut p=regression_tests::fixture();p.main.mode=1;
    property_child::activate(&mut p,&mut f.owner).unwrap();
    assert_eq!(p.main.property.selected_category,0);assert_eq!(p.main.notice_caption,Some(100));
    assert_eq!(f.owner.ui_selection().unwrap().published,Some(f.old));
    assert_eq!(f.owner.ui_selection().unwrap().transient,Some(f.old));
    // Restore now consumes fresh published/current, never unrelated retained C
    // or transient. The exact actual key is retained after Main's transaction.
    f.owner.ui_selection_mut().unwrap().published=Some(fresh);
    let mut g=GraphOwner::new();g.graph.retained_c=Some(f.old);
    ui_graph::restore_picked_panel(&mut g,&mut f.owner).unwrap();
    assert_eq!(g.graph.main.mode,1);assert_eq!(g.graph.main.property.selected_category,0);
    assert_eq!(f.owner.ui_selection().unwrap().current,Some(fresh));
    assert_eq!(f.owner.ui_selection().unwrap().published,Some(fresh));
    assert_eq!(g.graph.retained_c,Some(f.old));assert_eq!(f.owner.ui_selection().unwrap().transient,Some(f.old));
    assert_eq!(f.owner.resolve_ui_selected(Some(fresh)).unwrap().unwrap().source(),fresh);
}

#[test]
fn real_generic_visibility_rejects_stale_current_before_family_preflight_or_panel_writes() {
    let mut f=Installed::new();let fresh=f.rebuild();
    {let s=f.owner.ui_selection_mut().unwrap();s.published=Some(fresh);s.transient=Some(fresh);}
    let mut g=GraphOwner::new();g.graph.retained_c=Some(fresh);
    rejected_graph(&mut g,&mut f.owner,|g,o|ui_graph::check_visibility(g,PanelId::B,true,o));
    // Hide takes the same generic family path when the actual B is visible.
    g.graph.panel_mut(PanelId::B).unwrap().requested=1;
    rejected_graph(&mut g,&mut f.owner,|g,o|ui_graph::check_visibility(g,PanelId::B,false,o));
}
#[test]
fn real_restore_rejects_stale_current_before_main_mutation_with_fresh_published() {
    let mut f=Installed::new();let fresh=f.rebuild();
    {let s=f.owner.ui_selection_mut().unwrap();s.published=Some(fresh);s.transient=Some(fresh);}
    let mut g=GraphOwner::new();g.graph.retained_c=Some(fresh);
    rejected_graph(&mut g,&mut f.owner,|g,o|ui_graph::restore_picked_panel(g,o));
}
#[test]
fn null_current_and_pointer_only_c_hide_do_not_resolve_or_repair_unrelated_cells() {
    let mut f=Installed::new();f.rebuild();
    f.owner.ui_selection_mut().unwrap().current=None;
    let mut p=regression_tests::fixture();p.main.mode=1;
    property_child::activate(&mut p,&mut f.owner).unwrap();
    assert_eq!(p.main.property.selected_category,u32::MAX);
    assert_eq!(f.owner.ui_selection().unwrap().published,Some(f.old));
    assert_eq!(f.owner.ui_selection().unwrap().transient,Some(f.old));
    // Same pointer-only helper called by C hide: copying a retained facade
    // itself does not dereference descriptor fields or pretend to repair it.
    f.owner.ui_selection_mut().unwrap().strategic_hide(Some(f.old));
    assert_eq!(f.owner.ui_selection().unwrap().current,Some(f.old));
    assert_eq!(f.owner.ui_selection().unwrap().published,Some(f.old));
    assert_eq!(f.owner.ui_selection().unwrap().transient,Some(f.old));
    rejected_property(&mut p,&mut f.owner,|p,o|property_child::activate(p,o));
}
