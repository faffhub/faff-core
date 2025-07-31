use chrono::{NaiveDate, NaiveTime, DateTime, TimeZone, FixedOffset};
use chrono_tz::Tz;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Toy {
    pub word: String,
}

impl Toy {

    pub fn hello(&self) -> Result<String, std::io::Error> {
        Ok(self.word.clone())
    }

    pub fn toy(&self, new_word: String) -> Self {
        Self { word: new_word }
    }

    pub fn do_a_datetime(&self, datetime: DateTime<Tz>) -> Result<String, std::io::Error>
    {
        Ok(datetime.to_string())
    }

}