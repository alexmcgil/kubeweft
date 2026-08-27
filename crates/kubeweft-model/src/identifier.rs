use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidIdentifier {
    MissingNamespace,
    EmptySegment,
    InvalidCharacter,
}

impl fmt::Display for InvalidIdentifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::MissingNamespace => {
                "identifier must contain at least one dot-separated namespace"
            }
            Self::EmptySegment => "identifier segments must not be empty",
            Self::InvalidCharacter => {
                "identifier segments may contain only lowercase ASCII letters, digits, hyphens, or underscores"
            }
        };

        formatter.write_str(message)
    }
}

impl std::error::Error for InvalidIdentifier {}

fn validate_identifier(value: &str) -> Result<(), InvalidIdentifier> {
    let mut segments = value.split('.');
    let first = segments.next().ok_or(InvalidIdentifier::MissingNamespace)?;
    let second = segments.next().ok_or(InvalidIdentifier::MissingNamespace)?;

    for segment in std::iter::once(first)
        .chain(std::iter::once(second))
        .chain(segments)
    {
        if segment.is_empty() {
            return Err(InvalidIdentifier::EmptySegment);
        }

        if !segment.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        }) {
            return Err(InvalidIdentifier::InvalidCharacter);
        }
    }

    Ok(())
}

macro_rules! identifier_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidIdentifier> {
                let value = value.into();
                validate_identifier(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = InvalidIdentifier;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
    };
}

identifier_type!(DeviceId);
identifier_type!(CapabilityId);
identifier_type!(JobId);
