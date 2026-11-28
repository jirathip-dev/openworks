use openworks_core::{Call, Fail, Tx, Value};
use openworks_db::series::NEXT;

const DAY: i64 = 24 * 60 * 60 * 1000;

/// When a series starts again at one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Period {
    Never,
    Year,
    Month,
}

/// A number series: a counter row per org and period, its numbers written by a pattern with `{period}` and `{n}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Series {
    pub key: &'static str,
    pub pattern: &'static str,
    pub period: Period,
}

impl Series {
    /// Takes the next number in the write's transaction: a rolled-back write burns none, a committed one is never reused.
    pub fn next(&self, call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
        let period = self.label(call.at);
        let rows = tx.run(
            NEXT,
            &[
                Value::Text(call.who.org.0.clone()),
                Value::Text(self.key.to_owned()),
                Value::Text(period.clone()),
            ],
        )?;
        let Some(Value::Int(n)) = rows.first().and_then(|row| row.first()) else {
            return Err(Fail::Store(format!("series {} gave no number", self.key)));
        };
        Ok(self.pattern.replace("{period}", &period).replace("{n}", &n.to_string()))
    }

    fn label(&self, at: i64) -> String {
        let (year, month) = year_and_month(at);
        match self.period {
            Period::Never => String::new(),
            Period::Year => year.to_string(),
            Period::Month => format!("{year}-{month:02}"),
        }
    }
}

fn year_and_month(at: i64) -> (i64, i64) {
    let days = at.div_euclid(DAY) + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (year_of_era + era * 400 + i64::from(month <= 2), month)
}
