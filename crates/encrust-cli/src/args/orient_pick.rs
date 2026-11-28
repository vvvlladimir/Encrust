//! `--orient-pick`: a way of the search for every object, and each object's own.

use core_edit::ObjectId;

/// Which of the search's ways `--orient-pick` takes, counted from one: each object named its own,
/// the rest `every`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrientPick {
    pub every: u16,
    pub by_object: Vec<(u64, u16)>,
}

impl OrientPick {
    /// The way object `id` takes, counted from one.
    pub fn of(pick: Option<&Self>, id: ObjectId) -> u16 {
        let Some(pick) = pick else {
            return 1;
        };
        (pick.by_object.iter())
            .find(|(object, _)| *object == id.0)
            .map_or(pick.every, |(_, way)| *way)
    }
}

pub(super) fn parse(text: &str) -> Result<OrientPick, String> {
    let ranked = |word: &str| match word.trim().parse::<u16>() {
        Ok(pick) if (1..=64).contains(&pick) => Ok(pick),
        _ => Err(format!("a way is counted from 1 to 64, got {word:?}")),
    };
    let mut every = None;
    let mut by_object: Vec<(u64, u16)> = Vec::new();
    for word in text.split(',') {
        match word.split_once(':') {
            None if every.is_some() => {
                return Err(format!("two ways for every object in {text:?}"));
            }
            None => every = Some(ranked(word)?),
            Some((id, way)) => {
                let id =
                    (id.trim().parse::<u64>()).map_err(|_| format!("{id:?} is no object id"))?;
                if by_object.iter().any(|(named, _)| *named == id) {
                    return Err(format!("two ways for object {id} in {text:?}"));
                }
                by_object.push((id, ranked(way)?));
            }
        }
    }
    Ok(OrientPick {
        every: every.unwrap_or(1),
        by_object,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_alone_is_every_object_s_way() {
        let pick = parse("3").expect("a number reads");
        assert_eq!(OrientPick::of(Some(&pick), ObjectId(2)), 3);
    }

    #[test]
    fn an_object_named_takes_its_own_way_and_the_rest_the_number_given() {
        let pick = parse("2,3:4").expect("a list reads");
        assert_eq!(OrientPick::of(Some(&pick), ObjectId(3)), 4);
        assert_eq!(OrientPick::of(Some(&pick), ObjectId(5)), 2);
        let named = parse("3:4").expect("one pair reads");
        assert_eq!(
            OrientPick::of(Some(&named), ObjectId(5)),
            1,
            "the rest take the first"
        );
    }

    #[test]
    fn a_way_out_of_range_or_a_bad_id_is_refused() {
        assert!(parse("0").is_err());
        assert!(parse("65").is_err());
        assert!(parse("x:2").is_err());
    }

    #[test]
    fn two_ways_for_one_object_or_for_every_one_are_refused() {
        assert!(parse("3:4,3:5").is_err(), "object 3 named twice");
        assert!(parse("2,3").is_err(), "every object given two ways");
    }

    #[test]
    fn no_pick_is_the_first_way() {
        assert_eq!(OrientPick::of(None, ObjectId(1)), 1);
    }
}
