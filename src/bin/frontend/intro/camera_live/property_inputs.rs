//! Capture at the live Session/UI transaction boundary, before the worker is
//! blocked for synchronous camera/UI dispatch. No Scene or Host requests.
use openmt2001::{level_startup::World, pe::PeImage, player_manager::Manager};
use crate::block_information::{self,land_rent};
#[derive(Clone,Debug)]
pub struct Category {
    pub label:u32,pub metadata_group:u8,pub title:u32,pub color:u32,
    pub available:u8,pub rights:u32,pub offer_pending:u8,pub rights_pending:u8,
    pub owner_reference:u32,pub owner_slot:i32,pub owner_name:Vec<u8>,
    pub value:openmt2001::x87::Sum,pub selected_amount:f32,
    pub summary:Vec<block_information::Row>,pub land:Vec<land_rent::Row>,
}
#[derive(Clone,Debug)]
pub struct Inputs {
    pub categories:Vec<Category>,pub utility_labels:[u32;4],pub other_labels:[u32;4],
    pub current_reference:u32,pub current_slot:u8,
    pub mode_titles:[u32;3],pub mode_images:[u32;3],
    pub info_titles:[u32;5],pub info_images:[u32;5],pub special_image:u32,
    pub inset:f64,pub currency_symbol:Vec<u8>,pub currency_suffix:bool,
}
fn word(b:&[u8],at:usize)->Result<u32,String>{Ok(u32::from_le_bytes(
    b.get(at..at+4).ok_or("property payload truncated")?.try_into().unwrap()))}
fn table(image:&PeImage<'_>,address:u32)->Result<u32,String>{word(image.at(address,4)?,0)}
fn name(bytes:&[u8])->Result<Vec<u8>,String>{
    let bytes=bytes.get(0x158..).ok_or("actual property owner name absent")?;
    let end=bytes.iter().position(|&b|b==0).ok_or("property owner name unterminated")?;
    Ok(bytes[..end].to_vec())
}
/// images is the ACTUAL process68ae28 image-handle table, special_image the
/// actual68b2dc cell. They are never PE pointers or guessed texture indices.
pub fn capture(world:&World,manager:&Manager,current_reference:u32,image:&PeImage<'_>,
    images:&[u32],special_image:u32,currency_symbol:&[u8],currency_suffix:bool)->Result<Inputs,String>{
    let current=manager.players.iter().find(|p|p.reference==current_reference)
        .ok_or("property payload requires actual86cfc0 human")?;
    let current_slot=*current.bytes.get(0x1bc).ok_or("property current identity missing")?;
    let labels=|base:u32|->Result<[u32;4],String>{
        let mut result=[0;4];
        for i in 0..4 {
            let index=table(image,base+i as u32*4)?;
            result[i]=word(world.categories.get(index as usize).ok_or("property special category")?,0)?;
        }Ok(result)
    };
    let utility_labels=labels(0x541eac)?;let other_labels=labels(0x541ebc)?;
    let category_refs:Vec<_>=world.categories.iter().map(|b|b.as_slice()).collect();
    let building_refs:Vec<_>=world.buildings.iter().map(|b|b.as_slice()).collect();
    let business_refs:Vec<_>=world.ledgers.businesses.iter().map(|b|b.as_slice()).collect();
    let memberships=land_rent::runtime_memberships(&world.categories,&world.buildings)?;
    let land=land_rent::from_memberships(&category_refs,&building_refs,&business_refs,
        manager.players.len(),image,&memberships)?;
    let bias=f64::from_le_bytes(image.at(0x525050,8)?.try_into().unwrap());
    let mut categories=Vec::with_capacity(52);
    for (index,row) in world.categories.iter().enumerate(){
        let label=word(row,0)?;
        let owner_slot=i32::from(row[5] as i8)-1;
        let owner=if row[5]==0{None}else{Some(manager.players.get(usize::try_from(owner_slot)
            .map_err(|_|"negative property owner")?).ok_or("property owner outside actual roster")?)};
        // Actual40453e indexes the prestige input by category.label, not the
        // selected category index.4045b1 compares ENCODED owner directly with
        // current+1bc (without subtracting1). Preserve both native quirks.
        let prestige_row=world.categories.get(label as usize).ok_or("property label category index")?;
        let summary=block_information::rows(f32::from_bits(word(prestige_row,0x84)?),bias,
            word(row,0x78)? as i32,row[5],current_slot,word(row,0x7c)? as i32)?;
        let metadata=word(row,0x8c)?;
        let value=openmt2001::ai_messages::offers::execution::category_value(row,image)?;
        categories.push(Category{label,metadata_group:*image.at(0x5385a4+label*88,1)?.first().unwrap(),
            title:table(image,0x5385a0+metadata*88)?.wrapping_add(3),color:word(row,0x94)?,
            available:row[0x74],rights:word(row,0x78)?,offer_pending:row[0x6c],rights_pending:row[0x6d],
            owner_reference:owner.map_or(0,|p|p.reference),owner_slot,
            owner_name:owner.map(|p|name(&p.bytes)).transpose()?.unwrap_or_default(),
            value,selected_amount:openmt2001::property_offer::selected_amount(row,image)?,
            summary,land:land.get(index).ok_or("property land payload missing category")?.clone()});
    }
    let mut mode_titles=[0;3];let mut mode_images=[0;3];
    let mut info_titles=[0;5];let mut info_images=[0;5];
    for i in 0..3 {mode_titles[i]=table(image,0x533238+i as u32*4)?;
        let key=table(image,0x533244+i as u32*4)?;
        mode_images[i]=*images.get(key as usize).ok_or("property mode image outside actual table")?;}
    for i in 0..5 {let key=table(image,0x533250+i as u32*8)?;
        info_images[i]=*images.get(key as usize).ok_or("property info image outside actual table")?;
        info_titles[i]=table(image,0x533254+i as u32*8)?;}
    let inset=f64::from_le_bytes(image.at(0x525038,8)?.try_into().unwrap());
    if !inset.is_finite(){return Err("nonfinite supplied property inset".into());}
    Ok(Inputs{categories,utility_labels,other_labels,current_reference,current_slot,
        mode_titles,mode_images,info_titles,info_images,special_image,inset,
        currency_symbol:currency_symbol.to_vec(),currency_suffix})
}
