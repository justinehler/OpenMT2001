//! Original47ab80 acquisition selection, including complete mode2 valuations.
use crate::{ai_messages::offers::{execution,auction::valuation},ai_strategy_queries as q,ai_valuation_queries as v,player_manager::{Player,word,put},x87::Sum};
/// Original47c4f0 seeks a property auction. Valuation retains the live player
/// side effects while the temporary category is restored after admission.
pub fn seek_auction(player:&mut Player,w:&valuation::scoring::World<'_>,auction:Option<(u32,bool)>,identity:&mut u32,prior:[u8;104])->Result<(),String>{
    let loan=*w.loan_status.get(player.bytes[0x1bc] as usize).ok_or("auction seeking cash status missing")?!=0;
    //472800 returns false for each original character0..9, true otherwise.
    if (loan&&word(&player.bytes,0x150)<=9)||player.bytes[0xb86]!=0||auction.is_some_and(|(_,active)|active){return Ok(());}
    let original=word(&player.bytes,0xb68);put(&mut player.bytes,0xb68,u32::MAX);
    let mut best=None;let mut best_score=0;let mut pending_owner=u32::MAX;let mut pending_amount=0;let mut budget=0;
    for id in 0..52{
        if w.categories[id][0x74]==0||player.bytes[0x178+id]!=0{continue;}
        if crate::ai_actions::queries::pending_offer(player,id as u32,&mut pending_owner,&mut pending_amount)?{continue;}
        put(&mut player.bytes,0xb68,id as u32);
        let score=valuation::evaluate(player,0,auction.map(|(id,_)|id),w.image.f32_at(0x5250e0)?,&mut budget,|call,player|w.dispatch(call,player))?.ftol_low_word();
        let score=if score<75{0}else{score};
        if score>best_score{best_score=score;best=Some(id as u32);}
    }
    if let Some(id)=best{
        crate::ai_actions::proposals::transaction(0x4784c0,player,[id,100],prior,identity,|call,_,_|match call{
            crate::ai_actions::proposals::Call::CategoryValue(id)=>execution::category_value(w.categories.get(id as usize).ok_or("auction proposal category missing")?,w.image),
            crate::ai_actions::proposals::Call::Diagnostic(_)=>Ok(Sum::from_i32(0)),
            other=>Err(format!("unexpected auction seeking proposal {other:?}")),
        })?;
        player.bytes[0x13c5]=1;
    }
    put(&mut player.bytes,0xb68,original);Ok(())
}
///470360 converts only the right score through an f32 stack store.
fn compare(left:[i32;2],right:[i32;2])->i32{
    match Sum::from_i32(left[0]).compare(Sum::from_f32(Sum::from_i32(right[0]).to_f32()).expect("finite integer")){
        std::cmp::Ordering::Less=>1,std::cmp::Ordering::Equal=>0,std::cmp::Ordering::Greater=>-1,
    }
}
///477570 scans non-utility groups for24 ground cells held by this player's businesses.
pub fn substantial_presence(player:&Player,w:&valuation::scoring::World<'_>)->Result<bool,String>{
    for (id,category) in w.categories.iter().enumerate(){
        let group=w.image.at(0x5385a4+id as u32*88,1)?[0];
        if matches!(group,0|10|11)||player.bytes[0x178+id]!=0{continue;}
        if v::owned_area(player,category,w.buildings,w.businesses)? >=24{return Ok(true);}
    }
    Ok(false)
}
///The original late checks discard their return values; retain their actual
///chain traversal and conditional query sequence instead of guessing a veto.
fn late_checks(player:&Player,category:usize,w:&valuation::scoring::World<'_>)->Result<(),String>{
    if !q::utility_or_transport(w.image,category as u32)?{
        let _vacant=q::occupied_area(&w.categories[category],w.buildings)?==0; //476ac0
    }
    if v::owned_businesses(u32::from(player.bytes[0x1bc]),&w.categories[category],w.buildings,w.businesses)?==0{
        let percent=q::occupied_area(&w.categories[category],w.buildings)?.wrapping_mul(100)/49; //473ce0
        if percent<=30{let _=substantial_presence(player,w)?;}
    }
    Ok(())
}
pub fn select(player:&mut Player,w:&valuation::scoring::World<'_>,auction:Option<(u32,bool)>,checks:bool)->Result<bool,String>{
    if auction.is_some_and(|(_,blocked)|blocked){return Ok(false);}
    let cash=f32::from_bits(word(&player.bytes,4));
    let mut capacity=Sum::from_i32(v::income_margin(player)?);capacity.add_f32(cash)?;let capacity=capacity.ftol_low_word();
    let mut rows=[[-1i32;2];52];let mut count=0;let mut pending_owner=u32::MAX;let mut pending_amount=0;
    let mut factor=Sum::from_i32(crate::ai_profiles::adjustment(word(&player.bytes,0x150)));
    factor.add_f32(w.image.f32_at(0x525ca0)?)?;factor.mul_f32(w.image.f32_at(0x5250e0)?)?;factor.add_f32(w.image.f32_at(0x525058)?)?;
    let mut factor=factor.ftol_low_word();
    if *w.loan_status.get(player.bytes[0x1bc] as usize).ok_or("acquisition loan slot missing")?!=0{
        factor=factor.wrapping_sub(24i32.wrapping_sub(crate::ai_actions::queries::trouble_hours(player,w.loan_status,w.hour)?));
    }
    for (id,category) in w.categories.iter().enumerate(){
        if category[0x74]==0||category[5]!=0||player.bytes[0x178+id]!=0{continue;}
        // These output locals deliberately survive successive4698c0 calls.
        if crate::ai_actions::queries::pending_offer(player,id as u32,&mut pending_owner,&mut pending_amount)?{continue;}
        put(&mut player.bytes,0xb68,id as u32);
        let mut score=0;
        valuation::evaluate(player,2,auction.map(|(id,_)|id),w.image.f32_at(0x5250e0)?,&mut score,|call,p|w.dispatch(call,p))?;
        let threshold=Sum::from_i32(score).to_f32();
        let mut value=execution::category_value(category,w.image)?;value.add_f32(w.image.f32_at(0x525178)?)?;
        if !value.compare(Sum::from_f32(threshold)?).is_gt(){rows[count]=[score,id as i32];count+=1;}
    }
    if count==0{return Ok(false);}
    let capacity=factor.wrapping_mul(capacity)/100;
    let mut maximum=0f32;
    for row in &rows[..count]{let score=Sum::from_i32(row[0]);if score.compare(Sum::from_f32(maximum)?).is_gt(){maximum=score.to_f32();}}
    let mut budget=Sum::from_i32(capacity);let max=Sum::from_f32(maximum)?;if max.compare(budget).is_lt(){budget=max;}let budget=budget.ftol_low_word();
    let mut threshold=Sum::from_f32(crate::ai_profiles::factor(w.image,0x47d360,word(&player.bytes,0x150))?)?;
    threshold.mul_f32(maximum)?;threshold.mul_f32(w.image.f32_at(0x5250e0)?)?;
    let mut choices=count;
    for row in &mut rows[..count]{if Sum::from_i32(row[0]).compare(threshold).is_lt(){row[0]=0;choices-=1;}}
    crate::native_sort::sort(&mut rows,&compare);
    let random=word(&player.bytes,0x224).wrapping_mul(word(&player.bytes,0x228)).wrapping_add(word(&player.bytes,0x22c));put(&mut player.bytes,0x224,random);
    if choices==0{return Err("original47ab80 divides by zero after filtering all candidates".into());}
    let category=rows[((random>>16)&0x7fff) as usize%choices][1];if category<=-1{return Ok(false);}
    if checks{late_checks(player,category as usize,w)?;}
    put(&mut player.bytes,0xcb8,budget as u32);put(&mut player.bytes,0xb68,category as u32);put(&mut player.bytes,0xb64,2);put(&mut player.bytes,0xb54,0);player.bytes[0xb76]=1;
    Ok(true)
}

#[test]
#[ignore="requires complete original acquisition selector fixture and executable"]
fn matches_native_acquisition(){
    use crate::{pe::PeImage,economy::ResidentAccounts,business_pool::BusinessCatalog,resident::{Resident,SpendingAccount}};
    fn take<'a>(data:&mut &'a [u8],n:usize)->&'a[u8]{let(a,b)=data.split_at(n);*data=b;a}
    fn read(data:&mut &[u8])->u32{word(take(data,4),0)}
    let fixture=std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_ACQUISITION").unwrap()).unwrap();
    let exe=std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();let image=PeImage::parse(&exe).unwrap();let mut data=&fixture[..];
    assert_eq!(take(&mut data,8),b"MTAIACQ1");assert_eq!(read(&mut data),0x37f);let count=read(&mut data);
    for case in 0..count{
        let slot=read(&mut data) as usize;let hour=read(&mut data) as i32;let auction=read(&mut data);let auction_category=read(&mut data);let checks=read(&mut data)!=0;
        let status:&[u8;6]=take(&mut data,6).try_into().unwrap();let sentinel=take(&mut data,1)[0];
        let categories:Box<[[u8;380];52]>=take(&mut data,52*380).chunks_exact(380).map(|r|r.try_into().unwrap()).collect::<Vec<_>>().into_boxed_slice().try_into().unwrap();
        let mut buildings=vec![[0u8;916];600];for(out,b)in buildings.iter_mut().zip(take(&mut data,16*916).chunks_exact(916)){out.copy_from_slice(b);}buildings[599][0x24d]=sentinel;
        let mut businesses=vec![[0u8;544];600];for(out,b)in businesses.iter_mut().zip(take(&mut data,16*544).chunks_exact(544)){out.copy_from_slice(b);}
        let raw=take(&mut data,45*116);let catalog=BusinessCatalog::from_rows(std::array::from_fn(|i|raw[i*116..(i+1)*116].try_into().unwrap()));
        let utilities=std::array::from_fn(|_|f32::from_bits(read(&mut data)));
        let nodes=take(&mut data,36*330).chunks_exact(330).map(|r|Resident::from_bytes(r.try_into().unwrap())).collect();
        let accounts=take(&mut data,36*144).chunks_exact(144).map(|r|SpendingAccount::from_bytes(r.try_into().unwrap())).collect();let residents=ResidentAccounts{active_count:36,nodes,accounts};
        let raw=take(&mut data,32);let active=std::array::from_fn(|i|raw[i]!=0);
        let mut players:Vec<Player>=(0..6).map(|i|Player{reference:0x10010000+i*0x2000,bytes:take(&mut data,0x1e50).to_vec(),auxiliaries:vec![]}).collect();
        let returned=read(&mut data)!=0;let expected=take(&mut data,6*0x1e50);let valuations=read(&mut data);let _native_categories=take(&mut data,valuations as usize*4);
        let(before,current)=players.split_at_mut(slot);let(current,after)=current.split_first_mut().unwrap();
        let world=valuation::scoring::World{image:&image,categories:&categories,buildings:&buildings,businesses:&businesses,catalog:&catalog,native_utilities:&utilities,loan_status:status,hour,residents:&residents,commodity_active:&active,before,after};
        let actual=select(current,&world,if auction==0{None}else{Some((auction_category,auction==1))},checks).unwrap_or_else(|e|panic!("case{case}: {e}"));
        assert_eq!(actual,returned,"case{case} result");
        for(i,p)in players.iter().enumerate(){let want=&expected[i*0x1e50..(i+1)*0x1e50];if let Some(at)=p.bytes.iter().zip(want).position(|(a,b)|a!=b){panic!("case{case} player{i} byte{at:x}: {:02x} != {:02x}",p.bytes[at],want[at]);}}
    }
    assert!(data.is_empty());
}

#[test]
#[ignore="requires complete original acquisition selector fixture and executable"]
fn matches_native_auction_seeking(){
    use crate::{pe::PeImage,economy::ResidentAccounts,business_pool::BusinessCatalog,resident::{Resident,SpendingAccount}};
    fn take<'a>(data:&mut &'a [u8],n:usize)->&'a[u8]{let(a,b)=data.split_at(n);*data=b;a}
    fn read(data:&mut &[u8])->u32{word(take(data,4),0)}
    let fixture=std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_AUCTION_SEEKING").unwrap()).unwrap();
    let exe=std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();let image=PeImage::parse(&exe).unwrap();let mut data=&fixture[..];
    assert_eq!(take(&mut data,8),b"MTAISEK1");assert_eq!(read(&mut data),0x37f);let count=read(&mut data);
    for case in 0..count{
        let slot=read(&mut data) as usize;let hour=read(&mut data) as i32;let auction=read(&mut data);let auction_category=read(&mut data);
        let status:&[u8;6]=take(&mut data,6).try_into().unwrap();let sentinel=take(&mut data,1)[0];
        let categories:Box<[[u8;380];52]>=take(&mut data,52*380).chunks_exact(380).map(|r|r.try_into().unwrap()).collect::<Vec<_>>().into_boxed_slice().try_into().unwrap();
        let mut buildings=vec![[0u8;916];600];for(out,b)in buildings.iter_mut().zip(take(&mut data,16*916).chunks_exact(916)){out.copy_from_slice(b);}buildings[599][0x24d]=sentinel;
        let mut businesses=vec![[0u8;544];600];for(out,b)in businesses.iter_mut().zip(take(&mut data,16*544).chunks_exact(544)){out.copy_from_slice(b);}
        let raw=take(&mut data,45*116);let catalog=BusinessCatalog::from_rows(std::array::from_fn(|i|raw[i*116..(i+1)*116].try_into().unwrap()));
        let utilities=std::array::from_fn(|_|f32::from_bits(read(&mut data)));
        let nodes=take(&mut data,36*330).chunks_exact(330).map(|r|Resident::from_bytes(r.try_into().unwrap())).collect();
        let accounts=take(&mut data,36*144).chunks_exact(144).map(|r|SpendingAccount::from_bytes(r.try_into().unwrap())).collect();let residents=ResidentAccounts{active_count:36,nodes,accounts};
        let raw=take(&mut data,32);let active=std::array::from_fn(|i|raw[i]!=0);
        let mut players:Vec<Player>=(0..6).map(|i|Player{reference:0x10010000+i*0x2000,bytes:take(&mut data,0x1e50).to_vec(),auxiliaries:vec![]}).collect();
        let prior=take(&mut data,104).try_into().unwrap();let expected_identity=read(&mut data);let valuations=read(&mut data);let expected=take(&mut data,6*0x1e50);let _native_categories=take(&mut data,valuations as usize*4);let mut identity=31;
        let(before,current)=players.split_at_mut(slot);let(current,after)=current.split_first_mut().unwrap();
        let world=valuation::scoring::World{image:&image,categories:&categories,buildings:&buildings,businesses:&businesses,catalog:&catalog,native_utilities:&utilities,loan_status:status,hour,residents:&residents,commodity_active:&active,before,after};
        seek_auction(current,&world,if auction==0{None}else{Some((auction_category,auction==1))},&mut identity,prior).unwrap_or_else(|e|panic!("case{case}: {e}"));
        assert_eq!(identity,expected_identity,"case{case} proposal identity");
        for(i,p)in players.iter().enumerate(){let want=&expected[i*0x1e50..(i+1)*0x1e50];if let Some(at)=p.bytes.iter().zip(want).position(|(a,b)|a!=b){panic!("case{case} player{i} byte{at:x}: {:02x} != {:02x}",p.bytes[at],want[at]);}}
    }
    assert!(data.is_empty());
}
