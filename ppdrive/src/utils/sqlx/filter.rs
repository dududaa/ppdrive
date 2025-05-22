use crate::errors::AppError;

use super::sqlx_utils::{BackendName, Filter};

/// create a grouped filter [Filter::Group] from a vec.
/// example:
/// ```
/// let input = vec!["name", "AND", "age"];
/// let filter = parse_filter_group(input)?;
/// println!("{filter}") // (name AND age)
/// ```
fn parse_filter_group<'a>(items: Vec<&'a str>) -> Result<Filter<'a>, AppError> {
    use Filter::*;

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

pub fn filter_parser<'a>(input: &'a str) -> Result<Filter<'a>, AppError> {
    use Filter::*;

    let items: Vec<&str> = input.split(" ").collect();

    if let Some(first) = items.first() {
        if items.len() == 1 {
            Ok(Base(first))
        } else {
            if ["AND", "OR"].contains(first) {
                if items.len() > 2 {
                    let it = input.split_at(4);

                    let gs = it.1.trim_start_matches("(");
                    let gs = gs.trim_end_matches(")");

                    let group_items: Vec<&str> = gs.split(" ").collect();
                    let group = Box::new(parse_filter_group(group_items)?);

                    let f = if first == &"AND" {
                        AndGroup(group)
                    } else {
                        OrGroup(group)
                    };
                    Ok(f)
                } else {
                    let col = items
                        .get(1)
                        .ok_or(AppError::ParsingError("missing column".to_string()))?;

                    let f = if first == &"AND" { And(col) } else { Or(col) };
                    Ok(f)
                }
            } else {
                parse_filter_group(items)
            }
        }
    } else {
        return Err(AppError::ParsingError(
            "filter input cannot be empty".to_string(),
        ));
    }
}

fn build_filter_query(filter: Filter<'_>, index_tracker: &mut u8, bn: &BackendName) -> String {
    use Filter::*;

    match filter {
        Group(items) => {
            let ls: Vec<String> = items
                .iter()
                .map(|col| {
                    let bq = bn.to_query(*index_tracker);
                    *index_tracker += 1;

                    format!("{col} = {bq}")
                })
                .collect();
            format!("({})", ls.join(" "))
        }
        _ => {
            let bq = bn.to_query(*index_tracker);
            *index_tracker += 1;

            format!("{filter} = {bq}")
        }
    }
}

pub fn filter_to_query(
    items: &Vec<&str>,
    offset: &u8,
    bn: &BackendName,
) -> Result<String, AppError> {
    use Filter::*;

    let len = items.len();
    let mut filters = Vec::with_capacity(len);

    let mut index_tracker = *offset;

    for f in items {
        let filter = Filter::try_from(*f)?;

        let fs = match filter {
            AndGroup(filter) => {
                let q = build_filter_query(*filter, &mut index_tracker, bn);
                format!("AND {q}")
            }
            OrGroup(filter) => {
                let q = build_filter_query(*filter, &mut index_tracker, bn);
                format!("OR {q}")
            }
            _ => build_filter_query(filter, &mut index_tracker, bn),
        };

        filters.push(fs);
    }

    Ok(filters.join(" "))
}
