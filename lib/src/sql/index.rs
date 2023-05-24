use crate::sql::comment::{mightbespace, shouldbespace};
use crate::sql::error::IResult;
use crate::sql::ident::{ident, Ident};
use crate::sql::number::number;
use crate::sql::scoring::{scoring, Scoring};
use crate::sql::Number;
use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case};
use nom::combinator::{map, opt};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum Index {
	/// (Basic) non unique
	Idx,
	/// Unique index
	Uniq,
	/// Index with Full-Text search capabilities
	Search {
		az: Ident,
		hl: bool,
		sc: Scoring,
		order: Number,
	},
}

impl Default for Index {
	fn default() -> Self {
		Self::Idx
	}
}

impl fmt::Display for Index {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Idx => Ok(()),
			Self::Uniq => f.write_str("UNIQUE"),
			Self::Search {
				az,
				hl,
				sc,
				order,
			} => {
				write!(f, "SEARCH {} {} ORDER {}", az, sc, order)?;
				if *hl {
					f.write_str(" HIGHLIGHTS")?
				}
				Ok(())
			}
		}
	}
}

pub fn index(i: &str) -> IResult<&str, Index> {
	alt((unique, search, non_unique))(i)
}

pub fn non_unique(i: &str) -> IResult<&str, Index> {
	let (i, _) = tag("")(i)?;
	Ok((i, Index::Idx))
}

pub fn unique(i: &str) -> IResult<&str, Index> {
	let (i, _) = tag_no_case("UNIQUE")(i)?;
	Ok((i, Index::Uniq))
}

pub fn order(i: &str) -> IResult<&str, Number> {
	let (i, _) = mightbespace(i)?;
	let (i, _) = tag_no_case("ORDER")(i)?;
	let (i, _) = shouldbespace(i)?;
	let (i, order) = number(i)?;
	Ok((i, order))
}

pub fn highlights(i: &str) -> IResult<&str, bool> {
	let (i, _) = mightbespace(i)?;
	alt((map(tag("HIGHLIGHTS"), |_| true), map(tag(""), |_| false)))(i)
}

pub fn search(i: &str) -> IResult<&str, Index> {
	let (i, _) = tag_no_case("SEARCH")(i)?;
	let (i, _) = shouldbespace(i)?;
	let (i, az) = ident(i)?;
	let (i, _) = shouldbespace(i)?;
	let (i, sc) = scoring(i)?;
	let (i, o) = opt(order)(i)?;
	let (i, hl) = highlights(i)?;
	Ok((
		i,
		Index::Search {
			az,
			sc,
			hl,
			order: o.unwrap_or(Number::Int(100)),
		},
	))
}