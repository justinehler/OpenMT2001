//! One genuine normal Visual442600 implementation shared by Voting and Chat.
//! Caller retains the subtype BEFORE invoking initialize442600; failures keep
//! the pre-caption prefix. Node linking/key/aliases belong to each real arena.
use super::{
    case_zero_text::{Fonts, Text},
    main_construction::Field,
    process_images::ImageId,
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageRef {
    Null,
    Image(ImageId),
    Invalid,
}
pub struct Visual {
    pub caption: Text,
    pub icon_bounds: [f32; 5],
    ///Absent entries are unwritten/unknown, not zero native preimages. In
    ///particular2f4 and350..364 remain absent throughout this constructor.
    pub scalars: BTreeMap<u32, u32>,
    pub images: BTreeMap<u32, ImageRef>,
    pub embedded_context24: Field<u32>,
    ///Some activates unaligned native X ownership; None keeps established Voting carrier.
    pub caption_x: Option<Vec<Option<i16>>>,
}

impl Visual {
    pub(super) fn normal_fields() -> Self {
        Self {
            caption: Text::normal_fields(),
            icon_bounds: [0., 0., 0., 0., 0.5],
            scalars: BTreeMap::new(),
            images: BTreeMap::new(),
            embedded_context24: Field::Unknown,
            caption_x: None,
        }
    }
    ///442600 stores on either side of the actual embedded440900 call.
    pub(super) fn initialize442600(&mut self, e: [f32; 4]) -> Result<(), String> {
        for offset in [0x310, 0x30c, 0x304, 0x300] {
            self.images.insert(offset, ImageRef::Invalid);
        }
        self.scalars.insert(0x32c, u32::MAX);
        for offset in [0x330, 0x333, 0x331, 0x332, 0x33c] {
            self.scalars.insert(offset, 0);
        }
        self.scalars.insert(0x13c, 2);
        self.scalars.insert(0x314, 0);
        self.caption.extent440900(e)?;
        self.images.insert(0x348, ImageRef::Invalid);
        for offset in [0x335, 0x338, 0x336, 0x34c, 0x2fc] {
            self.scalars.insert(offset, 0);
        }
        for offset in [0x300, 0x304, 0x308, 0x30c, 0x310] {
            self.images.insert(offset, ImageRef::Invalid);
        }
        self.scalars.insert(0x370, 0);
        Ok(())
    }
    ///442960: reset, identifier, second resize/color clear, THEN2f4.
    ///The shared native-order identifier helper selects born Font0 itself.
    pub(super) fn caption_identifier442960(
        &mut self,
        font: &mut Fonts,
        id: u32,
    ) -> Result<(), String> {
        let x = self.caption_x.get_or_insert_with(Vec::new);
        self.caption.reset_unaligned(x)?;
        self.caption.identifier441220_unaligned(font, id, x)?;
        self.caption.resize_unaligned(x)?; //4429e2 distinct repeat
        let n = self.caption.group.count as usize;
        self.caption.colors[..n].fill(0);
        self.scalars.insert(0x2f4, id);
        Ok(())
    }
    ///442a20 leaves2f4 untouched, even for the supplied empty literal.
    pub(super) fn caption_literal442a20(
        &mut self,
        font: &mut Fonts,
        literal: &[u8],
    ) -> Result<(), String> {
        let x = self.caption_x.get_or_insert_with(Vec::new);
        self.caption.reset_unaligned(x)?;
        self.caption.literal441770_unaligned(font, literal, x)
    }
}
