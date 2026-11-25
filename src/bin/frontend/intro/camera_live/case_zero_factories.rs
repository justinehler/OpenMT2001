//! Moves real subtype products once after native construction/layout.
//! The font manager is borrowed from the common process owner, never moved
//! into case0. Missing containing process dispatch remains explicitly gated.
use super::{process_image::Cells,case_zero::{State,List},case_zero_text::{Text,Fonts,Literals},property_child::Selector};
use std::collections::BTreeMap;
pub struct CaseZeroContent {
    pub lists:BTreeMap<u32,List>,pub texts:BTreeMap<u32,Text>,pub selectors:BTreeMap<u32,Selector>,
}
pub fn case_zero(c:&Cells,content:&mut CaseZeroContent,fonts:&Fonts,literals:&mut Option<Literals>)
    ->Result<(State,Text),String>{
    if !fonts.fonts.contains_key(&fonts.current){return Err("actual shared case0 active font absent".into())}
    literals.as_ref().ok_or("actual case0 PE literals absent or already moved")?;
    let parent=0x58e3c0;let p=c.word(parent+0x38c)?;
    if p==0||c.word(p)?!=parent{return Err("actual child0 parent linkage absent".into())}
    let list=c.word(p+8)?;let indicator=c.word(p+12)?;let notice=c.word(parent+0x3a0)?;
    let state=c.word(p+4)?;
    if notice==0||[list,indicator].contains(&notice)||list!=0&&list==indicator{
        return Err("actual case0 products alias incompatible owners".into())
    }
    // A native address has one typed product, including across product maps.
    for key in [list,indicator,notice].into_iter().filter(|&key|key!=0){
        let owners=usize::from(content.lists.contains_key(&key))+usize::from(content.texts.contains_key(&key))
            +usize::from(content.selectors.contains_key(&key));
        if owners!=1{return Err("actual case0 address lacks a unique subtype owner".into())}
    }
    let actual_list=if list==0{None}else{
        let l=content.lists.get(&list).ok_or("actual case0 list subtype not constructed or already moved")?;
        if l.key!=list{return Err("actual case0 list address/key mismatch".into())}Some(l)
    };
    if indicator!=0{
        let selector=content.selectors.get(&indicator).ok_or("actual case0 metric subtype not constructed or already moved")?;
        actual_list.ok_or("actual case0 selector requires attached list")?;
        if c.word(indicator+0x15c)?!=list||c.word(indicator+0x160)?!=0{
            return Err("actual case0 selector native attachments differ from list owner".into())
        }
        if selector.simple.is_some()||selector.text_list.is_some()||selector.last_list.is_some()||selector.first_list.is_none(){
            return Err("case0 selector attachments differ from actual41f750 list binding".into())
        }
    }
    content.texts.get(&notice).ok_or("actual common parent notice text not constructed or already moved")?;
    // No fallible producer or validation remains after this ownership boundary.
    let list=if list==0{None}else{Some(content.lists.remove(&list).expect("validated case0 list"))};
    let indicator=if indicator==0{None}else{Some(content.selectors.remove(&indicator).expect("validated case0 selector"))};
    let notice=content.texts.remove(&notice).expect("validated case0 notice");
    let literals=literals.take().expect("validated PE literals");
    Ok((State{state,list,indicator,literals},notice))
}
//At owned_factories::main, append these actual products to installed's args:
//  ...,property,model,notice_text,case_zero,children
//Do not also move the same notice via Content.captions. Its previous scalar
//caption pair can be retained only as a projection from this exact Text owner.

///Actualbusiness and child5 reset owners from the same completed factories.
///Owned subtype products MUST be supplied by the actual layout constructors;
///this adapter only moves them, preserving pointer aliases by address.
pub fn other_resets(c:&Cells,nodes:&mut BTreeMap<u32,super::other_child_resets::Node>,
    selectors:&mut BTreeMap<u32,Selector>,image:&openmt2001::pe::PeImage<'_>)
    ->Result<(Option<super::other_child_resets::Business>,Option<super::other_child_resets::Five>,Text),String>{
    use super::other_child_resets::{Business,Five,LinkedSelector,Currency,Node};
    let parent=0x58e3c0;
    let title=c.word(parent+0x4b8)?;
    match nodes.get(&title).ok_or("actual parent+4b8 text facet absent or already moved")?{
        Node::Text{..} if title!=0=>(),
        _=>return Err("actual parent+4b8 text facet subtype mismatch".into())
    }
    if selectors.contains_key(&title){return Err("actual shared title has duplicate selector authority".into())}
    // Plans contain scalar data and validated keys, never cloned live products.
    let two=c.word(parent+0x398)?;
    let plan=if two==0{None}else{
        if c.word(two+0x44)?!=parent{return Err("actual business reset parent mismatch".into())}
        let mut pointers=BTreeMap::new();
        for at in [0x1c,0x20,0x50,0x54,0x58,0x64,0x88,0x8c,0x90,0x98,0xa4,0xac,0xc8]{pointers.insert(at,c.word(two+at)?);}
        let selector=c.word(two+0xb8)?;
        let attachments=if selector==0{None}else{
            if selector==title||nodes.contains_key(&selector){return Err("actual business selector aliases another subtype owner".into())}
            let owner=selectors.get(&selector).ok_or("actual business selector absent or multiply moved")?;
            let first=c.word(selector+0x15c)?;let last=c.word(selector+0x160)?;
            if owner.simple.is_some()||owner.text_list.is_some(){return Err("business selector requires unsupported external simple/text source".into())}
            if first==0&&owner.first_list.is_some()||last==0&&owner.last_list.is_some(){
                return Err("business selector attachment/source mismatch".into())
            }
            Some((selector,first,last))
        };
        let mut keys=std::collections::BTreeSet::new();
        for &p in pointers.values(){if p!=0{keys.insert(p);}}
        if let Some((_,first,last))=attachments{if first!=0{keys.insert(first);}if last!=0{keys.insert(last);}}
        for &p in &keys{
            if p==title||selectors.contains_key(&p){return Err("actual business node aliases a separately owned title/selector".into())}
            let node=nodes.get(&p).ok_or_else(||format!("actual business subtype{p:08x} absent or multiply moved"))?;
            if let Node::List(l)=node{if l.key!=p{return Err("actual business list address/key mismatch".into())}}
        }
        if let Some((_,first,last))=attachments{
            for key in [first,last].into_iter().filter(|&key|key!=0){
                if !matches!(nodes.get(&key),Some(Node::List(_))){return Err("business selector actual attached list owner absent".into())}
            }
        }
        for at in [0x90,0x88,0x8c,0xac,0xc8,0x64,0x50,0x54,0x58]{
            if pointers[&at]==0{return Err(format!("business mandatory field+{at:x} is null"))}
        }
        if pointers[&0xa4]!=0&&!matches!(nodes.get(&pointers[&0xa4]),Some(Node::Text{..})){
            return Err("business+a4 must be the actual text subtype".into())
        }
        for at in [0xac,0xc8]{if pointers[&at]!=0&&!matches!(nodes.get(&pointers[&at]),Some(Node::Visual(_))){
            return Err("business native+333/+335 target is not actual visual subtype".into())
        }}
        fn ntbs(image:&openmt2001::pe::PeImage<'_>,at:u32)->Result<Vec<u8>,String>{let bytes=image.at(at,32)?;
            let n=bytes.iter().position(|&b|b==0).ok_or("supplied currency format unterminated")?;Ok(bytes[..n].to_vec())}
        let mut buffer=c.ntbs(0x57f734,1024)?;buffer.push(0);
        let currency=Currency{raw_symbol:c.ntbs(0x57e6ec,1024)?,suffix:c.word(0x59050c)?,buffer,
            prefix_format:ntbs(image,0x536410)?,suffix_format:ntbs(image,0x536400)?};
        Some((pointers,keys,attachments,c.word(two+0x28)?,c.word(two+0x2c)?,c.word(two+0x34)?,
            c.word(two+0x48)?,c.byte(two+0x4c)?,c.word(two+0xb4)?,currency))
    };
    let five=c.word(parent+0x394)?;
    let five=if five==0{None}else{Some(Five{state24:c.word(five+0x24)?})};
    // All Cells/PE reads, map lookups, subtype and alias checks precede moves.
    // Deduplicated keys retain one node for any repeated business pointers.
    let info_text=match nodes.remove(&title).expect("validated shared title"){
        Node::Text{text,..}=>text,_=>unreachable!("validated shared title subtype")
    };
    let business=plan.map(|(pointers,keys,attachments,category28,selection_mode2c,option34,action48,active4c,cacheb4,currency)|{
        let selector=attachments.map(|(key,first,last)|LinkedSelector{
            owner:selectors.remove(&key).expect("validated business selector"),first,last});
        let owned=keys.into_iter().map(|key|(key,nodes.remove(&key).expect("validated business node"))).collect();
        Business{pointers,nodes:owned,selector,category28,selection_mode2c,option34,action48,active4c,cacheb4,currency}
    });
    Ok((business,five,info_text))
}
//Final State::installed suffix:
// ...,property,model,notice_text,case_zero,info_text,business_reset,five_reset,children

fn group(c:&Cells,p:u32)->Result<super::case_zero_text::Group,String>{
        use super::case_zero_text::Group;
        let nc=c.word(p)? as usize;let nb=c.word(p+4)? as usize;let b=c.word(p+8)?;let o=c.word(p+12)?;
        let bytes=if nb==0{Vec::new()}else{c.read(b,nb)?.to_vec()};
        let mut offsets=Vec::with_capacity(nc);
        for i in 0..nc{offsets.push(c.word(o+i as u32*4)?)}
        Ok(Group{bytes,offsets,count:c.word(p+16)?})
    }

///Literal actual440160/441940/440330 model state, never Caption::Id -> Text.
///Reads fail on any consumed cell lacking a real constructor/layout writer.
pub fn text_owner(c:&Cells,p:u32)->Result<Text,String>{
    if p==0{return Err("mandatory actual text owner null".into())}
    let count=c.half(p+0x158)? as i16;
    if count<0{return Err("text source capacity outside native finite domain".into())}
    let bytes=|at|->Result<Vec<u8>,String>{if count==0{return Ok(Vec::new())}Ok(c.read(c.word(p+at)?,count as usize)?.to_vec())};
    let xp=c.word(p+0x168)?;let mut x=Vec::new();for i in 0..count as u32{x.push(c.half(xp+i*2)? as i16)}
    Ok(Text{reset_target:super::case_zero_text::ResetTarget::from_source_target(
        c.word(c.word(p)?.checked_add(0x2c).ok_or("Text reset vtable offset overflow")?)?),bounds:c.rect(p+4)?,depth:c.float(p+0x14)?,font:c.word(p+0x13c)?,pitch:c.word(p+0x140)? as i32,
        gap:c.byte(p+0x144)? as i8,visible:c.word(p+0x148)?,line_mode:c.byte(p+0x14c)?,scale:c.float(p+0x150)?,
        padding:c.word(p+0x154)? as i32,colors:bytes(0x15c)?,blink_colors:bytes(0x160)?,blink:bytes(0x164)?,x,
        extent:c.rect(p+0x16c)?,group:group(c,p+0x180)?,y:c.half(p+0x194)? as i16,inset:c.word(p+0x198)? as i32,
        scroll:c.word(p+0x19c)? as i32,style:c.word(p+0x1a0)?,first:c.word(p+0x1a4)?,dirty:c.byte(p+0x1a8)?,
        geometry_dirty:c.byte(p+0x1a9)?,background_color:c.word(p+0x1ac)?,background:c.byte(p+0x1b0)?})
}

///Actual initialized manager8b22cc. Font counts are the registration loader's
///real counts for its actual0..31 slots; they cannot be guessed from a name.
pub fn font_manager(c:&Cells,registered_counts:&BTreeMap<u32,u32>,
    scratch:Vec<super::case_zero_text::ScratchByte>,error:super::case_zero_text::ErrorWord)->Result<Fonts,String>{
    use super::case_zero_text::{FontData,Group};
    let p=c.word(0x8b22cc)?;if p==0{return Err("actual font manager8b22cc absent".into())}
    //Scratch is moved from the actual font-constructor/map-sample producer.
    //Unwritten slots retain unknown native bytes; no zero-heap fiction.
    if scratch.len()!=1024{return Err("actual font manager scratch span differs from native1024".into())}
    let mut fonts=BTreeMap::new();
    for (&id,&count) in registered_counts{
        if id>=32{return Err("actual font registration index outside native32 slots".into())}
        let ptr=c.word(p+0x410+id*4)?;let mut table=count.to_le_bytes().to_vec();
        table.extend_from_slice(c.read(ptr,count as usize*44)?);openmt2001::font::Font::parse(&table)?;
        fonts.insert(id,FontData{table,baseline:c.word(p+0x4b0+id*4)? as i32,scale:c.float(p+0x530+id*4)?});
    }
    let count=c.word(p+0x5b0)?;let offsets=c.word(p)?;let bytes=c.word(p+4)?;
    let mut lengths=Vec::new();let mut at=0u32;
    for i in 0..count{let n=c.half(offsets+i*2)?;lengths.extend_from_slice(&n.to_le_bytes());at=at.checked_add(u32::from(n)).ok_or("actual packed language length overflow")?;}
    let language=super::ui_text_resources::PackedLanguage::parse(&lengths,c.read(bytes,at as usize)?.to_vec())?;
    let g=p+0x5cc;let nc=c.word(g)?;let nb=c.word(g+4)?;let gp=c.word(g+8)?;let op=c.word(g+12)?;
    let mut offsets=Vec::new();for i in 0..nc{offsets.push(c.word(op+i*4)?)}
    let formatted=Group{bytes:if nb==0{Vec::new()}else{c.read(gp,nb as usize)?.to_vec()},offsets,count:c.word(g+16)?};
    //The caller moves observed error provenance. Do not read unwritten5b4.
    Ok(Fonts{fonts,language,current:c.word(p+0x5e0)?,gap:c.word(p+0x5ec)? as i32,
        line_mode:c.byte(p+0x5e8)?,scratch,formatted,temporary:group(c,p+0x5b8)?,error,
        selector:super::case_zero_text::SelectorState{cached:c.word(0x6aad08)?,pending:[c.word(0x6aad00)?,c.word(0x6aad0c)?],notify:c.word(p+0x5f4)?},
        context:c.word(p+0x5e4)?,glyph_callback:c.word(p+0x5f0)?,ready_callback:c.word(p+0x5f8)?,four_callback:c.word(p+0x5fc)?,
        pair:[c.word(p+0x600)?,c.word(p+0x604)?],tail:Some(c.word(p+0x608)?)})
}

///Replace only property factory's former controls.widget(a+4b8) expression.
///The caption storage itself was moved by other_resets into main.info_text.
pub fn property_title(c:&Cells,caption:super::property_child::Caption)->Result<super::property_child::Title,String>{
    let p=c.word(0x58e3c0+0x4b8)?;
    if p==0{return Err("actual shared report title node absent".into())}
    Ok(super::property_child::Title{shown:c.byte(p+0x1c)?,caption})
    //Projection comes from the actual typed text producer at this boundary.
}

#[cfg(test)]
mod binding_retention_regressions {
    use super::*;
    use super::super::{main_panel::regression_tests as fixture,other_child_resets::Node};
    const PARENT:u32=0x58e3c0;const CHILD:u32=0x600000;const TITLE:u32=0x610000;
    const VISUAL:u32=0x620000;const MONEY:u32=0x620100;const LIST:u32=0x620200;const SELECTOR:u32=0x630000;
    // A synthetic PE header/section for parser tests; contains no game bytes.
    fn pe(with_formats:bool)->Vec<u8>{
        let mut bytes=vec![0;0x200+if with_formats{0x6500}else{0}];
        bytes[..2].copy_from_slice(b"MZ");bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        bytes[0x86..0x88].copy_from_slice(&u16::from(with_formats).to_le_bytes());
        bytes[0x94..0x96].copy_from_slice(&64u16.to_le_bytes());bytes[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        bytes[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
        if with_formats{
            bytes[0xe0..0xe4].copy_from_slice(&0x6500u32.to_le_bytes());
            bytes[0xe4..0xe8].copy_from_slice(&0x130000u32.to_le_bytes());
            bytes[0xe8..0xec].copy_from_slice(&0x6500u32.to_le_bytes());
            bytes[0xec..0xf0].copy_from_slice(&0x200u32.to_le_bytes());
            bytes[0x6600..0x6605].copy_from_slice(b"%i%s\0");bytes[0x6610..0x6615].copy_from_slice(b"%s%i\0");
        }bytes
    }
    fn cells()->Cells{
        let mut c=Cells::process_allocation(&pe(false)).unwrap();c.heap_allocation(PARENT,0x500).unwrap();c
    }
    fn maps(content:&CaseZeroContent)->String{format!("{:?}/{:?}/{:?}",content.lists,content.texts,content.selectors)}
    #[test]
    fn case_zero_keeps_every_owner_on_unwritten_state_missing_notice_and_attachment_failure_then_retries(){
        let mut c=cells();c.heap_allocation(CHILD,16).unwrap();c.heap_allocation(SELECTOR,0x170).unwrap();
        for (at,v) in [(PARENT+0x38c,CHILD),(CHILD,PARENT),(CHILD+8,LIST),(CHILD+12,SELECTOR),(PARENT+0x3a0,TITLE),
            (SELECTOR+0x15c,LIST),(SELECTOR+0x160,0)]{c.set_word(at,v).unwrap();}
        let mut content=CaseZeroContent{lists:BTreeMap::from([(LIST,fixture::list(LIST))]),texts:BTreeMap::new(),selectors:BTreeMap::from([(SELECTOR,fixture::selector())])};
        let mut fonts=Some(fixture::fonts());let mut literals=Some(fixture::literals());
        let font_ptr=fonts.as_ref().unwrap().fonts[&0].table.as_ptr();let before=maps(&content);
        assert!(case_zero(&c,&mut content,fonts.as_ref().unwrap(),&mut literals).unwrap_err().contains("producer"));
        assert_eq!(maps(&content),before);assert!(fonts.is_some()&&literals.is_some());
        c.set_word(CHILD+4,7).unwrap();
        assert!(case_zero(&c,&mut content,fonts.as_ref().unwrap(),&mut literals).is_err());assert_eq!(maps(&content),before);
        content.texts.insert(TITLE,fixture::text());c.set_word(SELECTOR+0x15c,TITLE).unwrap();let before=maps(&content);
        assert!(case_zero(&c,&mut content,fonts.as_ref().unwrap(),&mut literals).unwrap_err().contains("attachments"));assert_eq!(maps(&content),before);
        c.set_word(SELECTOR+0x15c,LIST).unwrap();
        let (state,_notice)=case_zero(&c,&mut content,fonts.as_ref().unwrap(),&mut literals).unwrap();
        assert_eq!(state.state,7);assert_eq!(state.list.unwrap().key,LIST);assert!(state.indicator.is_some());
        assert_eq!(fonts.as_ref().unwrap().fonts[&0].table.as_ptr(),font_ptr,"factory borrows sole process manager");
        assert!(fonts.is_some()&&literals.is_none());assert!(content.lists.is_empty()&&content.texts.is_empty()&&content.selectors.is_empty());
    }
    #[test]
    fn case_zero_rejects_duplicate_address_authority_without_consuming_products(){
        let mut c=cells();c.heap_allocation(CHILD,16).unwrap();
        for (at,v) in [(PARENT+0x38c,CHILD),(CHILD,PARENT),(CHILD+4,0),(CHILD+8,LIST),(CHILD+12,0),(PARENT+0x3a0,TITLE)]{c.set_word(at,v).unwrap();}
        let mut content=CaseZeroContent{lists:BTreeMap::from([(LIST,fixture::list(LIST))]),texts:BTreeMap::from([(TITLE,fixture::text()),(LIST,fixture::text())]),selectors:BTreeMap::new()};
        let before=maps(&content);let mut fonts=Some(fixture::fonts());let mut literals=Some(fixture::literals());
        assert!(case_zero(&c,&mut content,fonts.as_ref().unwrap(),&mut literals).unwrap_err().contains("unique"));assert_eq!(maps(&content),before);
        content.texts.remove(&LIST);case_zero(&c,&mut content,fonts.as_ref().unwrap(),&mut literals).unwrap();
    }
    fn other_fixture()->(Cells,BTreeMap<u32,Node>,BTreeMap<u32,Selector>){
        let mut c=cells();c.heap_allocation(CHILD,0xd0).unwrap();c.heap_allocation(CHILD+0x100,0x30).unwrap();c.heap_allocation(SELECTOR,0x170).unwrap();
        for (at,v) in [(PARENT+0x4b8,TITLE),(PARENT+0x398,CHILD),(PARENT+0x394,CHILD+0x100),(CHILD+0x44,PARENT),
            (CHILD+0xb8,SELECTOR),(SELECTOR+0x15c,LIST),(SELECTOR+0x160,LIST),
            (CHILD+0x28,7),(CHILD+0x2c,1),(CHILD+0x34,1),(CHILD+0x48,9)]{c.set_word(at,v).unwrap();}
        c.set_byte(CHILD+0x4c,1).unwrap();
        for at in [0x1c,0x20,0x50,0x54,0x58,0x64,0x88,0x8c,0x90,0x98,0xa4,0xac,0xc8]{c.set_word(CHILD+at,if at==0xa4{MONEY}else{VISUAL}).unwrap();}
        for (at,bytes) in [(0x57f734,b"old\0".as_slice()),(0x57e6ec,b"$\0".as_slice())]{c.heap_allocation(at,1024).unwrap();c.write(at,bytes).unwrap();}
        c.heap_allocation(0x59050c,4).unwrap();c.set_word(0x59050c,0).unwrap();
        let nodes=BTreeMap::from([(TITLE,Node::Text{shown:1,text:fixture::text()}),(VISUAL,Node::Visual(fixture::widget())),
            (MONEY,Node::Text{shown:1,text:fixture::text()}),(LIST,Node::List(fixture::list(LIST)))]);
        (c,nodes,BTreeMap::from([(SELECTOR,fixture::selector())]))
    }
    #[test]
    fn other_resets_keeps_title_selector_and_aliased_nodes_on_late_reads_then_moves_originals_once(){
        let (mut c,mut nodes,mut selectors)=other_fixture();let bytes=pe(true);let image=openmt2001::pe::PeImage::parse(&bytes).unwrap();
        let before=format!("{nodes:?}/{selectors:?}");
        assert!(other_resets(&c,&mut nodes,&mut selectors,&image).unwrap_err().contains("producer"));assert_eq!(format!("{nodes:?}/{selectors:?}"),before);
        c.set_word(CHILD+0xb4,3).unwrap();
        assert!(other_resets(&c,&mut nodes,&mut selectors,&image).unwrap_err().contains("producer"));assert_eq!(format!("{nodes:?}/{selectors:?}"),before);
        c.set_word(CHILD+0x124,4).unwrap();
        let bad_bytes=pe(false);let bad_image=openmt2001::pe::PeImage::parse(&bad_bytes).unwrap();
        assert!(other_resets(&c,&mut nodes,&mut selectors,&bad_image).is_err());assert_eq!(format!("{nodes:?}/{selectors:?}"),before);
        let (business,five,_title)=other_resets(&c,&mut nodes,&mut selectors,&image).unwrap();let business=business.unwrap();
        assert_eq!(business.nodes.len(),3);assert_eq!(business.pointers[&0x64],business.pointers[&0x50]);
        assert_eq!(business.selector.as_ref().unwrap().first,LIST);assert_eq!(business.selector.as_ref().unwrap().last,LIST);
        assert_eq!(five.unwrap().state24,4);assert!(nodes.is_empty()&&selectors.is_empty());
    }
    #[test]
    fn wrong_title_money_subtype_and_shared_title_alias_leave_all_products_retryable(){
        let (mut c,mut nodes,mut selectors)=other_fixture();let bytes=pe(true);let image=openmt2001::pe::PeImage::parse(&bytes).unwrap();
        nodes.insert(TITLE,Node::Visual(fixture::widget()));let before=format!("{nodes:?}/{selectors:?}");
        assert!(other_resets(&c,&mut nodes,&mut selectors,&image).unwrap_err().contains("subtype"));assert_eq!(format!("{nodes:?}/{selectors:?}"),before);
        nodes.insert(TITLE,Node::Text{shown:1,text:fixture::text()});nodes.insert(MONEY,Node::Visual(fixture::widget()));let before=format!("{nodes:?}/{selectors:?}");
        assert!(other_resets(&c,&mut nodes,&mut selectors,&image).unwrap_err().contains("a4"));assert_eq!(format!("{nodes:?}/{selectors:?}"),before);
        nodes.insert(MONEY,Node::Text{shown:1,text:fixture::text()});c.set_word(CHILD+0xa4,TITLE).unwrap();let before=format!("{nodes:?}/{selectors:?}");
        assert!(other_resets(&c,&mut nodes,&mut selectors,&image).unwrap_err().contains("aliases"));assert_eq!(format!("{nodes:?}/{selectors:?}"),before);
        c.set_word(CHILD+0xa4,MONEY).unwrap();c.set_word(CHILD+0xb4,3).unwrap();c.set_word(CHILD+0x124,4).unwrap();
        other_resets(&c,&mut nodes,&mut selectors,&image).unwrap();
    }
    #[test]
    fn missing_node_bad_attachment_and_unterminated_currency_do_not_consume_other_products(){
        let (mut c,mut nodes,mut selectors)=other_fixture();let bytes=pe(true);let image=openmt2001::pe::PeImage::parse(&bytes).unwrap();
        c.set_word(CHILD+0xb4,3).unwrap();c.set_word(CHILD+0x124,4).unwrap();
        let visual=nodes.remove(&VISUAL).unwrap();let before=format!("{nodes:?}/{selectors:?}");
        assert!(other_resets(&c,&mut nodes,&mut selectors,&image).is_err());assert_eq!(format!("{nodes:?}/{selectors:?}"),before);
        nodes.insert(VISUAL,visual);c.set_word(SELECTOR+0x15c,MONEY).unwrap();let before=format!("{nodes:?}/{selectors:?}");
        assert!(other_resets(&c,&mut nodes,&mut selectors,&image).unwrap_err().contains("attached list"));assert_eq!(format!("{nodes:?}/{selectors:?}"),before);
        c.set_word(SELECTOR+0x15c,LIST).unwrap();c.write(0x57e6ec,&[b'X';1024]).unwrap();let before=format!("{nodes:?}/{selectors:?}");
        assert!(other_resets(&c,&mut nodes,&mut selectors,&image).unwrap_err().contains("NTBS"));assert_eq!(format!("{nodes:?}/{selectors:?}"),before);
        c.write(0x57e6ec,b"$\0").unwrap();other_resets(&c,&mut nodes,&mut selectors,&image).unwrap();
    }

}
