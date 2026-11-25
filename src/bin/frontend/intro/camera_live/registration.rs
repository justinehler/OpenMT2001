//! 43cc60 /4918c0 /434120 actual bind-and-register order, not head order.
use super::{navigation::Group,ui_graph::PanelId,process_image::Cells};
#[derive(Clone,Copy,Debug)]
pub enum Descriptor { Model(u8), Special(u32) }
#[derive(Clone,Copy,Debug)]
pub struct Entry {pub id:PanelId,pub object:u32,pub descriptor:Descriptor}
use Descriptor::{Model,Special};use PanelId::*;
pub const PRIMARY:[Entry;18]=[
    Entry{id:D,object:0x574488,descriptor:Special(0x574810)},
    Entry{id:OtherPrimary(1),object:0x573b38,descriptor:Model(15)}, //voting
    Entry{id:OtherPrimary(2),object:0x571378,descriptor:Model(14)}, //EventList
    Entry{id:OtherPrimary(3),object:0x57ef28,descriptor:Model(13)}, //chat
    Entry{id:G,object:0x57e798,descriptor:Model(12)},
    Entry{id:H,object:0x571808,descriptor:Model(11)},
    Entry{id:B,object:0x577048,descriptor:Model(10)},
    Entry{id:OtherPrimary(7),object:0x58f438,descriptor:Model(9)}, //CommunityChanceCard
    Entry{id:OtherPrimary(8),object:0x575280,descriptor:Model(8)}, //external auction
    Entry{id:OtherPrimary(9),object:0x575d98,descriptor:Model(7)}, //Prompt
    Entry{id:E,object:0x58ca08,descriptor:Model(6)},
    Entry{id:Clock,object:0x58fb40,descriptor:Model(5)},
    Entry{id:A,object:0x58e3c0,descriptor:Model(4)},
    Entry{id:Comparison,object:0x56f3f0,descriptor:Model(3)},
    Entry{id:C,object:0x58e8d0,descriptor:Model(2)},
    Entry{id:OtherPrimary(15),object:0x56cdc8,descriptor:Model(1)}, //console
    Entry{id:F,object:0x58cdb0,descriptor:Model(0)},
    Entry{id:OtherPrimary(17),object:0x57f838,descriptor:Special(0x580448)}, //city notice
];
// Frontend id is descriptor/registration ordinal; birth order differs at
// Board19/Custom22/Login16. Never use field/birth order as registration order.
pub const FRONTEND_OFFSETS:[u32;23]=[0x38,0x3d8,0x1f44,0x13b20,0x15dcc,0x16184,
    0x16540,0x1698c,0x16e34,0x1880c,0x19184,0x1b478,0x1b9b0,0x1beec,0x1c274,
    0x1e118,0x1d94c,0x1ea64,0x1fd14,0x172e0,0x1e5b4,0x202cc,0x18cc8];
pub const AUCTION_OBJECTS:[u32;7]=[0x567d20,0x5641a8,0x560b28,0x566578,0x563230,0x565bb0,0x5648a8];
pub fn entries(group:Group)->Vec<Entry>{match group{
    Group::Primary=>PRIMARY.to_vec(),
    Group::Secondary=>FRONTEND_OFFSETS.iter().enumerate().map(|(i,&offset)|Entry{
        id:Frontend(i as u8),object:0x86d670+offset,descriptor:Model(i as u8)}).collect(),
    Group::Tertiary=>(0..7).rev().map(|i|Entry{id:Auction(i as u8),object:AUCTION_OBJECTS[i],descriptor:Model(i as u8)}).collect(),
}}
pub fn group_address(group:Group)->u32{match group{Group::Primary=>0x57e710,Group::Secondary=>0x86d670,Group::Tertiary=>0x565b58}}
///43c600 insertion/promotion over the same source-owned links. Registration
///MUST happen after each bind/layout returns, at its actual call boundary.
pub fn promote(c:&mut Cells,group:Group,p:u32)->Result<(),String>{
    let g=group_address(group);let head=c.word(g+8)?;
    if head==0{c.set_word(g+12,p)?;c.set_word(g+8,p)?;c.set_word(p+0x384,0)?;c.set_word(p+0x380,0)?;return Ok(())}
    let previous=c.word(p+0x384)?;
    if previous!=0{let next=c.word(p+0x380)?;
        if next!=0{c.set_word(previous+0x380,next)?;
            let next=c.word(p+0x380)?;let previous=c.word(p+0x384)?;c.set_word(next+0x384,previous)?;
        }else{c.set_word(previous+0x380,0)?;let previous=c.word(p+0x384)?;c.set_word(g+12,previous)?;}}
    let head=c.word(g+8)?;
    if head!=p{c.set_word(head+0x384,p)?;let head=c.word(g+8)?;
        c.set_word(p+0x380,head)?;c.set_word(g+8,p)?;c.set_word(p+0x384,0)?;}
    Ok(())
}
pub const PRIMARY_ASSET_VAS:[u32;16]=[0x5473b8,0x547398,0x54736c,0x547340,
    0x547314,0x5472e0,0x5472b0,0x547280,0x547244,0x547210,0x5471e8,0x5471b8,
    0x547314,0x54718c,0x54715c,0x547130];
/// Exact supplied literal for each Primary descriptor; G/A share an asset
/// name but native calls create separate bound descriptors and model copies.
pub fn primary_asset(cells:&Cells,index:u8)->Result<String,String>{
    let va=*PRIMARY_ASSET_VAS.get(index as usize).ok_or("Primary model index outside16")?;
    String::from_utf8(cells.ntbs(va,256)?).map_err(|_|"actual asset literal is not UTF8".into())
}
pub const FRONTEND_ASSET_VAS:[u32;23]=[0x547108,0x5470d0,0x54709c,0x54706c,
    0x547108,0x547040,0x547014,0x546fe4,0x546fb4,0x546f84,0x546f58,0x546f2c,
    0x546f00,0x546ed4,0x546ea8,0x546ed4,0x546e7c,0x546e50,0x546e20,0x546df4,
    0x546ed4,0x547108,0x546f84];
pub const AUCTION_ASSET_VAS:[u32;7]=[0x546dc4,0x546d94,0x546d60,0x546d30,0x546cfc,0x546ccc,0x546e50];
pub fn asset(c:&Cells,group:Group,index:u8)->Result<String,String>{
    let vas:&[u32]=match group{Group::Primary=>&PRIMARY_ASSET_VAS,Group::Secondary=>&FRONTEND_ASSET_VAS,Group::Tertiary=>&AUCTION_ASSET_VAS};
    let va=*vas.get(index as usize).ok_or("actual group descriptor index outside bound pool")?;
    String::from_utf8(c.ntbs(va,256)?).map_err(|_|"supplied asset literal is not UTF8".into())
}
