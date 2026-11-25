//! Sole retained UI selection authority.
use super::property_descriptors::Handle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionKey {
    /// Existing actual native object resolver domain. Never derive this from
    /// category, render-vector position, or a modern Handle.
    NativeObject(u32),
    PropertyDescriptor(Handle),
}
impl SelectionKey {
    pub fn native_key(self) -> Result<u32,String> {
        match self {
            Self::NativeObject(key) => Ok(key),
            Self::PropertyDescriptor(_) => Err("modern property handle has no proven native pointer/key conversion".into()),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selected {
    pub key: SelectionKey,
    pub kind: i16,
    pub category: u32,
    pub hit: Option<crate::selection::Hit>, // display projection, not identity
}
/// Actual6175f8 related object domain; NOT a pavement handle or selected
/// building590f60 alias. No related-object dereference is implemented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelatedKey(u32);
impl RelatedKey {
    pub fn from_native_cell(raw: u32) -> Option<Self> { (raw != 0).then_some(Self(raw)) }
    pub fn native_key(self) -> u32 { self.0 }
}
#[derive(Debug)]
pub struct SelectionState {
    pub current: Option<Selected>,       //822d50
    pub published: Option<Selected>,     //590f58
    pub transient: Option<Selected>,     //823168
    pub related: Option<RelatedKey>,     //6175f8
    pub selected_building: Option<u32>,  //590f60
    pub sequence_index: Option<i32>,
    pub list_business: i32,
    pub list_commodity: i32,
}
impl SelectionState {
    /// Explicit process-cell/live-producer install only. Never Default.
    pub fn installed(current: Option<Selected>, published: Option<Selected>,
        transient: Option<Selected>, related: Option<RelatedKey>,
        selected_building: Option<u32>, sequence_index: Option<i32>,
        list_business: i32, list_commodity: i32,
    ) -> Self {
        Self { current,published,transient,related,selected_building,
            sequence_index,list_business,list_commodity }
    }
    ///40f890 preserved: it does not implicitly overwrite823168/6175f8.
    pub fn strategic_hide(&mut self, retained: Option<Selected>) {
        self.selected_building = None;
        self.current = retained;
        self.list_business = -1;
        self.list_commodity = -1;
    }
    pub fn publish_current(&mut self) { self.published = self.current; }
}
