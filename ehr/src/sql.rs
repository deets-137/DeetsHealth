//! SQL joined at run time (sqlx 0.9; DEPENDENCIES.md §4).
//!
//! sqlx takes only a `&'static str` as a query, so text from a request, a record or a setting can
//! never become SQL. A few queries are joined from pieces: a class's schema and role, a table and
//! its sealed columns, a numbered parameter. `Sql` joins them under the same rule. It takes
//! `&'static str` text, numbers and bytes, and never a `String`, so every piece is text written
//! in this code. Every value still goes through a `$n` bind.
//!
//! This is the one `AssertSqlSafe` in ehr. A query that needs no pieces stays a plain
//! `&'static str` (with `concat!` for a fixed fragment used in several queries).

use sqlx::{AssertSqlSafe, SqlSafeStr, SqlStr};

/// A query joined from text fixed in the code.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sql(String);

impl Sql {
    pub fn new() -> Self {
        Self::default()
    }

    /// Text written in the code: a keyword, a table, a column, a fixed expression.
    pub fn text(mut self, text: &'static str) -> Self {
        self.0.push_str(text);
        self
    }

    /// A number: a parameter's index (`$` comes from `text`) or a name's suffix.
    pub fn num(mut self, n: usize) -> Self {
        self.0.push_str(&n.to_string());
        self
    }

    /// One byte as a `bytea` literal: `'\xNN'::bytea`.
    pub fn byte(mut self, b: u8) -> Self {
        self.0.push_str(&format!("'\\x{b:02x}'::bytea"));
        self
    }

    /// Another piece joined under the same rule.
    pub fn part(mut self, part: Sql) -> Self {
        self.0.push_str(&part.0);
        self
    }

    /// Each part, `sep` between them.
    pub fn join(mut self, parts: impl IntoIterator<Item = Sql>, sep: &'static str) -> Self {
        for (i, part) in parts.into_iter().enumerate() {
            if i > 0 {
                self.0.push_str(sep);
            }
            self.0.push_str(&part.0);
        }
        self
    }

    /// The text, for a test.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SqlSafeStr for Sql {
    fn into_sql_str(self) -> SqlStr {
        // Safe: every piece is a `&'static str` from this code, a number or a byte (above).
        AssertSqlSafe(self.0).into_sql_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_the_pieces_in_order() {
        let cols = ["a", "b"].map(|c| Sql::new().text(c).text(" = v.n").num(1));
        let q = Sql::new().text("UPDATE t SET ").join(cols, ", ").text(" WHERE ").byte(3).text(" <> $").num(2);
        assert_eq!(q.as_str(), "UPDATE t SET a = v.n1, b = v.n1 WHERE '\\x03'::bytea <> $2");
    }

    #[test]
    fn an_empty_join_adds_nothing() {
        assert_eq!(Sql::new().text("x").join(std::iter::empty(), ", ").as_str(), "x");
    }
}
