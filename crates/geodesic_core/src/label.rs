use std::{fmt, str::FromStr};

use crate::LabelError;

/// A unique, public case-preserving ASCII geometric-object identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Label(String);

impl Label {
    /// Parses a label accepted by scripts, CLI requests, and project files.
    ///
    /// # Errors
    ///
    /// Returns [`LabelError`] when `value` is empty or not a portable ASCII
    /// identifier. Labels preserve their case and compare case-sensitively.
    pub fn parse(value: impl Into<String>) -> Result<Self, LabelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(LabelError::Empty);
        }

        let valid = value.bytes().enumerate().all(|(index, byte)| match byte {
            b'a'..=b'z' | b'A'..=b'Z' => true,
            b'0'..=b'9' | b'_' => index > 0,
            _ => false,
        }) && !value.ends_with('_')
            && !value.contains("__");

        if valid {
            Ok(Self(value))
        } else {
            Err(LabelError::Invalid { value })
        }
    }

    /// Returns the canonical label text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Label {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for Label {
    type Err = LabelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(test)]
mod tests {
    use super::Label;

    #[test]
    fn accepts_case_preserving_ascii_labels() {
        for value in ["a", "A", "point_a", "PointA", "line2", "point_2"] {
            assert!(
                Label::parse(value).is_ok(),
                "expected {value:?} to be valid"
            );
        }
    }

    #[test]
    fn rejects_non_identifier_labels() {
        for value in ["", "point-a", "2point", "point_", "point__a", "点"] {
            assert!(
                Label::parse(value).is_err(),
                "expected {value:?} to be invalid"
            );
        }
    }

    #[test]
    fn treats_case_as_part_of_the_public_identity() {
        assert_ne!(
            Label::parse("A").expect("label"),
            Label::parse("a").expect("label")
        );
    }
}
