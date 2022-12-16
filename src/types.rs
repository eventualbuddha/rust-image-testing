use serde::{Deserialize, Serialize};

impl TryFrom<&str> for BallotSide {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "front" => Ok(Self::Front),
            "back" => Ok(Self::Back),
            _ => Err(()),
        }
    }
}

impl<'de> Deserialize<'de> for BallotSide {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.as_str()
            .try_into()
            .map_err(|_| serde::de::Error::custom(format!("invalid value for BallotSide: {}", s)))
    }
}

impl Serialize for BallotSide {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Front => serializer.serialize_str("front"),
            Self::Back => serializer.serialize_str("back"),
        }
    }
}

// Defines a new type that wraps a String for use as an ID.
macro_rules! idtype {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            #[allow(dead_code)]
            pub const fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

pub(crate) use idtype;

use crate::ballot_card::BallotSide;
