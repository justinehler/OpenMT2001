//! The ONE missing 68b4c0 eleven-slot chat object; unrelated to rival notices.
//! No Default and no frame constructor. Untouched bytes come from the actual
//! allocation/source cells; constructor/reset only write the two control words.
pub type Result<T> = std::result::Result<T,String>;
pub const SLOT_BYTES:usize=264;
pub const SLOTS:usize=11;
#[derive(Debug,PartialEq,Eq)]
pub struct Ring {slots:[[u8;SLOT_BYTES];SLOTS],head:i32,tail:i32}
impl Ring {
    ///452040. Takes ownership of actual untouched allocation backing once.
    pub fn construct452040(untouched:[[u8;SLOT_BYTES];SLOTS])->Self{
        Self{slots:untouched,tail:0,head:0}
    }
    ///Transfer actual live cells once; never a per-frame snapshot/import loop.
    pub fn from_source(slots:[[u8;SLOT_BYTES];SLOTS],head:i32,tail:i32)->Result<Self>{
        if !(0..11).contains(&head)||!(0..11).contains(&tail){
            return Err("68b4c0 invalid ring head/tail; native arbitrary memory domain unsupported".into())
        }
        Ok(Self{slots,head,tail})
    }
    pub fn head(&self)->i32{self.head}
    pub fn tail(&self)->i32{self.tail}
    pub fn slots(&self)->&[[u8;SLOT_BYTES];SLOTS]{&self.slots}
    ///452060: retained payload/countdowns are NOT cleared.
    pub fn reset452060(&mut self){self.tail=0;self.head=0;}
    ///452070 bounded string domain: source strlen+NUL fits its 256-byte text
    ///region. Longer native strings/overlap are explicit unsupported admission,
    ///before writes; never truncate or zero-fill the remaining text region.
    pub fn append452070(&mut self,color:u32,nul_terminated:&[u8])->Result<()>{
        let length=nul_terminated.iter().position(|&b|b==0)
            .ok_or("452070 requires a source NUL-terminated string")?+1;
        if length>256{return Err("452070 string exceeds bounded native text domain".into())}
        let slot=&mut self.slots[self.tail as usize];
        slot[256..260].copy_from_slice(&1000u32.to_le_bytes());
        slot[..length].copy_from_slice(&nul_terminated[..length]);
        slot[260..264].copy_from_slice(&color.to_le_bytes());
        self.tail=(self.tail+1)%11;
        if self.tail==self.head{self.head=(self.head+1)%11;}
        Ok(())
    }
    ///452110: signed occupancy, ONE initial ESI/slot. Even when head advances,
    ///subsequent decrements hit this SAME DWORD. This is not a per-slot expiry.
    pub fn tick452110(&mut self){
        let mut occupancy=self.tail-self.head;
        if occupancy<0{occupancy+=11;}
        if occupancy<=0{return}
        let initial=self.head as usize;
        for _ in 0..occupancy{
            let slot=&mut self.slots[initial];
            let left=u32::from_le_bytes(slot[256..260].try_into().unwrap()).wrapping_sub(1);
            slot[256..260].copy_from_slice(&left.to_le_bytes());
            if left==0{self.head=(self.head+1)%11;}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cells(counter:u32)->[[u8;264];11]{let mut s=[[0x6d;264];11];for v in &mut s{v[256..260].copy_from_slice(&counter.to_le_bytes());}s}
    fn counter(r:&Ring,i:usize)->u32{u32::from_le_bytes(r.slots()[i][256..260].try_into().unwrap())}
    #[test] fn repeated_same_slot_after_fresh_head_advance(){
        let mut r=Ring::from_source(cells(1),9,1).unwrap();r.tick452110();
        assert_eq!(r.head(),10);assert_eq!(counter(&r,9),u32::MAX-1);
        assert_eq!(counter(&r,10),1);assert_eq!(counter(&r,0),1);
    }
    #[test] fn empty_preserves_all_bytes_and_reset_keeps_payload(){
        let mut r=Ring::from_source(cells(7),6,6).unwrap();let old=*r.slots();
        r.tick452110();assert_eq!(*r.slots(),old);r.reset452060();
        assert_eq!((r.head(),r.tail()),(0,0));assert_eq!(*r.slots(),old);
    }
    #[test] fn append_wrap_collision_and_exact_retained_bytes(){
        let mut r=Ring::from_source(cells(7),0,10).unwrap();r.append452070(0x12345678,b"hi\0ignored").unwrap();
        assert_eq!((r.head(),r.tail()),(1,0));assert_eq!(&r.slots()[10][..4],b"hi\0m");
        assert_eq!(counter(&r,10),1000);assert_eq!(&r.slots()[10][260..],&0x12345678u32.to_le_bytes());
        assert_eq!(counter(&r,0),7);
    }
    #[test] fn constructor_preserves_backing_and_invalid_import_or_string_rejects(){
        let s=cells(17);let mut r=Ring::construct452040(s);assert_eq!(*r.slots(),s);
        assert!(Ring::from_source(s,-1,0).is_err());assert!(Ring::from_source(s,0,11).is_err());
        assert!(r.append452070(0,b"no nul").is_err());let mut long=vec![b'x';257];long.push(0);assert!(r.append452070(0,&long).is_err());
        assert_eq!(*r.slots(),s);assert_eq!((r.head(),r.tail()),(0,0));
    }
}
