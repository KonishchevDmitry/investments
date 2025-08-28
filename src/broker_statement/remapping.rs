use crate::time::DateOptTime;

pub struct SymbolRemappingRule {
    pub old: String,
    pub new: String,
}

#[derive(Clone, Copy)]
pub enum SymbolRenameType {
    CorporateAction{
        time: DateOptTime,
    },
    Remapping {
        check_existence: bool,
        allow_override: bool,
    },
}