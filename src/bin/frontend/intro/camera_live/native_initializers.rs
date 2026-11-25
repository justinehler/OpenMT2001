//! Finite native first writers. These functions never execute original code.
//! Operations named *_scalars project exactly consumed scalar stores; the
//! text/resource owner required by an enclosing constructor remains separate.
use super::process_image::Cells;
pub fn node43e700(c:&mut Cells,p:u32)->Result<(),String>{
    for o in [4,8,12,16,0x20,0x28,0x2c,0x30,0x34]{c.set_word(p+o,0)?;}
    c.set_word(p+0x14,0x3f000000)?;c.set_word(p+0x18,1)?;
    c.set_byte(p+0x1c,1)?;c.set_byte(p+0x1d,0)?;c.set_byte(p+0x38,0)?;
    c.write(p+0x39,&[0;255])?;c.set_word(p,0x52597c)
    // +24, padding1e/1f deliberately untouched, not zeroed heap state.
}
pub fn group43c570(c:&mut Cells,p:u32)->Result<(),String>{
    //4dfa90 embedded buffer; pointer,count,capacity,flag0c.
    for o in [0x28,0x2c,0x30]{c.set_word(p+o,0)?;}c.set_byte(p+0x34,0)?;
    c.set_word(p,0x5258fc)?;c.set_byte(p+4,0)?;
    for o in [0xc,8,0x24]{c.set_word(p+o,0)?;}c.set_word(0x590504,0)?;
    c.set_byte(p+0x20,0)?;c.set_word(p+0x10,0)?;c.set_byte(p+5,0)?;
    c.set_word(p+0x14,0)?;c.set_word(p+0x18,0)
    //+1c key-target is not a constructor store. Preserve actual preimage.
}
///401000 entire containing function has no callees. Static object560820 is
///allocated by the PE loader;+24/report is not initialized by this constructor.
pub fn property401000(c:&mut Cells,p:u32)->Result<(),String>{
    for o in [0,4,0x14,0x18]{c.set_word(p+o,u32::MAX)?;}
    for o in [0x20,0x28,0x70]{c.set_word(p+o,0)?;}
    for o in [0xc0,0x1c,0x11,0x12]{c.set_byte(p+o,0)?;}
    for start in [0x4c,0x94]{for i in 0..6{c.set_word(p+start+i*4,0)?;}}
    Ok(())
}
pub fn panel43b380_scalars(c:&mut Cells,p:u32)->Result<(),String>{
    panel43b380_prefix(c,p)?;
    panel43b380_suffix(c,p)
}
/// Common containing constructor calls caption440160 BETWEEN these halves.
pub fn panel43b380_prefix(c:&mut Cells,p:u32)->Result<(),String>{
    node43e700(c,p)?;
    for start in [0x13c,0x150,0x164]{
        for i in 0..4{c.set_word(p+start+i*4,0)?;}c.set_word(p+start+16,0x3f000000)?;
    }
    Ok(())
}
pub fn panel43b380_suffix(c:&mut Cells,p:u32)->Result<(),String>{
    //440160 caption at+188 must already have completed for a full constructor.
    c.set_word(p,0x52553c)?;
    for o in [0x345,0x344,0x349,0x346,0x34c,0x34e,0x34d,0x347,0x184]{c.set_byte(p+o,0)?;}
    c.set_byte(p+0x348,1)?;c.set_word(p+0x350,2)?;
    for o in [0x384,0x380,0x354,0x17c,0x180,0x36c,0x368,0x364,0x138]{c.set_word(p+o,0)?;}
    Ok(())
}
pub fn main439ae0_scalars(c:&mut Cells,p:u32)->Result<(),String>{
    panel43b380_scalars(c,p)?;
    for i in 0..6{let a=p+0x3b0+i*0x24;
        for o in [12,8,4,0]{c.set_word(a+o,0)?;}c.set_word(a+16,0x3f000000)?;}
    for o in [0x4e0,0x4dc,0x4d8,0x4d4]{c.set_word(p+o,0)?;}
    c.set_word(p+0x4e4,0x3f000000)?;c.set_word(p,0x525668)
}
///Actual F layout43d010 writes click-enabled+15e0=0. UpperF's source finding:
///43d130 hover prelude is gated by inherited node+1d, NOT this byte.
pub fn navigator43d010_scalar(c:&mut Cells,p:u32)->Result<(),String>{c.set_byte(p+0x15e0,0)}
///42a6d0 layout reset leaves counter560b14, rows and colors intact.
pub fn world42a6d0(c:&mut Cells,p:u32)->Result<(),String>{
    c.set_word(p+0x4a8,0)?;c.set_byte(p+0xde8,1)
}
///4253a0 reset is caption+network state;425400 only does the caption suffix.
pub fn bottom4253a0(c:&mut Cells,p:u32)->Result<(),String>{
    let speed=c.word(p+0x38c)?;
    if speed==0{return Err("actual E speed control absent before reset".into())}
    if c.byte(0x93bbb9)?!=0{c.set_byte(speed+0x333,1)?;c.set_byte(speed+0x335,0)?;}
    else{c.set_byte(speed+0x333,0)?;}
    bottom425400(c,p)
}
pub fn bottom425400(c:&mut Cells,p:u32)->Result<(),String>{
    let compass=c.word(p+0x388)?;
    if compass==0{return Err("actual E compass absent before caption reset".into())}
    let id=if c.word(0x8b1050)?==3{1143}else{1142};
    c.set_byte(compass+0x38,0)?;c.set_word(compass+0x34,id)
}
///Direct prefix of4b9d40, also used by the process startup caller. This is
///not the whole world reset; root must still run its remaining actual steps.
pub fn world_reset_selection_prefix(c:&mut Cells)->Result<(),String>{
    for a in [0x822d50,0x823168,0x590f60,0x6175f8]{c.set_word(a,0)?;}Ok(())
}
