//! Native4c0f9f ordered GenericTextures640 products. Source labels are
//! provenance, never renderer handles or modern allocation addresses.
use openmt2001::{archive::ArchiveIndex,pe::PeImage,texture::TextureSet};
use std::ops::Range;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ImageId(usize);
#[derive(Debug)]
pub struct Record {
    pub ordinal:usize,pub registration:u32,pub range:Range<usize>,
    pub dimensions:[u32;2],pub header:[u32;14],pub scale:Option<f32>,
}
pub struct Images {
    pub(super) construction_identity:super::main_construction::ResourceIdentity,
    pub path:String,pub archive_range:Range<usize>,pub metadata:[u32;13],
    pub ui_dimensions:[u32;2],pub records:Vec<Record>,bytes:Vec<u8>,
}
impl Images {
    pub fn load(executable:&[u8],index:&ArchiveIndex,archive:&[u8],ui:[u32;2])->Result<Self,String>{
        if ui!=[640,480]{return Err("4c0e70: only actual GenericTextures640 producer admitted".into())}
        let pe=PeImage::parse(executable)?;let raw=pe.at(0x54c8f8,128)?;
        let n=raw.iter().position(|&v|v==0).ok_or("4c0e70 selected image path unterminated")?;
        let path=std::str::from_utf8(&raw[..n]).map_err(|_|"4c0e70 selected image path not ASCII")?.to_owned();
        let e=index.find(&path).ok_or_else(||format!("4c0e70: missing actual selected collection {path}"))?;
        let first=usize::try_from(e.offset).map_err(|_|"4c0e70 offset")?;
        let last=usize::try_from(e.offset.checked_add(e.length).ok_or("4c0e70 archive span overflow")?).map_err(|_|"4c0e70 end")?;
        let bytes=archive.get(first..last).ok_or("4c0e70 selected collection outside archive")?.to_vec();
        let textures=TextureSet::parse(&bytes).map_err(|e|format!("4c0e70 {path}: {e}"))?;
        let mut records=Vec::new();let mut at=4usize;
        for (ordinal,t) in textures.textures.iter().enumerate(){
            let end=at.checked_add(56).and_then(|v|v.checked_add(t.texels.len())).ok_or("4c0f9f image record overflow")?;
            let registration=0x68ae28u32.checked_add(u32::try_from(ordinal).map_err(|_|"4c0f9f ordinal")?.checked_mul(4).ok_or("4c0f9f ordinal overflow")?).ok_or("4c0f9f table overflow")?;
            // Unreached registration internals have no invented scale preimage.
            records.push(Record{ordinal,registration,range:at..end,dimensions:[t.width,t.height],header:t.header,scale:None});at=end;
        }
        if at!=bytes.len(){return Err("4c0f9f unaccounted selected collection tail".into())}
        Ok(Self{construction_identity:super::main_construction::ResourceIdentity::new(),path,archive_range:first..last,metadata:e.metadata,ui_dimensions:ui,records,bytes})
    }
    pub fn textures(&self)->Result<TextureSet<'_>,String>{TextureSet::parse(&self.bytes).map_err(|e|e.to_string())}
    pub fn id(&self,label:u32)->Result<ImageId,String>{
        let n=label.checked_sub(0x68ae28).filter(|v|v%4==0).ok_or_else(||format!("4c0f9f: image label{label:x} outside ordered table"))?/4;
        let n=n as usize;self.records.get(n).ok_or_else(||format!("4c0f9f: actual image ordinal{n} missing"))?;Ok(ImageId(n))
    }
    pub fn record(&self,id:ImageId)->Result<&Record,String>{self.records.get(id.0).ok_or_else(||"image identity leaves owner".into())}
    /// Source store scale1 precedes each dimension call. Integer native LEA
    /// arithmetic wraps before unsigned DIV; no float texture-size shortcut.
    pub fn dimension(&mut self,label:u32,axis:usize)->Result<u32,String>{
        if axis>1{return Err("image dimension axis".into())}
        let id=self.id(label)?;let r=&mut self.records[id.0];r.scale=Some(1.);
        let divisor=self.ui_dimensions[axis];if divisor==0{return Err("432ff0/433010: actual display divisor zero".into())}
        Ok(r.dimensions[axis].wrapping_mul([640,480][axis])/divisor)
    }
}
