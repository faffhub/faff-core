#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueType {
    String(String),
    List(Vec<String>),
    Integer(i32),
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

    pub fn as_integer(&self) -> Option<i32> {
        match self {
            ValueType::Integer(i) => Some(*i),
            _ => None,
        }
    }
}
