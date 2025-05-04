use crate::errors::AppError;
use std::fmt::Display;

/// Generates compatible SQL string for defined Sqlx types
pub trait ToQuery {
    fn to_query(self, bn: &BackendName) -> String;
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

impl<'a> Display for Filter<'a> {
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
}

impl<'a> SqlxFilters<'a> {
    pub fn new(col: &'a str) -> Self {
        SqlxFilters {
            items: Vec::from([Filter::Base(col)]),
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

impl<'a> ToQuery for SqlxFilters<'a> {
    fn to_query(self, bn: &BackendName) -> String {
        let output: Vec<String> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, s)| match bn {
                BackendName::Postgres => format!("{} = ${}", s, i + 1),
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
pub struct SqlxValues(pub u8);
impl ToQuery for SqlxValues {
    fn to_query(self, bn: &BackendName) -> String {
        let mut values = Vec::with_capacity(self.0 as usize);

        for i in 0..self.0 {
            match bn {
                BackendName::Postgres => values.push(format!("${}", i + 1)),
                _ => values.push("?".to_string()),
            }
        }

        let values = values.join(", ");
        format!("VALUES({values})")
    }
}

// impl<'t> sqlx::Decode<'t, sqlx::Any> for NaiveDateTime {
//     fn decode(
//         value: <sqlx::Any as sqlx::Database>::ValueRef<'t>,
//     ) -> Result<Self, sqlx::error::BoxDynError> {
//         NaiveDateTime::decode(value)
//     }
// }
