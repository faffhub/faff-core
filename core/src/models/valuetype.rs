#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueType {
    String(String),
    List(Vec<String>),
}

impl ValueType {
    pub fn as_string(&self) -> Option<&String> {
        match self {
            ValueType::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<String>> {
        match self {
            ValueType::List(v) => Some(v),
            _ => None,
        }
    }
}
