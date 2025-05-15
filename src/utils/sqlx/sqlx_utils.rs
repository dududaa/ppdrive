use crate::errors::AppError;
use std::fmt::Display;

/// Generates compatible SQL string for defined Sqlx types
pub trait ToQuery {
    fn to_query(&self, bn: &BackendName) -> String;
}

#[derive(Clone)]
/// Rust representations for [sqlx::PoolConnection<Any>::backend_name].
pub enum BackendName {
    Postgres,
    Mysql,
    Sqlite,
}

impl BackendName {
    pub fn to_query(&self, index: u8) -> String {
        use BackendName::*;

        match self {
            Postgres => format!("${}", index),
            Sqlite => format!("?{}", index),
            Mysql => "?".to_string(),
        }
    }
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
    Group(Vec<Filter<'a>>),
}

impl Display for Filter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Filter::*;

        match self {
            Base(col) => write!(f, "{col}"),
            And(col) => write!(f, "AND {col}"),
            Or(col) => write!(f, "OR {col}"),
            Group(items) => {
                let ss: Vec<String> = items.iter().map(|i| i.to_string()).collect();
                write!(f, "{}", ss.join(" "))
            }
        }
    }
}

impl<'a> TryFrom<&'a str> for Filter<'a> {
    type Error = AppError;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        use Filter::*;

        let items: Vec<&str> = value.split(" ").collect();

        if let Some(first) = items.first() {
            if items.len() == 1 {
                Ok(Base(first))
            } else {
                if ["AND", "OR"].contains(first) {
                    let col = items
                        .get(1)
                        .ok_or(AppError::ParsingError("missing column".to_string()))?;

                    let f = if first == &"AND" { And(col) } else { Or(col) };
                    Ok(f)
                } else {
                    if items.len() < 3 {
                        Err(AppError::ParsingError(
                            "unable to parse filter input".to_string(),
                        ))
                    } else {
                        let col1 = items
                            .get(0)
                            .ok_or(AppError::ParsingError("missing column1".to_string()))?;
                        let op = items
                            .get(1)
                            .ok_or(AppError::ParsingError("missing operator".to_string()))?;
                        let col2 = items
                            .get(2)
                            .ok_or(AppError::ParsingError("missing column2".to_string()))?;

                        if ["AND", "OR"].contains(op) {
                            let f2 = if op == &"AND" { And(col2) } else { Or(col2) };
                            Ok(Group(vec![Base(col1), f2]))
                        } else {
                            Err(AppError::ParsingError(
                                "invalid filter operator".to_string(),
                            ))
                        }
                    }
                }
            }
        } else {
            return Err(AppError::ParsingError(
                "filter input cannot be empty".to_string(),
            ));
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
    items: Vec<&'a str>,
    offset: u8,
}

impl<'a> SqlxFilters<'a> {
    pub fn new(filter: &'a str, offset: u8) -> Self {
        SqlxFilters {
            items: Vec::from([filter]),
            offset,
        }
    }

    pub fn add(mut self, filter: &'a str) -> Self {
        self.items.push(filter);
        self
    }

    pub fn to_query(&self, bn: &BackendName) -> Result<String, AppError> {
        use Filter::*;

        let len = &self.items.len();
        let mut filters = Vec::with_capacity(*len);

        let mut index_tracker = self.offset;

        for f in &self.items {
            let filter = Filter::try_from(*f)?;

            let fs = match filter {
                Group(items) => {
                    let ls: Vec<String> = items
                        .iter()
                        .map(|col| {
                            let bq = bn.to_query(index_tracker);
                            index_tracker += 1;

                            format!("{col} = {bq}")
                        })
                        .collect();
                    format!("({})", ls.join(" "))
                }
                _ => {
                    let bq = bn.to_query(index_tracker);
                    index_tracker += 1;

                    format!("{filter} = {bq}")
                }
            };

            filters.push(fs);
        }

        Ok(filters.join(" "))
    }
}

/// Generates query string with compatible placeholders for SQL VALUES. Allows you to
/// provide how many placeholders you would like to generate.
///
///
/// Example:
/// ```
/// let values = SqlxValues(3);
/// values.to_query(bn)?; // VALUES($1, $2, $3)
/// ```
pub struct SqlxValues(pub u8, pub u8);
impl ToQuery for SqlxValues {
    fn to_query(&self, bn: &BackendName) -> String {
        let mut values = Vec::with_capacity(self.0 as usize);

        for i in 0..self.0 {
            let bq = bn.to_query(i + self.1);
            values.push(bq);
        }

        let values = values.join(", ");
        format!("VALUES({values})")
    }
}

pub struct SqlxSetters<'a> {
    items: Vec<&'a str>,
    offset: u8,
}

impl<'a> SqlxSetters<'a> {
    pub fn new(col: &'a str, offset: u8) -> Self {
        Self {
            items: vec![col],
            offset,
        }
    }

    pub fn add(mut self, col: &'a str) -> Self {
        self.items.push(col);
        self
    }
}

impl ToQuery for SqlxSetters<'_> {
    fn to_query(&self, bn: &BackendName) -> String {
        let out: Vec<String> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, col)| {
                let bq = bn.to_query(i as u8 + self.offset);
                format!("{col} = {bq}")
            })
            .collect();

        out.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use crate::errors::AppError;

    use super::{BackendName, SqlxFilters, SqlxValues, ToQuery};

    #[test]
    fn test_sqlx_filters_pg() -> Result<(), AppError> {
        let filters = SqlxFilters::new("id", 1);
        let bn = BackendName::Postgres;

        assert_eq!(&filters.to_query(&bn)?, "id = $1");

        let filters = filters.add("AND age").add("OR name");
        assert_eq!(&filters.to_query(&bn)?, "id = $1 AND age = $2 OR name = $3");

        let filters = SqlxFilters::new("asset_path OR custom_path", 1)
            .add("AND asset_type")
            .to_query(&bn)?;

        assert_eq!(
            &filters,
            "(asset_path = $1 OR custom_path = $2) AND asset_type = $3"
        );

        Ok(())
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
    fn test_sqlx_filters_mysql() -> Result<(), AppError> {
        let filters = SqlxFilters::new("id", 1);
        let bn = BackendName::Mysql;

        assert_eq!(&filters.to_query(&bn)?, "id = ?");

        let filters = filters.add("AND age").add("OR name");
        assert_eq!(&filters.to_query(&bn)?, "id = ? AND age = ? OR name = ?");

        Ok(())
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
