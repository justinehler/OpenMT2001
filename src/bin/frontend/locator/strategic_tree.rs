//! Actual43e8b0 depth-first phase11 dispatch, retained under Locator State.
//! Leaf bodies below are finite original11 branches, not successful fallbacks.
use std::collections::{BTreeMap,BTreeSet};
#[derive(Clone,Debug)]
pub enum Handler {
    ///Exact43fb70 RET12, including the ordinary base/caption leaf.
    Ret43fb70,
    ///430690 jump-table index7=3 ->430769. Selector431c90 index6=3 ->431da5.
    Report430690,Selector431c90,
    ///42c9e0 button /443360 action both copy previous pulse then zero pulse.
    Button{pulse:u8,previous:u8},Action{pulse:u8,previous:u8},
    ///42f2b0, before its blocked/event11 admission tests.
    Image{changed:u8,previous:u8},
    ///42efe0 clears changed, then event!=12 returns without notification.
    Slider{changed:u8},
    ///Deliberately explicit. An unknown actual vtable never becomes Ret43fb70.
    Unported(u32),
}
#[derive(Clone,Debug)]
pub struct Node {pub shown:u8,pub first:Option<u32>,pub next:Option<u32>,pub handler:Handler}
#[derive(Clone,Debug)]
pub struct Tree {pub first:Option<u32>,pub nodes:BTreeMap<u32,Node>}
impl Tree {
    ///Use real factory topology and exact leaf handler identities. No Default,
    ///no omission of hidden controls/scrollbars, no guessed registration order.
    pub fn installed(first:Option<u32>,nodes:BTreeMap<u32,Node>)->Result<Self,String>{
        if first.is_none()||nodes.is_empty(){return Err("C actual layout births nonempty child tree".into());}
        let s=Self{first,nodes};
        //Birth validates real topology, not future handlers hidden by source.
        fn shape(tree:&Tree,mut key:Option<u32>,seen:&mut BTreeSet<u32>)->Result<(),String>{
            while let Some(id)=key{if !seen.insert(id){return Err("C factory child topology cycles/shares a node".into());}
                let n=tree.nodes.get(&id).ok_or("C factory child topology unresolved")?;
                shape(tree,n.first,seen)?;key=n.next;}Ok(())
        }
        shape(&s,s.first,&mut BTreeSet::new())?;Ok(s)
    }
    pub fn check(&self)->Result<(),String>{let mut copy=self.clone();copy.dispatch11()}
    pub fn dispatch11(&mut self)->Result<(),String>{
        fn walk(tree:&mut Tree,mut key:Option<u32>,seen:&mut BTreeSet<u32>)->Result<(),String>{
            while let Some(id)=key{
                if !seen.insert(id){return Err("C child tree cycle/shared node".into());}
                let node=tree.nodes.get(&id).ok_or("C actual child identity absent")?;
                if node.shown!=0{
                    let child=node.first;walk(tree,child,seen)?;
                    //Reread visibility after descendants, as43e8b0 does.
                    let node=tree.nodes.get_mut(&id).ok_or("C child disappeared after descendants")?;
                    if node.shown!=0{match &mut node.handler{
                        Handler::Ret43fb70|Handler::Report430690|Handler::Selector431c90=>{},
                        Handler::Button{pulse,previous}|Handler::Action{pulse,previous}=>{*previous=*pulse;*pulse=0;},
                        Handler::Image{changed,previous}=>{*previous=*changed;*changed=0;},
                        Handler::Slider{changed}=>*changed=0,
                        Handler::Unported(va)=>return Err(format!("C reached unported phase11 child handler {va:08x}")),
                    }}
                }
                //Hidden parent suppresses descendants, never its following sibling.
                key=tree.nodes.get(&id).ok_or("C child disappeared before sibling load")?.next;
            }Ok(())
        }
        let first=self.first;walk(self,first,&mut BTreeSet::new())
    }
}
///Exact4342c0 comparisons. Raw logical pointer cells survive leaving the window;
///the producer must supply them and93c60d. Option<display-hover> is not equivalent.
#[derive(Clone,Debug)]
pub struct Geometry {
    pub open:[f32;4],pub closed:[f32;4],pub owned:[f32;4],
    pub pointer:[i32;2],pub pointer_blocked:u8,
}
impl Geometry {
    pub fn contains(&self,bounds:[f32;4],offset:[f32;2])->Result<bool,String>{
        //4342c0 returns on93c60d before any floating operand is consumed.
        //This API requires a bound, nonnull descriptor offset. Its absence
        //must be handled by the real factory/caller, never a zero-offset guess.
        if self.pointer_blocked!=0{return Ok(false);}
        //Explicit engine domain: finite f32 operands, nearest-even PC64 Sum.
        //Reject all nonfinite inputs, even if a later native comparison would
        //not be reached. No unordered/FPU-exception/control-word claim.
        if bounds.iter().chain(offset.iter()).any(|v|!v.is_finite()){
            return Err("C actual bounds outside admitted finite4342c0 domain".into());}
        use openmt2001::x87::Sum;
        use std::cmp::Ordering::{Less,Greater};
        let [x,y]=self.pointer.map(Sum::from_i32);let [l,t,r,b]=bounds;
        //Retain each shifted sum at PC64; never spill through f32/f64.
        //Keep the recovered right, left, bottom, top comparison order.
        let mut right=Sum::from_f32(r)?;right.add_f32(offset[0])?;
        if x.compare(right)!=Less{return Ok(false);}
        let mut left=Sum::from_f32(offset[0])?;left.add_f32(l)?;
        if x.compare(left)!=Greater{return Ok(false);}
        let mut bottom=Sum::from_f32(b)?;bottom.add_f32(offset[1])?;
        if y.compare(bottom)==Greater{return Ok(false);}
        let mut top=Sum::from_f32(t)?;top.add_f32(offset[1])?;
        Ok(y.compare(top)!=Less)
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::Geometry;
    fn geometry(pointer:[i32;2])->Geometry {
        Geometry{open:[0.;4],closed:[0.;4],owned:[0.;4],pointer,pointer_blocked:0}
    }
    #[test]
    fn strict_x_inclusive_y_and_signed_coordinates() {
        let bounds=[10.,20.,30.,40.];let offset=[3.,5.];
        for (p,expected) in [([13,35],false),([33,35],false),([14,35],true),
            ([32,35],true),([20,25],true),([20,45],true),([20,24],false),
            ([20,46],false),([13,25],false),([33,45],false)] {
            assert_eq!(geometry(p).contains(bounds,offset).unwrap(),expected,"{p:?}");
        }
        for (p,expected) in [([-33,-25],false),([-13,-25],false),
            ([-32,-45],true),([-14,-25],true),([-20,-46],false)] {
            assert_eq!(geometry(p).contains([-30.,-40.,-10.,-20.],[-3.,-5.]).unwrap(),expected);
        }
        assert!(!geometry([0,0]).contains([0.,-1.,0.,1.],[0.,0.]).unwrap());
    }
    #[test]
    fn pc64_tiny_offsets_survive_each_edge_comparison() {
        //2^-60 is an exact f32. At magnitude1, binary64 loses it; PC64
        //retains it. These assertions fail if an edge sum spills to f32/f64.
        let tiny=f32::from_bits(0x2180_0000);
        let rect=[-1.,-1.,1.,1.];
        assert!(geometry([1,0]).contains(rect,[tiny,0.]).unwrap());
        assert!(geometry([-1,0]).contains(rect,[-tiny,0.]).unwrap());
        assert!(!geometry([1,0]).contains(rect,[-tiny,0.]).unwrap());
        assert!(!geometry([-1,0]).contains(rect,[tiny,0.]).unwrap());
        //Inclusive equality must not hide a discarded tiny Y displacement.
        assert!(!geometry([0,1]).contains(rect,[0.,-tiny]).unwrap());
        assert!(!geometry([0,-1]).contains(rect,[0.,tiny]).unwrap());
        assert!(geometry([0,1]).contains(rect,[0.,tiny]).unwrap());
        assert!(geometry([0,-1]).contains(rect,[0.,-tiny]).unwrap());
        assert!(!geometry([1,0]).contains(rect,[0.,0.]).unwrap());
        assert!(!geometry([-1,0]).contains(rect,[0.,0.]).unwrap());
    }
    #[test]
    fn block_precedes_floats_and_nonfinite_policy_is_checked() {
        let mut g=geometry([0,0]);g.pointer_blocked=1;
        assert!(!g.contains([f32::NAN;4],[f32::INFINITY;2]).unwrap());
        g.pointer_blocked=0;
        for bad in [f32::NAN,f32::INFINITY,f32::NEG_INFINITY] {
            for i in 0..4 {let mut b=[-1.,-1.,1.,1.];b[i]=bad;
                assert!(g.contains(b,[0.,0.]).is_err());}
            for i in 0..2 {let mut o=[0.,0.];o[i]=bad;
                assert!(g.contains([-1.,-1.,1.,1.],o).is_err());}
        }
        //Signed32 coordinates are FILD values; no intermediate f32 rounding.
        assert!(geometry([i32::MAX,0]).contains([2147483520.,-1.,2147483648.,1.],[0.,0.]).unwrap());
        assert!(!geometry([i32::MIN,0]).contains([-2147483648.,-1.,0.,1.],[0.,0.]).unwrap());
    }
}
