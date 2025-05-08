use crate::errors::AppError;
use std::fmt::Display;

/// Generates compatible SQL string for defined Sqlx types
pub trait ToQuery {
    fn to_query(&self, bn: &BackendName) -> String;
    fn offset(&self) -> &u8 {
        &1
    }
}

#[derive(Clone)]
/// Rust representations for [sqlx::PoolConnection<Any>::backend_name].
pub enum BackendName {
    Postgres,
    Mysql,
    Sqlite,
}

impl<'a> TryFrom<&'a str> for BackendName {
    type Error = AppError;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        if value == "PostgreSQL" {
            Ok(Self::Postgres)
        } else if value == "MySQL" {
            Ok(Self::Mysql)
        } else if value == "SQLite" {
            Ok(Self::Sqlite)
        } else {
            Err(AppError::DatabaseError(format!(
                "unable to parse backend name {value}"
            )))
        }
    }
}

/// For generating compatible filter (WHERE) SQL string
enum Filter<'a> {
    Base(&'a str),
    And(&'a str),
    Or(&'a str),
}

impl Display for Filter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Filter::Base(col) => write!(f, "{col}"),
            Filter::And(col) => write!(f, "AND {col}"),
            Filter::Or(col) => write!(f, "OR {col}"),
        }
    }
}

/// A wrapper that holds a chain of [Filter]s which can be converted
/// into sql query string.
///
///
/// Example:
/// ```
/// let filters = SqlxFilters::("id");
/// filters.and("age");
/// filters.to_query(&bn); // id = $1 AND age = $1
/// ```
pub struct SqlxFilters<'a> {
    items: Vec<Filter<'a>>,
    offset: u8,
}

impl<'a> SqlxFilters<'a> {
    pub fn new(col: &'a str, offset: u8) -> Self {
        SqlxFilters {
            items: Vec::from([Filter::Base(col)]),
            offset,
        }
    }

    pub fn and(mut self, col: &'a str) -> Self {
        self.items.push(Filter::And(col));
        self
    }

    pub fn or(mut self, col: &'a str) -> Self {
        self.items.push(Filter::Or(col));
        self
    }
}

impl ToQuery for SqlxFilters<'_> {
    fn offset(&self) -> &u8 {
        &self.offset
    }

    fn to_query(&self, bn: &BackendName) -> String {
        let output: Vec<String> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, s)| match bn {
                BackendName::Postgres => format!("{} = ${}", s, (i as u8) + self.offset()),
                _ => format!("{} = ?", s),
            })
            .collect();

        output.join(" ")
    }
}

/// Generates query string with compatible placeholders for SQL VALUES. Allows you to
/// provide how many placeholders you would like to generate.
///
///
/// Example:
/// ```
/// let values = SqlxValues(3);
/// values.to_query(bn); // VALUES($1, $2, $3)
/// ```
pub struct SqlxValues(pub u8, pub u8);
impl ToQuery for SqlxValues {
    fn offset(&self) -> &u8 {
        &self.1
    }
    fn to_query(&self, bn: &BackendName) -> String {
        let mut values = Vec::with_capacity(self.0 as usize);

        for i in 0..self.0 {
            match bn {
                BackendName::Postgres => values.push(format!("${}", i + self.offset())),
                _ => values.push("?".to_string()),
            }
        }

        let values = values.join(", ");
        format!("VALUES({values})")
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::sqlx_utils::{SqlxValues, ToQuery};

    use super::{BackendName, SqlxFilters};

    #[test]
    fn test_sqlx_filters_pg() {
        let filters = SqlxFilters::new("id", 1);
        let bn = BackendName::Postgres;

        assert_eq!(&filters.to_query(&bn), "id = $1");

        let filters = filters.and("age").or("name");
        assert_eq!(&filters.to_query(&bn), "id = $1 AND age = $2 OR name = $3");
    }

    #[test]
    fn test_sqlx_values_pg() {
        let values = SqlxValues(1, 1);
        let bn = BackendName::Postgres;

        assert_eq!(&values.to_query(&bn), "VALUES($1)");

        let values = SqlxValues(3, 1);
        assert_eq!(&values.to_query(&bn), "VALUES($1, $2, $3)");
    }

    #[test]
    fn test_sqlx_filters_mysql() {
        let filters = SqlxFilters::new("id", 1);
        let bn = BackendName::Mysql;

        assert_eq!(&filters.to_query(&bn), "id = ?");

        let filters = filters.and("age").or("name");
        assert_eq!(&filters.to_query(&bn), "id = ? AND age = ? OR name = ?");
    }

    #[test]
    fn test_sqlx_values_mysql() {
        let values = SqlxValues(1, 1);
        let bn = BackendName::Mysql;

        assert_eq!(&values.to_query(&bn), "VALUES(?)");

        let values = SqlxValues(3, 1);
        assert_eq!(&values.to_query(&bn), "VALUES(?, ?, ?)");
    }
}
